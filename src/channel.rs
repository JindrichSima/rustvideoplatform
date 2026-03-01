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
    Extension(db): Extension<Db>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    // Two queries in one call: user info + subscriber count via graph
    #[derive(Deserialize)]
    struct UserInfo {
        login: String,
        name: String,
        profile_picture: Option<String>,
        channel_picture: Option<String>,
    }
    #[derive(Deserialize)]
    struct CountRow { count: i64 }

    let mut result = db
        .query("SELECT record::id(id) AS login, name, profile_picture, channel_picture FROM type::thing('users', $userid); SELECT count() AS count FROM subscribes WHERE out = type::thing('users', $userid) GROUP ALL")
        .bind(("userid", &userid))
        .await
        .expect("Database error");

    let user_info: Option<UserInfo> = result.take(0).expect("Database error");
    let sub_count: Option<CountRow> = result.take(1).unwrap_or(None);

    let user_info = user_info.unwrap();
    let user = UserChannel {
        login: user_info.login,
        name: user_info.name,
        profile_picture: user_info.profile_picture,
        channel_picture: user_info.channel_picture,
        subscribed: Some(sub_count.map(|r| r.count).unwrap_or(0)),
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

    let group_ids = get_user_group_ids(&db, &user_login).await;

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, views, type FROM media WHERE owner = $userid AND (visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN $groups)) ORDER BY upload DESC LIMIT 41 START $offset")
        .bind(("userid", &userid))
        .bind(("groups", &group_ids))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut media: Vec<Medium> = result.take(0).expect("Database error");

    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/usermedia/{}/{}", userid, next_page);

    let template = HXMediumCardTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
