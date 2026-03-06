#[derive(Serialize, Deserialize)]
struct Comment {
    id: i64,
    user: String,
    user_name: String,
    user_picture: Option<String>,
    text: serde_json::Value,
    time: i64,
}

#[derive(Deserialize)]
struct CommentsQuery {
    page: Option<i64>,
}

#[derive(Template)]
#[template(path = "pages/hx-comments.html", escape = "none")]
struct HXCommentsTemplate {
    comments: Vec<Comment>,
    medium_id: String,
    next_page: Option<i64>,
    config: Config,
}

const COMMENTS_PER_PAGE: i64 = 20;

async fn hx_comments(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
    axum::extract::Query(query): axum::extract::Query<CommentsQuery>,
) -> axum::response::Html<Vec<u8>> {
    let page = query.page.unwrap_or(0);
    let offset = page * COMMENTS_PER_PAGE;

    let rows = sqlx::query(
        r#"SELECT c.id, c."user", u.name as user_name, u.profile_picture as user_picture, c.text, c.time
           FROM comments c
           LEFT JOIN users u ON c."user" = u.login
           WHERE c.media=$1 ORDER BY c.time DESC LIMIT $2 OFFSET $3;"#,
    )
    .bind(&mediumid)
    .bind(COMMENTS_PER_PAGE + 1)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .expect("Database error");

    use sqlx::Row;
    let comments: Vec<Comment> = rows
        .iter()
        .map(|row| Comment {
            id: row.get("id"),
            user: row.get("user"),
            user_name: row.get("user_name"),
            user_picture: row.get("user_picture"),
            text: row.get("text"),
            time: row.get("time"),
        })
        .collect();

    let has_more = comments.len() as i64 > COMMENTS_PER_PAGE;
    let comments: Vec<Comment> = comments.into_iter().take(COMMENTS_PER_PAGE as usize).collect();
    let next_page = if has_more { Some(page + 1) } else { None };

    let template = HXCommentsTemplate {
        comments,
        medium_id: mediumid,
        next_page,
        config,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Deserialize)]
struct CommentForm {
    text: String,
}

async fn comment_delta(
    Extension(pool): Extension<PgPool>,
    Path(comment_id): Path<i64>,
) -> Json<serde_json::Value> {
    let row = sqlx::query!(
        r#"SELECT text FROM comments WHERE id=$1;"#,
        comment_id
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    Json(row.text)
}

#[derive(Template)]
#[template(path = "pages/hx-comment-single.html", escape = "none")]
struct HXCommentSingleTemplate {
    comment: Comment,
    config: Config,
}

async fn hx_add_comment(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CommentForm>,
) -> impl IntoResponse {
    let user = get_user_login(headers, &pool, redis.clone()).await;

    if user.is_none() {
        return (StatusCode::UNAUTHORIZED, Html(Vec::new()));
    }

    let user = user.unwrap();

    let delta: serde_json::Value =
        serde_json::from_str(&form.text).unwrap_or_default();

    let row = sqlx::query(
        r#"WITH inserted AS (
            INSERT INTO comments (media, "user", text) VALUES ($1, $2, $3) RETURNING id, "user", text, time
        )
        SELECT i.id, i."user", u.name as user_name, u.profile_picture as user_picture, i.text, i.time
        FROM inserted i
        LEFT JOIN users u ON i."user" = u.login;"#,
    )
    .bind(&mediumid)
    .bind(&user.login)
    .bind(delta)
    .fetch_one(&pool)
    .await
    .expect("Database error");

    use sqlx::Row;
    let comment = Comment {
        id: row.get("id"),
        user: row.get("user"),
        user_name: row.get("user_name"),
        user_picture: row.get("user_picture"),
        text: row.get("text"),
        time: row.get("time"),
    };

    let template = HXCommentSingleTemplate { comment, config };
    (StatusCode::OK, Html(minifi_html(template.render().unwrap())))
}
