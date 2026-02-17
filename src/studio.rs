#[derive(Template)]
#[template(path = "pages/studio.html", escape = "none")]
struct StudioTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn studio(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = StudioTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumStudio {
    id: String,
    name: String,
    description: Option<serde_json::Value>,
    views: i64,
    r#type: String,
}
#[derive(Template)]
#[template(path = "pages/hx-studio.html", escape = "none")]
struct HXStudioTemplate {
    media: Vec<MediumStudio>,
}
async fn hx_studio(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let media = sqlx::query_as!(
        MediumStudio,
        "SELECT id,name,description,views,type FROM media WHERE owner=$1 ORDER BY upload DESC;",
        user_info.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
    let template = HXStudioTemplate { media };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/studio-lists.html", escape = "none")]
struct StudioListsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn studio_lists(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, session_store).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = StudioListsTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-lists.html", escape = "none")]
struct HXStudioListsTemplate {
    lists: Vec<ListWithCount>,
}
async fn hx_studio_lists(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let lists = sqlx::query_as!(
        ListWithCount,
        "SELECT l.id, l.name, l.owner, l.public, (SELECT COUNT(*) FROM list_items li WHERE li.list_id = l.id) AS item_count FROM lists l WHERE l.owner = $1 ORDER BY l.created DESC;",
        user_info.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
    let template = HXStudioListsTemplate { lists };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_video(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                let result = "<b class=\"text-sucess\">MEDIA REMOVAL SUCESS</b><script>window.location.replace(\"/studio\");</script>".to_string();
                return Html(result);
            }
        }
        Err(_) => {
            let result = "<b class=\"text-sucess\">MEDIA REMOVAL FAILED</b><script>window.location.replace(\"/studio\");</script>".to_string();
            return Html(result);
        }
    }

    // Delete associated comments first
    let _ = sqlx::query!("DELETE FROM comments WHERE media=$1;", mediumid)
        .execute(&pool)
        .await;

    // Delete from any lists
    let _ = sqlx::query!("DELETE FROM list_items WHERE media_id=$1;", mediumid)
        .execute(&pool)
        .await;

    // Delete the video from database
    let delete_result = sqlx::query!("DELETE FROM media WHERE id=$1;", mediumid)
        .execute(&pool)
        .await;

    if delete_result.is_err() {
        let result = "<b class=\"text-sucess\">MEDIA REMOVAL FAILED</b><script>window.location.replace(\"/studio\");</script>".to_string();
        return Html(result);
    }

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    let result = "<b class=\"text-sucess\">MEDIA REMOVAL SUCESS</b><script>window.location.replace(\"/studio\");</script>".to_string();
    return Html(result);
}
