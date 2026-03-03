#[derive(Serialize, Deserialize)]
struct UserChannel {
    login: String,
    name: String,
    profile_picture: Option<String>,
    channel_picture: Option<String>,
    subscribed: Option<i64>,
}
#[derive(Template)]
#[template(path = "pages/channel.html", escape = "none")]
struct ChannelTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    user: UserChannel,
}
async fn channel(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = sqlx::query_as!(
        UserChannel,
        "SELECT
    u.login,
    u.name,
    u.profile_picture,
    u.channel_picture,
    COALESCE(subs.count, 0) AS subscribed
FROM
    users u
LEFT JOIN
    (
        SELECT
            target,
            COUNT(*) AS count
        FROM
            subscriptions
        GROUP BY
            target
    ) subs
ON
    u.login = subs.target
WHERE
    u.login = $1;",
        userid
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sidebar = generate_sidebar(&config, "channel".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ChannelTemplate {
        sidebar,
        config,
        common_headers,
        user,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_usermedia(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, pool, redis, headers, userid, 0).await
}

async fn hx_usermedia_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, pool, redis, headers, userid, page).await
}

async fn hx_usermedia_inner(
    config: Config,
    pool: PgPool,
    redis: RedisConn,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &pool, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let offset = page * 40;

    let mut media: Vec<Medium> = sqlx::query(
        "SELECT id,name,owner,views,type FROM media WHERE owner=$1 AND (visibility = 'public' OR (visibility = 'restricted' AND (restricted_to_group IN (SELECT group_id FROM user_group_members WHERE user_login = $2) OR (restricted_to_group = '__all_registered__' AND $2 != '') OR (restricted_to_group = '__subscribers__' AND $2 != '' AND owner IN (SELECT target FROM subscriptions WHERE subscriber = $2))))) ORDER BY upload DESC LIMIT 41 OFFSET $3;"
    )
    .bind(&userid)
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

    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/usermedia/{}/{}", userid, next_page);

    let template = HXMediumCardTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
