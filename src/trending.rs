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
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, db, redis, headers, 0).await
}

async fn hx_trending_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_trending_inner(config, db, redis, headers, page).await
}

/// Try to load a page of trending media from the Redis cache.
async fn try_trending_from_cache(redis: &mut RedisConn, offset: i64) -> Option<Vec<Medium>> {
    let exists: bool = redis.exists("cache:trending").await.ok()?;
    if !exists {
        return None;
    }

    let stop = offset + 30;
    let ids: Vec<String> = redis
        .zrevrange("cache:trending", offset as isize, stop as isize)
        .await
        .ok()?;

    if ids.is_empty() {
        return Some(Vec::new());
    }

    let mut pipe = redis::pipe();
    for id in &ids {
        pipe.cmd("HGETALL")
            .arg(format!("cache:trending:info:{}", id));
    }
    let results: Vec<std::collections::HashMap<String, String>> =
        pipe.query_async(redis).await.ok()?;

    let mut media = Vec::with_capacity(ids.len());
    for (id, info) in ids.into_iter().zip(results) {
        if info.is_empty() {
            continue;
        }
        media.push(Medium {
            id,
            name: info.get("name").cloned().unwrap_or_default(),
            owner: info.get("owner").cloned().unwrap_or_default(),
            views: info
                .get("views")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            r#type: info.get("type").cloned().unwrap_or_default(),
        });
    }

    Some(media)
}

async fn hx_trending_inner(
    config: Config,
    db: Db,
    mut redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let offset = page * 30;

    // Try Redis cache first (pre-computed by the indexer)
    let mut media = match try_trending_from_cache(&mut redis, offset).await {
        Some(cached) => cached,
        None => {
            // Cache not available — fall back to direct DB query
            // Use graph traversal to order by like count
            let user = get_user_login(headers, &db, redis.clone()).await;
            let user_login = user.map(|u| u.login).unwrap_or_default();
            let group_ids = get_user_group_ids(&db, &user_login).await;

            let mut result = db
                .query("SELECT record::id(id) AS id, name, owner, views, type FROM media WHERE visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN $groups) ORDER BY views DESC LIMIT 31 START $offset")
                .bind(("groups", &group_ids))
                .bind(("offset", offset))
                .await
                .expect("Database error");

            result.take(0).expect("Database error")
        }
    };

    let has_more = media.len() == 31;
    if has_more {
        media.truncate(30);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/trending/{}", next_page);

    let template = HXMediumCardTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
