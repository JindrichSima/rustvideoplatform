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

    #[derive(Deserialize)]
    struct MediumRow {
        id: surrealdb::RecordId,
        name: String,
        owner: String,
        views: i64,
        #[serde(rename = "type")]
        r#type: String,
    }

    let mut resp = db
        .query(
            "SELECT id, name, owner, views, type FROM media \
             WHERE owner IN (SELECT VALUE target FROM subscriptions WHERE subscriber = $user) \
             AND (visibility = 'public' OR (visibility = 'restricted' AND ( \
               restricted_to_group IN (SELECT VALUE group_id FROM user_group_members WHERE user_login = $user) \
               OR restricted_to_group = '__all_registered__' \
               OR (restricted_to_group = '__subscribers__' AND owner IN (SELECT VALUE target FROM subscriptions WHERE subscriber = $user)) \
             ))) \
             ORDER BY upload DESC LIMIT $limit START $offset"
        )
        .bind(("user", &user.login))
        .bind(("limit", 31i64))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let rows: Vec<MediumRow> = resp.take(0).unwrap_or_default();

    let mut media: Vec<Medium> = rows.into_iter().map(|row| Medium {
        id: row.id.key().to_string(),
        name: row.name,
        owner: row.owner,
        views: row.views,
        r#type: row.r#type,
        sprite_filename: None,
        sprite_x: 0,
        sprite_y: 0,
    }).collect();

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/subscriptions/{}", next_page);

    let template = HXMediumCardTemplate {
        media,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_subscribe(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &db, redis.clone()).await.unwrap();
    db.query(
        "UPSERT subscriptions:[$subscriber, $target] SET subscriber = $subscriber, target = $target"
    )
    .bind(("subscriber", &user.login))
    .bind(("target", &userid))
    .await
    .expect("Database error");
    Html(format!("<a hx-get=\"/hx/unsubscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-secondary\"><i class=\"fa-solid fa-user-minus\"></i>&nbsp;Unsubscribe</a>", userid))
}

async fn hx_unsubscribe(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    let user = get_user_login(headers, &db, redis.clone()).await.unwrap();
    db.query(
        "DELETE subscriptions WHERE subscriber = $subscriber AND target = $target"
    )
    .bind(("subscriber", &user.login))
    .bind(("target", &userid))
    .await
    .expect("Database error");
    Html(format!("<a hx-get=\"/hx/subscribe/{}\" hx-swap=\"outerHTML\" class=\"btn btn-primary\"><i class=\"fa-solid fa-user-plus\"></i>&nbsp;Subscribe</a>", userid))
}

async fn hx_subscribebutton(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(userid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let issubscribed = is_subscribed(&db, &user.login, &userid).await;

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
