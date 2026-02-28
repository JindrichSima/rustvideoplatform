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

async fn hx_trending(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, pool, redis, headers, 0).await
}

async fn hx_trending_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, pool, redis, headers, page).await
}

async fn hx_trending_inner(
    config: Config,
    pool: PgPool,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let offset = page * 30;

    let mut media: Vec<Medium> = sqlx::query(
        "SELECT id,name,owner,views,type FROM media WHERE visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN (SELECT group_id FROM user_group_members WHERE user_login = $1)) ORDER BY likes DESC LIMIT 31 OFFSET $2;"
    )
    .bind(&user_login)
    .bind(offset)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        Medium {
            id: row.get("id"),
            name: row.get("name"),
            owner: row.get("owner"),
            views: row.get("views"),
            r#type: row.get("type"),
        }
    })
    .fetch_all(&pool)
    .await
    .expect("Database error");

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/trending/{}", next_page);

    let template = HXMediumCardTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
