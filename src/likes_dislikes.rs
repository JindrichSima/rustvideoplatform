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

#[derive(Deserialize)]
struct ReactionStateRow {
    likes: i64,
    dislikes: i64,
    user_reaction: Option<String>,
}

/// Get reaction state from DB in a single query.
/// Reads denormalized likes_count/dislikes_count fields (kept current by DEFINE EVENT)
/// and fetches the current user's reaction in the same round-trip.
async fn get_reaction_state_db(db: &Db, mediumid: &str, user_login: Option<&str>) -> (i64, i64, Option<String>) {
    let user = user_login.unwrap_or("");
    let mut resp = db
        .query(
            "SELECT likes_count AS likes, dislikes_count AS dislikes, \
             (SELECT reaction FROM media_likes WHERE media_id = $mid AND user_login = $user LIMIT 1)[0].reaction AS user_reaction \
             FROM media WHERE id = $mid"
        )
        .bind(("mid", surrealdb::RecordId::from_table_key("media", mediumid)))
        .bind(("user", user))
        .await
        .expect("Database error");

    let rows: Vec<ReactionStateRow> = resp.take(0).unwrap_or_default();
    let row = rows.into_iter().next().unwrap_or(ReactionStateRow { likes: 0, dislikes: 0, user_reaction: None });
    (row.likes, row.dislikes, row.user_reaction)
}

/// Get reaction state using Redis cache for counts.
/// On cache hit: reads counts from Redis and user reaction from DB (one query).
/// On cache miss: single DB query for counts + user reaction using denormalized fields.
async fn get_reaction_state_cached(db: &Db, mediumid: &str, user_login: Option<&str>, mut redis: RedisConn) -> (i64, i64, Option<String>) {
    let cached_likes: Result<i64, _> = redis.get(format!("cache:media:{}:likes", mediumid)).await;
    let cached_dislikes: Result<i64, _> = redis.get(format!("cache:media:{}:dislikes", mediumid)).await;
    let user = user_login.unwrap_or("");

    if let (Ok(likes), Ok(dislikes)) = (cached_likes, cached_dislikes) {
        // Counts come from cache; fetch only the user's reaction
        let user_reaction = if user.is_empty() {
            None
        } else {
            #[derive(Deserialize)] struct ReactionRow { reaction: String }
            let mut resp = db
                .query("SELECT reaction FROM media_likes WHERE media_id = $mid AND user_login = $user LIMIT 1")
                .bind(("mid", mediumid))
                .bind(("user", user))
                .await
                .unwrap_or_else(|_| unreachable!());
            let rows: Vec<ReactionRow> = resp.take(0).unwrap_or_default();
            rows.into_iter().next().map(|r| r.reaction)
        };
        (likes, dislikes, user_reaction)
    } else {
        // Cache miss: single query — denormalized counts + user reaction
        let mut resp = db
            .query(
                "SELECT likes_count AS likes, dislikes_count AS dislikes, \
                 (SELECT reaction FROM media_likes WHERE media_id = $mid AND user_login = $user LIMIT 1)[0].reaction AS user_reaction \
                 FROM media WHERE id = $mid"
            )
            .bind(("mid", surrealdb::RecordId::from_table_key("media", mediumid)))
            .bind(("user", user))
            .await
            .expect("Database error");
        let rows: Vec<ReactionStateRow> = resp.take(0).unwrap_or_default();
        let row = rows.into_iter().next().unwrap_or(ReactionStateRow { likes: 0, dislikes: 0, user_reaction: None });
        (row.likes, row.dislikes, row.user_reaction)
    }
}

async fn update_reaction_cache(redis: &mut RedisConn, mediumid: &str, likes: i64, dislikes: i64) {
    let _: Result<(), _> = redis.set(format!("cache:media:{}:likes", mediumid), likes).await;
    let _: Result<(), _> = redis.set(format!("cache:media:{}:dislikes", mediumid), dislikes).await;
}

async fn hx_likedislikebutton(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let (likes, dislikes, user_reaction) = get_reaction_state_cached(&db, &mediumid, Some(&user.login), redis.clone()).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}

async fn hx_like(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let (_, _, current_reaction) = get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;

        if current_reaction.as_deref() == Some("like") {
            // Toggle off — delete the like
            db.query("DELETE media_likes WHERE media_id = $mid AND user_login = $user")
                .bind(("mid", &mediumid))
                .bind(("user", &user.login))
                .await
                .expect("Database error");
        } else {
            // Upsert like using composite record ID
            let rec_id = format!("{}:{}", mediumid, user.login);
            db.query(
                "UPSERT media_likes SET media_id = $mid, user_login = $user, reaction = 'like'"
            )
            .bind(("mid", &mediumid))
            .bind(("user", &user.login))
            .await
            .expect("Database error");
        }

        let (likes, dislikes, user_reaction) = get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;
        let mut cache_redis = redis.clone();
        update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}

async fn hx_dislike(
    headers: HeaderMap,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let (_, _, current_reaction) = get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;

        if current_reaction.as_deref() == Some("dislike") {
            db.query("DELETE media_likes WHERE media_id = $mid AND user_login = $user")
                .bind(("mid", &mediumid))
                .bind(("user", &user.login))
                .await
                .expect("Database error");
        } else {
            db.query(
                "UPSERT media_likes SET media_id = $mid, user_login = $user, reaction = 'dislike'"
            )
            .bind(("mid", &mediumid))
            .bind(("user", &user.login))
            .await
            .expect("Database error");
        }

        let (likes, dislikes, user_reaction) = get_reaction_state_db(&db, &mediumid, Some(&user.login)).await;
        let mut cache_redis = redis.clone();
        update_reaction_cache(&mut cache_redis, &mediumid, likes, dislikes).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state_cached(&db, &mediumid, None, redis.clone()).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}
