#[derive(Template)]
#[template(path = "pages/upload.html", escape = "none")]
struct UploadTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn upload(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = UploadTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_upload(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Html<String> {
    // Step 1: Authenticate User
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }

    // Step 2: Setup directories
    let upload_dir = std::path::Path::new("upload");
    fs::create_dir_all(upload_dir).await.unwrap(); // Ensure upload directory exists
    let medium_id = generate_medium_id();

    let mut response_html = String::new();
    response_html
        .push_str("<h3 class=\"text-center text-success\">File uploaded successfully!</h3>");

    // Step 3: Process multipart fields (handle file chunks)
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let chunk_index: usize = field.name().and_then(|n| n.parse().ok()).unwrap_or(0);
        let total_chunks: usize = field
            .headers()
            .get("total-chunks")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        // Determine file path
        let file_path = upload_dir.join(&medium_id);

        // Create or append to the file
        let mut file = if chunk_index == 0 {
            fs::File::create(&file_path).await.unwrap() // Create a new file for the first chunk
        } else {
            fs::File::options()
                .append(true)
                .open(&file_path)
                .await
                .unwrap() // Append for subsequent chunks
        };

        // Write the current chunk to the file
        while let Some(chunk) = field.chunk().await.unwrap() {
            file.write_all(&chunk).await.unwrap();
        }

        // If it's the last chunk, finalize the response
        if chunk_index + 1 == total_chunks {
            let file_type = field
                .content_type()
                .map(|ct| ct.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let file_size = fs::metadata(&file_path).await.unwrap().len();
            let formatted_file_size = format_file_size(file_size as usize);
        }

        // Update response HTML
        response_html.push_str("<table cellpadding=\"10\">");
        response_html.push_str(&format!(
            "<tr><th>File Name</th><td>{}</td></tr>",
            file_name
        ));
        response_html.push_str(&format!(
            "<tr><th>Medium ID</th><td>{}</td></tr>",
            medium_id
        ));
        response_html.push_str(&format!(
            "<tr><th>File Size</th><td>{}</td></tr>",
            formatted_file_size
        ));
        response_html.push_str(&format!(
            "<tr><th>File Type</th><td>{}</td></tr>",
            file_type
        ));
        response_html.push_str(
                    "<tr><th><a href=\"/studio/concepts\" class=\"btn btn-primary\">View Concepts</a></th></tr>"
                );
        response_html.push_str("</table><br>");
    }
    sqlx::query!(
        "INSERT INTO media_concepts (id, name, owner, type) VALUES ($1, $2, $3, $4)",
        medium_id,
        file_name,
        user_info.clone().unwrap().login,
        detect_medium_type_mime(file_type)
    )
    .execute(&pool)
    .await
    .expect("Database error");
    Html(response_html)
}
