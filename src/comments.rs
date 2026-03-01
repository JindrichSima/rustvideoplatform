#[derive(Serialize, Deserialize)]
struct Comment {
    id: String,
    user: String,
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
}

const COMMENTS_PER_PAGE: i64 = 20;

async fn hx_comments(
    Extension(db): Extension<Db>,
    Path(mediumid): Path<String>,
    axum::extract::Query(query): axum::extract::Query<CommentsQuery>,
) -> axum::response::Html<Vec<u8>> {
    let page = query.page.unwrap_or(0);
    let offset = page * COMMENTS_PER_PAGE;

    let mut result = db
        .query("SELECT record::id(id) AS id, user, text, time FROM comments WHERE media = $media ORDER BY time DESC LIMIT $limit START $offset")
        .bind(("media", &mediumid))
        .bind(("limit", COMMENTS_PER_PAGE + 1))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let comments: Vec<Comment> = result.take(0).expect("Database error");

    let has_more = comments.len() as i64 > COMMENTS_PER_PAGE;
    let comments: Vec<Comment> = comments.into_iter().take(COMMENTS_PER_PAGE as usize).collect();
    let next_page = if has_more { Some(page + 1) } else { None };

    let template = HXCommentsTemplate {
        comments,
        medium_id: mediumid,
        next_page,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Deserialize)]
struct CommentForm {
    text: String,
}

async fn comment_delta(
    Extension(db): Extension<Db>,
    Path(comment_id): Path<String>,
) -> Json<serde_json::Value> {
    #[derive(Deserialize)]
    struct TextRow { text: serde_json::Value }

    let mut result = db
        .query("SELECT text FROM type::thing('comments', $id)")
        .bind(("id", &comment_id))
        .await
        .expect("Database error");

    let row: Option<TextRow> = result.take(0).expect("Database error");
    Json(row.map(|r| r.text).unwrap_or_default())
}

#[derive(Template)]
#[template(path = "pages/hx-comment-single.html", escape = "none")]
struct HXCommentSingleTemplate {
    comment: Comment,
}

async fn hx_add_comment(
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

    let comment_id = generate_medium_id();
    let now = chrono::Utc::now().timestamp();

    db.query("CREATE type::thing('comments', $id) SET media = $media, user = $user, text = $text, time = $time")
        .bind(("id", &comment_id))
        .bind(("media", &mediumid))
        .bind(("user", &user.login))
        .bind(("text", &delta))
        .bind(("time", now))
        .await
        .expect("Database error");

    let comment = Comment {
        id: comment_id,
        user: user.login,
        text: delta,
        time: now,
    };

    let template = HXCommentSingleTemplate { comment };
    (StatusCode::OK, Html(minifi_html(template.render().unwrap())))
}
