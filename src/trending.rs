#[derive(Template)]
#[template(path = "pages/trending.html", escape = "none")]
struct TrendingTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn trending(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "trending".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = TrendingTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_trending(Extension(config): Extension<Config>, Extension(pool): Extension<PgPool>) -> axum::response::Html<Vec<u8>> {
    let media = sqlx::query_as!(Medium,
        "SELECT id,name,owner,views,type FROM media WHERE public=true ORDER BY likes DESC LIMIT 100;"
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let template = HXMediumCardTemplate { media, config };
    Html(minifi_html(template.render().unwrap()))
}
