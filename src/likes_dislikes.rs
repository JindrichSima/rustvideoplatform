fn like_dislike_html(mediumid: &str, likes: i64, dislikes: i64, user_reaction: Option<&str>) -> String {
    let like_color = if user_reaction == Some("like") { "#ffd700" } else { "white" };
    let dislike_color = if user_reaction == Some("dislike") { "#ffd700" } else { "white" };
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none" style="cursor: pointer; color: {like_color}" hx-get="/hx/like/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none" style="cursor: pointer; color: {dislike_color}" hx-get="/hx/dislike/{mediumid}" hx-swap="outerHTML" hx-target="closest li"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

fn like_dislike_html_unauth(mediumid: &str, likes: i64, dislikes: i64) -> String {
    format!(
        r#"<li class="d-flex align-items-center mx-2"><a class="text-decoration-none text-white" href="/login"><i class="fa-solid fa-thumbs-up fa-xl"></i>&nbsp;<b>{likes}</b></a><span class="text-white mx-2">|</span><a class="text-decoration-none text-white" href="/login"><i class="fa-solid fa-thumbs-down fa-xl"></i>&nbsp;<b>{dislikes}</b></a></li>"#
    )
}

async fn get_reaction_state(pool: &PgPool, mediumid: &str, user_login: Option<&str>) -> (i64, i64, Option<String>) {
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

async fn hx_likedislikebutton(
    headers: HeaderMap,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    if let Some(user) = get_user_login(headers, &pool, redis.clone()).await {
        let (likes, dislikes, user_reaction) = get_reaction_state(&pool, &mediumid, Some(&user.login)).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state(&pool, &mediumid, None).await;
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
        let (_, _, current_reaction) = get_reaction_state(&pool, &mediumid, Some(&user.login)).await;

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

        let (likes, dislikes, user_reaction) = get_reaction_state(&pool, &mediumid, Some(&user.login)).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state(&pool, &mediumid, None).await;
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
        let (_, _, current_reaction) = get_reaction_state(&pool, &mediumid, Some(&user.login)).await;

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

        let (likes, dislikes, user_reaction) = get_reaction_state(&pool, &mediumid, Some(&user.login)).await;
        Html(like_dislike_html(&mediumid, likes, dislikes, user_reaction.as_deref()))
    } else {
        let (likes, dislikes, _) = get_reaction_state(&pool, &mediumid, None).await;
        Html(like_dislike_html_unauth(&mediumid, likes, dislikes))
    }
}
