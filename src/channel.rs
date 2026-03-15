#[derive(Serialize, Deserialize, SurrealValue)]
struct UserChannel {
    login: String,
    name: String,
    profile_picture: Option<String>,
    channel_picture: Option<String>,
    subscribed: Option<i64>,
}

#[derive(Deserialize, SurrealValue)]
struct UserChannelRow {
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
    Extension(db): Extension<Db>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let mut response = db
        .query(
            "SELECT
    login,
    name,
    profile_picture,
    channel_picture,
    (SELECT count() FROM subscriptions WHERE target = $owner)[0].count AS subscribed
FROM users
WHERE login = $id;",
        )
        .bind(("id", userid.clone()))
        .bind(("owner", userid.clone()))
        .await
        .expect("Database error");

    let row: Option<UserChannelRow> = response.take(0).expect("Database error");
    let row = row.expect("User not found");

    let user = UserChannel {
        login: row.login,
        name: row.name,
        profile_picture: row.profile_picture,
        channel_picture: row.channel_picture,
        subscribed: row.subscribed,
    };

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
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, db, redis, headers, userid, 0).await
}

async fn hx_usermedia_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_usermedia_inner(config, db, redis, headers, userid, page).await
}

#[derive(Deserialize, SurrealValue)]
struct ChannelMediumRow {
    id: RecordId,
    name: String,
    owner: String,
    views: i64,
    #[serde(rename = "type")]
    r#type: String,
}

async fn hx_usermedia_inner(
    config: Config,
    db: Db,
    redis: RedisConn,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let offset = page * 40;
    let limit: i64 = 41;

    let mut response = db
        .query(
            "SELECT id, name, owner, views, type FROM media \
             WHERE owner = $owner \
             AND fn::visible_to(visibility, restricted_to_group, owner, $user) \
             ORDER BY upload DESC LIMIT $lim START $offset",
        )
        .bind(("owner", userid.clone()))
        .bind(("user", user_login.clone()))
        .bind(("lim", limit))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let rows: Vec<ChannelMediumRow> = response.take(0).expect("Database error");
    let mut media: Vec<Medium> = rows
        .into_iter()
        .map(|row| Medium {
            id: row.id.key_string(),
            name: row.name,
            owner: row.owner,
            views: row.views,
            r#type: row.r#type,
            sprite_filename: None,
            sprite_x: 0,
            sprite_y: 0,
        })
        .collect();

    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/usermedia/{}/{}", userid, next_page);

    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}
