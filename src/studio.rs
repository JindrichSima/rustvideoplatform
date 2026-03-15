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

#[derive(Serialize, Deserialize, SurrealValue)]
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

    let mut resp = db
        .query("SELECT id, name, description, views, type FROM media WHERE owner = $owner ORDER BY upload DESC LIMIT 41 START $offset;")
        .bind(("owner", user_info.login.clone()))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut media: Vec<MediumStudio> = resp.take(0).unwrap_or_default();

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

    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group, array::len((SELECT id FROM list_items WHERE list_id = $parent.id)) AS item_count FROM lists WHERE owner = $owner ORDER BY created DESC LIMIT 41 START $offset;")
        .bind(("owner", user_info.login.clone()))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut lists: Vec<ListWithCount> = resp.take(0).unwrap_or_default();

    let has_more = lists.len() == 41;
    if has_more {
        lists.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/studio/lists/{}", next_page);

    let template = HXStudioListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize, SurrealValue)]
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
    active_tab: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-description.html", escape = "none")]
struct HXStudioEditDescriptionTemplate {
    medium: MediumEdit,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-chapters.html", escape = "none")]
struct HXStudioEditChaptersTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-subtitles.html", escape = "none")]
struct HXStudioEditSubtitlesTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-thumbnail.html", escape = "none")]
struct HXStudioEditThumbnailTemplate {
    medium_id: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-danger.html", escape = "none")]
struct HXStudioEditDangerTemplate {
    medium_id: String,
    medium_name: String,
}

#[derive(Template)]
#[template(path = "pages/hx-studio-edit-permissions.html", escape = "none")]
struct HXStudioEditPermissionsTemplate {
    medium: MediumEdit,
    owner_groups: Vec<UserGroup>,
}

#[derive(Deserialize, SurrealValue)]
struct MediaRecord {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
    #[serde(rename = "type")]
    r#type: String,
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

    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group, type FROM media WHERE id = $id;")
        .bind(("id", mediumid.clone()))
        .await
        .expect("Database error");

    let record: Option<MediaRecord> = resp.take(0).unwrap_or_default();

