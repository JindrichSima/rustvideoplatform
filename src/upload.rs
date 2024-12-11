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

    // Step 2: Setup directories and response template
    let upload_dir = std::path::Path::new("upload");
    tokio::fs::create_dir_all(upload_dir).await.unwrap(); // Ensure upload directory exists
    let medium_id = generate_medium_id();

    let mut response_html = String::new();
    response_html
        .push_str("<h3 class=\"text-center text-success\">File uploaded successfully!</h3>");

    // Step 3: Process each field in the multipart form
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap().to_string();
        let file_type = field
            .content_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Step 4: Open the file for writing
        let file_path = upload_dir.join(&medium_id);
        let mut file = tokio::fs::File::create(file_path.clone()).await.unwrap();

        // Step 5: Stream and write chunks to the file
        let mut file_size = 0;
        while let Some(chunk) = field.chunk().await.unwrap() {
            file_size += chunk.len();
            file.write_all(&chunk).await.unwrap();
        }

        // Step 6: Format response HTML
        let formatted_file_size = format_file_size(file_size);
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

        // Step 7: Save metadata to the database
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
    }

    Html(response_html)
}
