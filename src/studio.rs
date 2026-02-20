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

#[derive(Serialize, Deserialize)]
struct MediumEdit {
    id: String,
    name: String,
    public: bool,
    medium_type: String,
}
#[derive(Template)]
#[template(path = "pages/studio-edit.html", escape = "none")]
struct StudioEditTemplate {
    sidebar: String,
    config: Config,
    medium: MediumEdit,
    common_headers: CommonHeaders,
}
async fn studio_edit(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let medium = sqlx::query!(
        "SELECT id,name,owner,public,type FROM media WHERE id=$1;",
        mediumid
    )
    .fetch_one(&pool)
    .await;

    match medium {
        Ok(record) => {
            if record.owner != user_info.login {
                return Html(minifi_html(
                    "<script>window.location.replace(\"/studio\");</script>".to_owned(),
                ));
            }
            let sidebar = generate_sidebar(&config, "studio".to_owned());
            let common_headers = extract_common_headers(&headers).unwrap();
            let template = StudioEditTemplate {
                sidebar,
                config,
                medium: MediumEdit {
                    id: record.id,
                    name: record.name,
                    public: record.public,
                    medium_type: record.r#type,
                },
                common_headers,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        Err(_) => Html(minifi_html(
            "<script>window.location.replace(\"/studio\");</script>".to_owned(),
        )),
    }
}

#[derive(Serialize, Deserialize)]
struct EditForm {
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
}
async fn studio_edit_save(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<EditForm>,
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
                return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
            }
        }
        Err(_) => {
            return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
        }
    }

    let ispublic = form.medium_visibility == "public";
    let description: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or(serde_json::Value::Null);

    let update_result = sqlx::query!(
        "UPDATE media SET name=$1, description=$2, public=$3 WHERE id=$4;",
        form.medium_name,
        description,
        ispublic,
        mediumid
    )
    .execute(&pool)
    .await;

    if update_result.is_err() {
        return Html(format!(
            "<b class=\"text-danger\">Failed to save changes.</b><script>setTimeout(function(){{window.location.replace(\"/studio/edit/{}\");}},2000);</script>",
            mediumid
        ));
    }

    Html(format!(
        "<script>window.location.replace(\"/studio/edit/{}\");</script>",
        mediumid
    ))
}

async fn hx_delete_video(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return ([("HX-Redirect", "/login")], "");
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return ([("HX-Redirect", "/studio")], "");
            }
        }
        Err(_) => {
            return ([("HX-Redirect", "/studio")], "");
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
        return ([("HX-Redirect", "/studio")], "");
    }

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    ([("HX-Redirect", "/studio")], "")
}
