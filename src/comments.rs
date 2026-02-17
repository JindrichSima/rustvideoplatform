#[derive(Serialize, Deserialize)]
struct Comment {
    id: i64,
    user: Option<String>,
    text: String,
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
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
    axum::extract::Query(query): axum::extract::Query<CommentsQuery>,
) -> axum::response::Html<Vec<u8>> {
    let page = query.page.unwrap_or(0);
    let offset = page * COMMENTS_PER_PAGE;

    let comments = sqlx::query_as!(
        Comment,
        "SELECT id,user,text,time FROM comments WHERE media=$1 ORDER BY time DESC LIMIT $2 OFFSET $3;",
        mediumid,
        COMMENTS_PER_PAGE + 1,
        offset
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");

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

#[derive(Template)]
#[template(path = "pages/hx-comment-single.html", escape = "none")]
struct HXCommentSingleTemplate {
    comment: Comment,
}

async fn hx_add_comment(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CommentForm>,
) -> impl IntoResponse {
    let user = get_user_login(headers, &pool, session_store).await;

    if user.is_none() {
        return (StatusCode::UNAUTHORIZED, Html(Vec::new()));
    }

    let user = user.unwrap();

    let comment = sqlx::query_as!(
        Comment,
        r#"INSERT INTO comments (media, "user", text) VALUES ($1, $2, $3) RETURNING id, "user", text, time;"#,
        mediumid,
        user.login,
        form.text
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let template = HXCommentSingleTemplate { comment };
    (StatusCode::OK, Html(minifi_html(template.render().unwrap())))
}
