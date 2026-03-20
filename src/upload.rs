#[derive(Template)]
#[template(path = "pages/hx-studio-upload.html", escape = "none")]
struct HXStudioUploadTemplate {}
async fn hx_studio_upload(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let template = HXStudioUploadTemplate {};
    Html(minifi_html(template.render().unwrap()))
}

async fn upload(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers);
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "upload".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_upload(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Step 1: Authenticate User
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Ok(Html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    // Step 2: Setup directories
    let upload_dir = std::path::Path::new("upload");
    if let Err(e) = tokio::fs::create_dir_all(upload_dir).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!(
                "<div class=\"alert alert-danger\">Failed to create upload directory: {}</div>",
                e
            )),
        ));
    }
    let medium_id = generate_medium_id();

    // Step 3: Process each field in the multipart form
    let field = match multipart.next_field().await {
        Ok(Some(field)) => field,
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Html("<div class=\"alert alert-danger\">No file was provided</div>".to_owned()),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Html(format!(
                    "<div class=\"alert alert-danger\">Upload error: {}</div>",
                    e
                )),
            ));
        }
    };

    let file_name = field.file_name().unwrap_or("unnamed").to_string();
    let file_type = field
        .content_type()
        .map(|ct| ct.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Step 4: Open the file for writing
    let file_path = upload_dir.join(&medium_id);
    let mut file = match tokio::fs::File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    "<div class=\"alert alert-danger\">Failed to create file: {}</div>",
                    e
                )),
            ));
        }
    };

    // Step 5: Stream and write chunks to the file
    let mut file_size: usize = 0;
    let mut field = field;
    loop {
        match field.chunk().await {
            Ok(Some(chunk)) => {
                file_size += chunk.len();
                if let Err(e) = file.write_all(&chunk).await {
                    // Clean up partial file on write error
                    let _ = tokio::fs::remove_file(&file_path).await;
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!(
                            "<div class=\"alert alert-danger\">Write error: {}</div>",
                            e
                        )),
                    ));
                }
            }
            Ok(None) => break,
            Err(e) => {
                // Clean up partial file on read error
                let _ = tokio::fs::remove_file(&file_path).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    Html(format!(
                        "<div class=\"alert alert-danger\">Upload interrupted: {}</div>",
                        e
                    )),
                ));
            }
        }
    }

    if let Err(e) = file.flush().await {
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!(
                "<div class=\"alert alert-danger\">Failed to finalize file: {}</div>",
                e
            )),
        ));
    }

    // Step 6: Save metadata to the database
    let medium_type = detect_medium_type_mime(file_type.clone());
    if let Err(e) = sqlx::query!(
        "INSERT INTO media_concepts (id, name, owner, type) VALUES ($1, $2, $3, $4)",
        medium_id,
        file_name,
        user_info.unwrap().login,
        medium_type
    )
    .execute(&pool)
    .await
    {
        let _ = tokio::fs::remove_file(&file_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!(
                "<div class=\"alert alert-danger\">Database error: {}</div>",
                e
            )),
        ));
    }

    // Step 7: Format success response
    let formatted_file_size = format_file_size(file_size);
    let response_html = format!(
        r#"<div class="text-center">
            <h3 class="text-success mb-3"><i class="fa-solid fa-circle-check"></i> Upload complete!</h3>
            <table class="table table-sm" style="max-width:400px;margin:0 auto">
                <tr><th>File</th><td>{}</td></tr>
                <tr><th>Size</th><td>{}</td></tr>
                <tr><th>Type</th><td>{}</td></tr>
            </table>
            <a href="/studio/concepts" class="btn btn-primary mt-3" preload="mouseover">
                <i class="fa-solid fa-arrow-right"></i> View Concepts
            </a>
        </div>"#,
        file_name, formatted_file_size, medium_type
    );

    Ok(Html(response_html))
}
