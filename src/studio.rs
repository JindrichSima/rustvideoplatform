#[derive(Template)]
#[template(path = "pages/studio.html", escape = "none")]
struct StudioTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn studio(
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
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumStudio {
    id: String,
    name: String,
    description: String,
    views: i64,
    r#type: String,
}
#[derive(Template)]
#[template(path = "pages/hx-studio.html", escape = "none")]
struct HXStudioTemplate {
    media: Vec<MediumStudio>,
}
async fn hx_studio(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let media = sqlx::query_as!(
        MediumStudio,
        "SELECT id,name,description,views,type FROM media WHERE owner=$1 ORDER BY upload DESC;",
        user_info.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
    let template = HXStudioTemplate { media };
    Html(minifi_html(template.render().unwrap()))
}
