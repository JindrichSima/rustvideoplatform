#[derive(Template)]
#[template(path = "pages/hx-studio-upload.html", escape = "none")]
struct HXStudioUploadTemplate {}
async fn hx_studio_upload(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let template = HXStudioUploadTemplate {};
    Html(minifi_html(template.render().unwrap()))
}

async fn upload(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "upload".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_upload(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }

    let upload_dir = std::path::Path::new("upload");
    tokio::fs::create_dir_all(upload_dir).await.unwrap();
    let medium_id = generate_medium_id();

    let mut response_html = String::new();
    response_html
        .push_str("<h3 class=\"text-center text-success\">File uploaded successfully!</h3>");

    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let file_name = field.file_name().unwrap().to_string();
        let file_type = field
            .content_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let file_path = upload_dir.join(&medium_id);
        let mut file = tokio::fs::File::create(file_path.clone()).await.unwrap();

        let mut file_size = 0;
        while let Some(chunk) = field.chunk().await.unwrap() {
            file_size += chunk.len();
            file.write_all(&chunk).await.unwrap();
        }

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
            "<tr><th><a href=\"/studio/concepts\" class=\"btn btn-primary\" preload=\"mouseover\">View Concepts</a></th></tr>"
        );
        response_html.push_str("</table><br>");

        let medium_type = detect_medium_type_mime(file_type);
        db.query("CREATE type::thing('media_concepts', $id) SET name = $name, owner = $owner, type = $type")
            .bind(("id", &medium_id))
            .bind(("name", &file_name))
            .bind(("owner", &user_info.clone().unwrap().login))
            .bind(("type", &medium_type))
            .await
            .expect("Database error");
    }

    Html(response_html)
}
