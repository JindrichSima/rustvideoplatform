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

struct MediumWithShowcase {
    medium: Medium,
    showcase_exists: bool,
}

#[derive(Template)]
#[template(path = "pages/hx-trending.html", escape = "none")]
struct HXTrendingTemplate {
    reccomendations: Vec<MediumWithShowcase>,
}

fn showcase_exists(medium_id: &str) -> bool {
    std::path::Path::new(&format!("source/{}/showcase.avif", medium_id)).exists()
}

async fn hx_trending(Extension(pool): Extension<PgPool>) -> axum::response::Html<Vec<u8>> {
    let media = sqlx::query_as!(Medium,
        "SELECT id,name,owner,views,type FROM media WHERE public=true ORDER BY likes DESC LIMIT 100;"
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let reccomendations: Vec<MediumWithShowcase> = media
        .into_iter()
        .map(|m| {
            let has_showcase = showcase_exists(&m.id);
            MediumWithShowcase {
                medium: m,
                showcase_exists: has_showcase,
            }
        })
        .collect();

    let template = HXTrendingTemplate { reccomendations };
    Html(minifi_html(template.render().unwrap()))
}
