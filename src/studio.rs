#[derive(Template)]
#[template(path = "pages/studio.html", escape = "none")]
struct StudioTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    active_tab: String,
}
async fn studio(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
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
        active_tab: "media".to_owned(),
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
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, db, redis, headers, 0).await
}

async fn hx_studio_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_inner(config, db, redis, headers, page).await
}

async fn hx_studio_inner(
    config: Config,
    db: Db,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let offset = page * 40;

    let mut result = db
        .query("SELECT record::id(id) AS id, name, description, views, type FROM media WHERE owner = $owner ORDER BY upload DESC LIMIT 41 START $offset")
        .bind(("owner", &user_info.login))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut media: Vec<MediumStudio> = result.take(0).expect("Database error");

    let has_more = media.len() == 41;
    if has_more {
        media.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/studio/{}", next_page);

    let template = HXStudioTemplate { media, config, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

async fn studio_lists(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &db, redis.clone()).await).await {
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
        active_tab: "lists".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-lists.html", escape = "none")]
struct HXStudioListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
}
async fn hx_studio_lists(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(db, redis, headers, 0).await
}

async fn hx_studio_lists_page(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(page): Path<i64>,
) -> axum::response::Html<Vec<u8>> {
    hx_studio_lists_inner(db, redis, headers, page).await
}

async fn hx_studio_lists_inner(
    db: Db,
    redis: RedisConn,
    headers: HeaderMap,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();
    let offset = page * 40;

    // Use graph traversal to count items per list
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, visibility, restricted_to_group, count(->list_contains) AS item_count FROM lists WHERE owner = $owner ORDER BY created DESC LIMIT 41 START $offset")
        .bind(("owner", &user_info.login))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut lists: Vec<ListWithCount> = result.take(0).expect("Database error");

    let has_more = lists.len() == 41;
    if has_more {
        lists.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/studio/lists/{}", next_page);

    let template = HXStudioListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct MediumEdit {
    id: String,
    name: String,
    visibility: String,
    restricted_to_group: String,
    medium_type: String,
}
#[derive(Template)]
#[template(path = "pages/studio-edit.html", escape = "none")]
struct StudioEditTemplate {
    sidebar: String,
    config: Config,
    medium: MediumEdit,
    common_headers: CommonHeaders,
    owner_groups: Vec<UserGroup>,
}
async fn studio_edit(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    #[derive(Deserialize)]
    struct EditRow {
        id: String,
        name: String,
        owner: String,
        visibility: String,
        restricted_to_group: Option<String>,
        r#type: String,
    }

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, visibility, restricted_to_group, type FROM type::thing('media', $id)")
        .bind(("id", &mediumid))
        .await
        .expect("Database error");

    let record: Option<EditRow> = result.take(0).expect("Database error");

    match record {
        Some(record) => {
            if record.owner != user_info.login {
                return Html(minifi_html(
                    "<script>window.location.replace(\"/studio\");</script>".to_owned(),
                ));
            }

            let mut grp_result = db
                .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
                .bind(("owner", &user_info.login))
                .await
                .unwrap_or_else(|_| unreachable!());

            let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

            let sidebar = generate_sidebar(&config, "studio".to_owned());
            let common_headers = extract_common_headers(&headers).unwrap();
            let template = StudioEditTemplate {
                sidebar,
                config,
                medium: MediumEdit {
                    id: record.id,
                    name: record.name,
                    visibility: record.visibility,
                    restricted_to_group: record.restricted_to_group.unwrap_or_default(),
                    medium_type: record.r#type,
                },
                common_headers,
                owner_groups,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html(
            "<script>window.location.replace(\"/studio\");</script>".to_owned(),
        )),
    }
}

#[derive(Serialize, Deserialize)]
struct EditForm {
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}
async fn studio_edit_save(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<EditForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let media_owner = get_media_owner(&db, &mediumid).await;
    match media_owner {
        Some(owner) => {
            if owner != user_info.login {
                return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
            }
        }
        None => {
            return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
        }
    }

    let visibility = match form.medium_visibility.as_str() {
        "public" | "hidden" | "restricted" => form.medium_visibility.clone(),
        _ => "hidden".to_owned(),
    };
    let restricted_to_group = if visibility == "restricted" {
        form.medium_restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };
    let description: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or(serde_json::Value::Null);

    let update_result = db
        .query("UPDATE type::thing('media', $id) SET name = $name, description = $desc, visibility = $vis, restricted_to_group = $group")
        .bind(("id", &mediumid))
        .bind(("name", &form.medium_name))
        .bind(("desc", &description))
        .bind(("vis", &visibility))
        .bind(("group", &restricted_to_group))
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
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return ([("HX-Redirect", "/login")], "");
    }
    let user_info = user_info.unwrap();

    let media_owner = get_media_owner(&db, &mediumid).await;
    match media_owner {
        Some(owner) => {
            if owner != user_info.login {
                return ([("HX-Redirect", "/studio")], "");
            }
        }
        None => {
            return ([("HX-Redirect", "/studio")], "");
        }
    }

    // Delete associated data: comments, list edges, reaction edges, then the media record
    let _ = db
        .query("DELETE FROM comments WHERE media = $id; DELETE FROM list_contains WHERE out = type::thing('media', $id); DELETE FROM reacts WHERE out = type::thing('media', $id); DELETE type::thing('media', $id)")
        .bind(("id", &mediumid))
        .await;

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    ([("Redirect", "/studio")], "")
}
