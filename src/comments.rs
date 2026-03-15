#[derive(Serialize, Deserialize, SurrealValue)]
struct Comment {
    id: String,
    #[serde(rename = "author")]
    user: String,
    user_name: String,
    user_picture: Option<String>,
    text: serde_json::Value,
    time: i64,
}

#[derive(Deserialize, SurrealValue)]
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

#[derive(Deserialize, SurrealValue)]
struct CommentRow {
    id: RecordId,
    author: String,
    user_name: String,
    user_picture: Option<String>,
    text: serde_json::Value,
    time: i64,
}

async fn hx_comments(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Path(mediumid): Path<String>,
    axum::extract::Query(query): axum::extract::Query<CommentsQuery>,
) -> axum::response::Html<Vec<u8>> {
    let page = query.page.unwrap_or(0);
    let offset = page * COMMENTS_PER_PAGE;
    let limit = COMMENTS_PER_PAGE + 1;

    let mut resp = db
        .query(
            "SELECT id, author, author.name AS user_name, author.profile_picture AS user_picture, text, time \
             FROM comments WHERE media = $media \
             ORDER BY time DESC LIMIT $limit START $offset"
        )
        .bind(("media", RecordId::new("media", mediumid.as_str())))
        .bind(("limit", limit))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let rows: Vec<CommentRow> = resp.take(0).unwrap_or_default();

    let comments: Vec<Comment> = rows
        .into_iter()
        .map(|row| Comment {
            id: row.id.key_string(),
            user: row.author,
            user_name: row.user_name,
            user_picture: row.user_picture,
            text: row.text,
            time: row.time,
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

#[derive(Deserialize, SurrealValue)]
struct CommentForm {
    text: String,
}

async fn comment_delta(
    Extension(db): Extension<Db>,
    Path(comment_id): Path<String>,
) -> Json<serde_json::Value> {
    #[derive(Deserialize, SurrealValue)]
    struct TextRow { text: serde_json::Value }

    let mut resp = db
        .query("SELECT text FROM comments WHERE id = $id")
        .bind(("id", RecordId::new("comments", comment_id.as_str())))
        .await
        .expect("Database error");
    let row: Option<TextRow> = resp.take(0).expect("Database error");
    Json(row.map(|r| r.text).unwrap_or(serde_json::Value::Null))
}

#[derive(Template)]
#[template(path = "pages/hx-comment-single.html", escape = "none")]
struct HXCommentSingleTemplate {
    comment: Comment,
    config: Config,
}

async fn hx_add_comment(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CommentForm>,
) -> impl IntoResponse {
    let user = get_user_login(headers, &db, redis.clone()).await;

    if user.is_none() {
        return (StatusCode::UNAUTHORIZED, Html(Vec::new()));
    }

    let user = user.unwrap();

    let delta: serde_json::Value =
        serde_json::from_str(&form.text).unwrap_or_default();

    use chrono::Utc;
    let now_unix = Utc::now().timestamp();

    let mut resp = db
        .query(
            "CREATE comments SET \
             media = $media, author = $author, text = $text, time = $time; \
             SELECT id, author, author.name AS user_name, author.profile_picture AS user_picture, text, time \
             FROM comments ORDER BY time DESC LIMIT 1"
        )
        .bind(("media", RecordId::new("media", mediumid.as_str())))
        .bind(("author", RecordId::new("users", user.login.as_str())))
        .bind(("text", delta))
        .bind(("time", now_unix))
        .await
        .expect("Database error");

    let rows: Vec<CommentRow> = resp.take(1).unwrap_or_default();
    let row = match rows.into_iter().next() {
        Some(r) => r,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Html(Vec::new())),
    };

    let comment = Comment {
        id: row.id.key_string(),
        user: row.author,
        user_name: row.user_name,
        user_picture: row.user_picture,
        text: row.text,
        time: row.time,
    };

    let template = HXCommentSingleTemplate { comment, config };
    (StatusCode::OK, Html(minifi_html(template.render().unwrap())))
}
