#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
    r#type: String,
}

#[derive(Template)]
#[template(path = "pages/concepts.html", escape = "none")]
struct ConceptsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn concepts(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "trending".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ConceptsTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html", escape = "none")]
struct HXConceptsTemplate {
    concepts: Vec<MediumConcept>,
}
async fn hx_concepts(Extension(pool): Extension<PgPool>, Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>, headers: HeaderMap) -> axum::response::Html<Vec<u8>> {
    let userinfo = get_user_login(headers, &pool, session_store).await.unwrap();

    let concepts = sqlx::query_as!(MediumConcept,
        "SELECT id,name,processed,type FROM media_concepts WHERE owner = $1;", userinfo.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
    let template = HXConceptsTemplate { concepts };
    Html(minifi_html(template.render().unwrap()))
}