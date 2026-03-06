fn like_dislike_html(mediumid: &str, likes: i64, dislikes: i64, user_reaction: Option<&str>) -> String {
    let like_color = if user_reaction == Some("like") { "var(--bs-success)" } else { "var(--bs-white)" };
    let dislike_color = if user_reaction == Some("dislike") { "var(--bs-danger)" } else { "var(--bs-white)" };
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" style="cursor: pointer; color: {like_color}" hx-get="/hx/like/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" style="cursor: pointer; color: {dislike_color}" hx-get="/hx/dislike/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

fn like_dislike_html_unauth(mediumid: &str, likes: i64, dislikes: i64) -> String {
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" href="/login"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" href="/login"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

/// Get reaction counts from DB (authoritative source). Used after write operations.
async fn get_reaction_state_db(pool: &PgPool, mediumid: &str, user_login: Option<&str>) -> (i64, i64, Option<String>) {
    use sqlx::Row;

    let counts_row = sqlx::query(
        "SELECT COUNT(*) FILTER (WHERE reaction = 'like') AS likes, COUNT(*) FILTER (WHERE reaction = 'dislike') AS dislikes FROM media_likes WHERE media_id=$1;"
    )
    .bind(mediumid)
    .fetch_one(pool)
    .await
    .expect("Database error");

    let likes: i64 = counts_row.get("likes");
    let dislikes: i64 = counts_row.get("dislikes");

    let user_reaction = if let Some(login) = user_login {
        sqlx::query(
            "SELECT reaction FROM media_likes WHERE media_id=$1 AND user_login=$2;"
        )
        .bind(mediumid)
        .bind(login)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            row.get::<String, _>("reaction")
        })
    } else {
        None
    };

    (likes, dislikes, user_reaction)
}

/// Get reaction state using Redis cache for counts (falls back to DB on cache miss).
/// User's personal reaction is always fetched from DB.
async fn get_reaction_state_cached(pool: &PgPool, mediumid: &str, user_login: Option<&str>, mut redis: RedisConn) -> (i64, i64, Option<String>) {
    // Try getting counts from Redis cache
    let cached_likes: Result<i64, _> = redis.get(format!("cache:media:{}:likes", mediumid)).await;
    let cached_dislikes: Result<i64, _> = redis.get(format!("cache:media:{}:dislikes", mediumid)).await;

    let (likes, dislikes) = if let (Ok(l), Ok(d)) = (cached_likes, cached_dislikes) {
        (l, d)
    } else {
        // Cache miss — fall back to DB
        use sqlx::Row;
        let counts_row = sqlx::query(
            "SELECT COUNT(*) FILTER (WHERE reaction = 'like') AS likes, COUNT(*) FILTER (WHERE reaction = 'dislike') AS dislikes FROM media_likes WHERE media_id=$1;"
        )
        .bind(mediumid)
        .fetch_one(pool)
        .await
        .expect("Database error");
        (counts_row.get("likes"), counts_row.get("dislikes"))
    };

    // User's personal reaction must come from DB (per-user, not worth caching individually)
    let user_reaction = if let Some(login) = user_login {
        sqlx::query(
            "SELECT reaction FROM media_likes WHERE media_id=$1 AND user_login=$2;"
        )
        .bind(mediumid)
        .bind(login)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            row.get::<String, _>("reaction")
        })
    } else {
        None
    };

    (likes, dislikes, user_reaction)
}

/// Update the cached reaction counts in Redis after a write operation.
async fn update_reaction_cache(redis: &mut RedisConn, mediumid: &str, likes: i64, dislikes: i64) {
    let _: Result<(), _> = redis.set(format!("cache:media:{}:likes", mediumid), likes).await;
    let _: Result<(), _> = redis.set(format!("cache:media:{}:dislikes", mediumid), dislikes).await;
}

async fn hx_likedislikebutton(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, redis.clone()).await {
        let (likes, dislikes, user_reaction) = get_reaction_state_cached(&pool, &mediumid, Some(&user.login), redis.clone()).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&pool, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}

async fn hx_like(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, redis.clone()).await {
        let (_, _, current_reaction) = get_reaction_state_db(&pool, &mediumid, Some(&user.login)).await;

        if current_reaction.as_deref() == Some("like") {
            sqlx::query(
                "DELETE FROM media_likes WHERE media_id=$1 AND user_login=$2;"
            )
            .bind(&mediumid)
            .bind(&user.login)
            .execute(&pool)
            .await
            .expect("Database error");
        } else {
            sqlx::query(
                "INSERT INTO media_likes (media_id, user_login, reaction) VALUES ($1,$2,'like') ON CONFLICT (media_id, user_login) DO UPDATE SET reaction='like';"
            )
            .bind(&mediumid)
            .bind(&user.login)
            .execute(&pool)
            .await
            .expect("Database error");
        }

        // Get fresh counts from DB after the write
        let (likes, dislikes, user_reaction) = get_reaction_state_db(&pool, &mediumid, Some(&user.login)).await;
        // Update Redis cache with the new counts
        let mut cache_redis = redis.clone();
        update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&pool, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}

async fn hx_dislike(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, redis.clone()).await {
        let (_, _, current_reaction) = get_reaction_state_db(&pool, &mediumid, Some(&user.login)).await;

        if current_reaction.as_deref() == Some("dislike") {
            sqlx::query(
                "DELETE FROM media_likes WHERE media_id=$1 AND user_login=$2;"
            )
            .bind(&mediumid)
            .bind(&user.login)
            .execute(&pool)
            .await
            .expect("Database error");
        } else {
            sqlx::query(
                "INSERT INTO media_likes (media_id, user_login, reaction) VALUES ($1,$2,'dislike') ON CONFLICT (media_id, user_login) DO UPDATE SET reaction='dislike';"
            )
            .bind(&mediumid)
            .bind(&user.login)
            .execute(&pool)
            .await
            .expect("Database error");
        }

        // Get fresh counts from DB after the write
        let (likes, dislikes, user_reaction) = get_reaction_state_db(&pool, &mediumid, Some(&user.login)).await;
        // Update Redis cache with the new counts
        let mut cache_redis = redis.clone();
        update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&pool, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}
