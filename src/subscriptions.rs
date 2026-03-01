#[derive(Template)]
#[template(path = "pages/subscriptions.html", escape = "none")]
struct SubscriptionsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}

async fn subscriptions(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let sidebar = generate_sidebar(&config, "subscribed".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SubscriptionsTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscriptions(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
) -> axum::response::Html<Vec<u8>> {
    hx_subscriptions_inner(config, headers, db, redis, 0).await
}

async fn hx_subscriptions_page(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_subscriptions_inner(config, headers, db, redis, page).await
}

async fn hx_subscriptions_inner(
    config: Config,
    headers: HeaderMap,
    db: Db,
    redis: RedisConn,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = match get_user_login(headers, &db, redis.clone()).await {
        Some(user) => user,
        None => {
            return Html(
                "Please log in to see your subscriptions"
                    .as_bytes()
                    .to_vec(),
            )
        }
    };

    let offset = page * 30;

    // Get subscribed users' media using graph traversal
    let group_ids = get_user_group_ids(&db, &user.login).await;
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, views, type FROM media WHERE owner IN (SELECT VALUE record::id(out) FROM subscribes WHERE in = type::thing('users', $subscriber)) AND (visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN $groups)) ORDER BY upload DESC LIMIT 31 START $offset")
        .bind(("subscriber", &user.login))
        .bind(("groups", &group_ids))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut media: Vec<Medium> = result.take(0).expect("Database error");

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/subscriptions/{}", next_page);

    let template = HXMediumCardTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscribe(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &db, redis.clone()).await.unwrap();
    // Create subscription graph edge
    db.query("RELATE type::thing('users', $subscriber) -> subscribes -> type::thing('users', $target)")
        .bind(("subscriber", &user.login))
        .bind(("target", &userid))
        .await
        .expect("Database error");
    Html(format!("<a hx-get=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>",user.login))
}

async fn hx_unsubscribe(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &db, redis.clone()).await.unwrap();
    // Delete subscription graph edge
    db.query("DELETE FROM subscribes WHERE in = type::thing('users', $subscriber) AND out = type::thing('users', $target)")
        .bind(("subscriber", &user.login))
        .bind(("target", &userid))
        .await
        .expect("Database error");
    Html(format!("<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",user.login))
}

async fn hx_subscribebutton(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        #[derive(Deserialize)]
        struct CountRow { count: i64 }

        let mut result = db
            .query("SELECT count() AS count FROM subscribes WHERE in = type::thing('users', $subscriber) AND out = type::thing('users', $target) GROUP ALL")
            .bind(("subscriber", &user.login))
            .bind(("target", &userid))
            .await
            .unwrap_or_else(|_| unreachable!());

        let count_row: Option<CountRow> = result.take(0).unwrap_or(None);
        let issubscribed = count_row.map(|r| r.count > 0).unwrap_or(false);

        let button = if issubscribed {
            format!(
                "<a hx-get=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>",
                userid
            )
        } else {
            format!(
                "<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>",
                userid
            )
        };

        return Html(button);
    }

    Html("<a href=\"/login\" class=\"btn btn-primary\" preload=\"mouseover\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>".to_string())
}
