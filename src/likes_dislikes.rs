fn like_dislike_html(mediumid: &str, likes: i64, dislikes: i64, user_reaction: Option<&str>) -> String {
    let like_color = if user_reaction == Some("like") { "var(--bs-success)" } else { "var(--bs-white)" };
    let dislike_color = if user_reaction == Some("dislike") { "var(--bs-danger)" } else { "var(--bs-white)" };
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" style="cursor: pointer; color: {like_color}" hx-get="/hx/like/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" style="cursor: pointer; color: {dislike_color}" hx-get="/hx/dislike/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

fn like_dislike_html_unauth(mediumid: &str, likes: i64, dislikes: i64) -> String {
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" style="color: var(--bs-success)" href="/login"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" style="color: var(--bs-warning)" href="/login"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

/// Get reaction counts and user's reaction using SurrealDB graph traversal
async fn get_reaction_state_db(db: &Db, mediumid: &str, user_login: Option<&str>) -> (i64, i64, Option<String>) {
    // Use graph traversal to count reactions on the media record
    #[derive(Deserialize)]
    struct CountsRow {
        likes: i64,
        dislikes: i64,
    }

    let mut result = db
        .query("SELECT count(<-reacts[WHERE reaction = 'like']) AS likes, count(<-reacts[WHERE reaction = 'dislike']) AS dislikes FROM type::thing('media', $media)")
        .bind(("media", mediumid))
        .await
        .expect("Database error");

    let counts: Option<CountsRow> = result.take(0).unwrap_or(None);
    let (likes, dislikes) = counts.map(|c| (c.likes, c.dislikes)).unwrap_or((0, 0));

    let user_reaction = if let Some(login) = user_login {
        #[derive(Deserialize)]
        struct ReactionRow { reaction: String }

        let mut result = db
            .query("SELECT reaction FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media) LIMIT 1")
            .bind(("user", login))
            .bind(("media", mediumid))
            .await
            .unwrap_or_else(|_| unreachable!());

        let row: Option<ReactionRow> = result.take(0).unwrap_or(None);
        row.map(|r| r.reaction)
    } else {
        None
    };

    (likes, dislikes, user_reaction)
}

/// Get reaction state using Redis cache for counts
async fn get_reaction_state_cached(db: &Db, mediumid: &str, user_login: Option<&str>, mut redis: RedisConn) -> (i64, i64, Option<String>) {
    let cached_likes: Result<i64, _> = redis.get(format!("cache:media:{}:likes", mediumid)).await;
    let cached_dislikes: Result<i64, _> = redis.get(format!("cache:media:{}:dislikes", mediumid)).await;

    let (likes, dislikes) = if let (Ok(l), Ok(d)) = (cached_likes, cached_dislikes) {
        (l, d)
    } else {
        #[derive(Deserialize)]
        struct CountsRow { likes: i64, dislikes: i64 }

        let mut result = db
            .query("SELECT count(<-reacts[WHERE reaction = 'like']) AS likes, count(<-reacts[WHERE reaction = 'dislike']) AS dislikes FROM type::thing('media', $media)")
            .bind(("media", mediumid))
            .await
            .expect("Database error");

        let counts: Option<CountsRow> = result.take(0).unwrap_or(None);
        counts.map(|c| (c.likes, c.dislikes)).unwrap_or((0, 0))
    };

    let user_reaction = if let Some(login) = user_login {
        #[derive(Deserialize)]
        struct ReactionRow { reaction: String }

        let mut result = db
            .query("SELECT reaction FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media) LIMIT 1")
            .bind(("user", login))
            .bind(("media", mediumid))
            .await
            .unwrap_or_else(|_| unreachable!());

        let row: Option<ReactionRow> = result.take(0).unwrap_or(None);
        row.map(|r| r.reaction)
    } else {
        None
    };

    (likes, dislikes, user_reaction)
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
            // Remove the like (toggle off)
            db.query("DELETE FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media)")
                .bind(("user", &user.login))
                .bind(("media", &mediumid))
                .await
                .expect("Database error");
        } else {
            // Upsert: delete existing reaction then create new one
            db.query("DELETE FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media); RELATE type::thing('users', $user) -> reacts -> type::thing('media', $media) SET reaction = 'like'")
                .bind(("user", &user.login))
                .bind(("media", &mediumid))
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
            db.query("DELETE FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media)")
                .bind(("user", &user.login))
                .bind(("media", &mediumid))
                .await
                .expect("Database error");
        } else {
            db.query("DELETE FROM reacts WHERE in = type::thing('users', $user) AND out = type::thing('media', $media); RELATE type::thing('users', $user) -> reacts -> type::thing('media', $media) SET reaction = 'dislike'")
                .bind(("user", &user.login))
                .bind(("media", &mediumid))
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