    match record {
        Some(record) => {
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
                    visibility: record.visibility,
                    restricted_to_group: record.restricted_to_group.unwrap_or_default(),
                    medium_type: record.r#type,
                },
                common_headers,
                active_tab: "description".to_owned(),
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html(
            "<script>window.location.replace(\"/studio\");</script>".to_owned(),
        )),
    }
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct EditForm {
    medium_name: String,
    medium_description: String,
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct PermissionsEditForm {
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}

#[derive(Deserialize, SurrealValue)]
struct OwnerRecord {
    owner: String,
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

    let description: serde_json::Value =
        serde_json::from_str(&form.medium_description).unwrap_or(serde_json::Value::Null);

    // Ownership check + update in one query: returns nothing if id/owner don't match
    let mut update_resp = db
        .query("UPDATE media SET name = $name, description = $description WHERE id = $id AND owner = $owner RETURN id")
        .bind(("name", form.medium_name.clone()))
        .bind(("description", description.clone()))
        .bind(("id", mediumid.clone()))
        .bind(("owner", user_info.login.clone()))
        .await;

    let updated: Vec<serde_json::Value> = update_resp.as_mut().ok()
        .and_then(|r| r.take(0).ok()).unwrap_or_default();
    if updated.is_empty() {
        return Html("<script>window.location.replace(\"/studio\");</script>".to_owned());
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
        return Redirect::to("/login");
    }
    let user_info = user_info.unwrap();

    // Ownership check + delete in one query; returns nothing if not owner
    let mut del_resp = db
        .query("DELETE FROM media WHERE id = $id AND owner = $owner RETURN id")
        .bind(("id", mediumid.clone()))
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    let deleted: Vec<serde_json::Value> = del_resp.take(0).unwrap_or_default();
    if deleted.is_empty() {
        return Redirect::to("/studio");
    }

    // Media deleted — clean up related records in a single round-trip
    let _ = db
        .query("DELETE FROM comments WHERE media = $id; DELETE FROM list_items WHERE media_id = $id")
        .bind(("id", mediumid.clone()))
        .await;

    // Delete the source directory
    let source_path = format!("source/{}", mediumid);
    let _ = fs::remove_dir_all(&source_path).await;

    Redirect::to("/studio")
}

async fn hx_studio_edit_description(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group, type FROM media WHERE id = $id;")
        .bind(("id", mediumid.clone()))
        .await
        .expect("Database error");

    let record: Option<MediaRecord> = resp.take(0).unwrap_or_default();

    match record {
        Some(record) => {
            if record.owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }

            let template = HXStudioEditDescriptionTemplate {
                medium: MediumEdit {
                    id: record.id,
                    name: record.name,
                    visibility: record.visibility,
                    restricted_to_group: record.restricted_to_group.unwrap_or_default(),
                    medium_type: record.r#type,
                },
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn hx_studio_edit_chapters_tab(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Single WHERE clause confirms ownership without a separate read
    let mut owns_resp = db
        .query("SELECT id FROM media WHERE id = $id AND owner = $owner")
        .bind(("id", mediumid.clone()))
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");
    if owns_resp.take::<Vec<serde_json::Value>>(0).unwrap_or_default().is_empty() {
        return Html(minifi_html("".to_owned()));
    }

    let template = HXStudioEditChaptersTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_subtitles_tab(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Single WHERE clause confirms ownership without a separate read
    let mut owns_resp = db
        .query("SELECT id FROM media WHERE id = $id AND owner = $owner")
        .bind(("id", mediumid.clone()))
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");
    if owns_resp.take::<Vec<serde_json::Value>>(0).unwrap_or_default().is_empty() {
        return Html(minifi_html("".to_owned()));
    }

    let template = HXStudioEditSubtitlesTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_thumbnail_tab(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Single WHERE clause confirms ownership without a separate read
    let mut owns_resp = db
        .query("SELECT id FROM media WHERE id = $id AND owner = $owner")
        .bind(("id", mediumid.clone()))
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");
    if owns_resp.take::<Vec<serde_json::Value>>(0).unwrap_or_default().is_empty() {
        return Html(minifi_html("".to_owned()));
    }

    let template = HXStudioEditThumbnailTemplate { medium_id: mediumid };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_studio_edit_danger_tab(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    #[derive(Deserialize, SurrealValue)]
    struct OwnerNameRecord {
        owner: String,
        name: String,
    }

    let mut resp = db
        .query("SELECT owner, name FROM media WHERE id = $id;")
        .bind(("id", mediumid.clone()))
        .await
        .expect("Database error");

    let row: Option<OwnerNameRecord> = resp.take(0).unwrap_or_default();

    match row {
        Some(record) => {
            if record.owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }
            let medium_name = record.name;
            let template = HXStudioEditDangerTemplate { medium_id: mediumid, medium_name };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn hx_studio_edit_permissions_tab(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("".to_owned()));
    }
    let user_info = user_info.unwrap();

    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group, type FROM media WHERE id = $id;")
        .bind(("id", mediumid.clone()))
        .await
        .expect("Database error");

    let record: Option<MediaRecord> = resp.take(0).unwrap_or_default();

    match record {
        Some(record) => {
            if record.owner != user_info.login {
                return Html(minifi_html("".to_owned()));
            }

            let mut owner_groups = system_groups_for_owner(&user_info.login);

            let mut groups_resp = db
                .query("SELECT id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
                .bind(("owner", user_info.login.clone()))
                .await
                .expect("Database error");

            let user_groups: Vec<UserGroup> = groups_resp.take(0).unwrap_or_default();
            owner_groups.extend(user_groups);

            let template = HXStudioEditPermissionsTemplate {
                medium: MediumEdit {
                    id: record.id,
                    name: record.name,
                    visibility: record.visibility,
                    restricted_to_group: record.restricted_to_group.unwrap_or_default(),
                    medium_type: record.r#type,
                },
                owner_groups,
            };
            Html(minifi_html(template.render().unwrap()))
        }
        None => Html(minifi_html("".to_owned())),
    }
}

async fn studio_edit_permissions_save(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<PermissionsEditForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let visibility = match form.medium_visibility.as_str() {
        "public" | "hidden" | "restricted" => form.medium_visibility.clone(),
        _ => "hidden".to_owned(),
    };
    let ispublic = visibility == "public";
    let restricted_to_group = if visibility == "restricted" {
        form.medium_restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    let update_result = db
        .query("UPDATE media SET public = $public, visibility = $visibility, restricted_to_group = $restricted_to_group WHERE id = $id;")
        .bind(("public", ispublic))
        .bind(("visibility", visibility.clone()))
        .bind(("restricted_to_group", restricted_to_group.clone()))
        .bind(("id", mediumid.clone()))
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
