#[derive(Serialize, Deserialize, Clone)]
struct List {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ListWithCount {
    id: String,
    name: String,
    owner: String,
    visibility: String,
    restricted_to_group: Option<String>,
    item_count: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct ListModalEntry {
    id: String,
    name: String,
    already_added: bool,
}

#[derive(Deserialize)]
struct CreateListForm {
    name: String,
    visibility: Option<String>,
    restricted_group: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/list.html", escape = "none")]
struct ListPageTemplate {
    sidebar: String,
    config: Config,
    list: List,
    is_owner: bool,
    common_headers: CommonHeaders,
}

#[derive(Template)]
#[template(path = "pages/hx-listitems.html", escape = "none")]
struct HXListItemsTemplate {
    items: Vec<Medium>,
    list_id: String,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}

#[derive(Template)]
#[template(path = "pages/hx-listmodal.html", escape = "none")]
struct HXListModalTemplate {
    lists: Vec<ListModalEntry>,
    medium_id: String,
    owner_groups: Vec<UserGroup>,
}

#[derive(Template)]
#[template(path = "pages/hx-userlists.html", escape = "none")]
struct HXUserListsTemplate {
    lists: Vec<ListWithCount>,
    page: i64,
    has_more: bool,
    next_url: String,
}

async fn list_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await
        .expect("Database error");

    let list: Option<List> = resp.take(0).expect("Deserialize error");
    let list = match list {
        Some(l) => l,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_owner = user_info
        .as_ref()
        .map(|u| u.login == list.owner)
        .unwrap_or(false);

    // Access control for restricted lists
    if !is_owner && !can_access_restricted(&db, &list.visibility, list.restricted_to_group.as_deref(), &list.owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers).unwrap();
    let sidebar = generate_sidebar(&config, "list".to_owned());
    let template = ListPageTemplate {
        sidebar,
        config,
        list,
        is_owner,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Deserialize)]
struct MediaWithOwner {
    id: String,
    name: String,
    description: String,
    upload: i64,
    owner: String,
    owner_name: String,
    owner_picture: Option<String>,
    views: i64,
    #[serde(rename = "type")]
    media_type: String,
    visibility: String,
    restricted_to_group: Option<String>,
}

async fn medium_in_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let mut resp = db
        .query("SELECT id, visibility, restricted_to_group, owner, name FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct ListBasic {
        id: String,
        visibility: String,
        restricted_to_group: Option<String>,
        owner: String,
        name: String,
    }

    let list: Option<ListBasic> = resp.take(0).expect("Deserialize error");
    let list = match list {
        Some(l) => l,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_logged_in = user_info.is_some();

    // Access control for restricted lists
    let is_owner = user_info.as_ref().map(|u| u.login == list.owner).unwrap_or(false);
    if !is_owner && !can_access_restricted(&db, &list.visibility, list.restricted_to_group.as_deref(), &list.owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers).unwrap();

    // Fetch media record
    let mut media_resp = db
        .query("SELECT id, name, description, upload, owner, views, type, visibility, restricted_to_group FROM media WHERE id = $id")
        .bind(("id", mediumid.to_ascii_lowercase()))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct MediaBasic {
        id: String,
        name: String,
        description: String,
        upload: i64,
        owner: String,
        views: i64,
        #[serde(rename = "type")]
        media_type: String,
        visibility: String,
        restricted_to_group: Option<String>,
    }

    let medium: Option<MediaBasic> = media_resp.take(0).expect("Deserialize error");
    let medium = match medium {
        Some(m) => m,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    if !can_access_restricted(&db, &medium.visibility, medium.restricted_to_group.as_deref(), &medium.owner, &user_info, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    // Fetch owner info
    let mut owner_resp = db
        .query("SELECT name, profile_picture FROM users WHERE id = $id")
        .bind(("id", surrealdb::RecordId::from_table_key("users", &medium.owner)))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct OwnerInfo {
        name: String,
        profile_picture: Option<String>,
    }

    let owner_info: Option<OwnerInfo> = owner_resp.take(0).unwrap_or(None);
    let (owner_name, owner_picture) = owner_info
        .map(|o| (o.name, o.profile_picture))
        .unwrap_or_else(|| (medium.owner.clone(), None));

    let medium_id = medium.id.clone();
    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<CaptionEntry> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium_id)).exists() {
        medium_captions_exist = true;
        for entry in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium_id)) {
            if !entry.trim().is_empty() {
                medium_captions_list.push(parse_caption_entry(&entry));
            }
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_has_ass_captions = medium_captions_list.iter().any(|c| c.is_ass);
    let medium_custom_font =
        std::path::Path::new(&format!("source/{}/captions/font.woff2", medium_id)).exists();

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", medium_id)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium_id)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let is_cmaf: bool;
    if std::path::Path::new(&format!("source/{}/video/video.m3u8", medium_id)).exists() {
        is_cmaf = true;
    } else {
        is_cmaf = false;
    }

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id,
        medium_name: medium.name,
        medium_owner: medium.owner,
        medium_owner_name: owner_name,
        medium_owner_picture: owner_picture,
        medium_upload: prettyunixtime(medium.upload).await,
        medium_views: medium.views,
        medium_type: medium.media_type,
        medium_captions_exist,
        medium_captions_list,
        medium_has_ass_captions,
        medium_custom_font,
        medium_chapters_exist,
        medium_previews_exist,
        is_cmaf,
        config,
        common_headers,
        is_logged_in,
        list_id: listid,
        list_name: list.name,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_items(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Path(listid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, db, listid, 0).await
}

async fn hx_list_items_page(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Path((listid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_list_items_inner(config, db, listid, page).await
}

async fn hx_list_items_inner(
    config: Config,
    db: Db,
    listid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let offset = page * 40;

    #[derive(Deserialize)]
    struct ListItemMedia {
        id: String,
        name: String,
        owner: String,
        views: i64,
        #[serde(rename = "type")]
        media_type: String,
    }

    let mut resp = db
        .query("SELECT media_id.id AS id, media_id.name AS name, media_id.owner AS owner, media_id.views AS views, media_id.type AS type FROM list_items WHERE list_id = $lid ORDER BY position ASC LIMIT 41 START $offset")
        .bind(("lid", listid.clone()))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let raw: Vec<ListItemMedia> = resp.take(0).expect("Deserialize error");
    let mut items: Vec<Medium> = raw.into_iter().map(|r| Medium {
        id: r.id,
        name: r.name,
        owner: r.owner,
        views: r.views,
        r#type: r.media_type,
        sprite_filename: None,
        sprite_x: 0,
        sprite_y: 0,
    }).collect();

    let has_more = items.len() == 41;
    if has_more {
        items.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/listitems/{}/{}", listid, next_page);

    let template = HXListItemsTemplate {
        items,
        list_id: listid,
        config,
        page,
        has_more,
        next_url,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_sidebar(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    #[derive(Deserialize)]
    struct ListItemMedia {
        id: String,
        name: String,
        owner: String,
        views: i64,
        #[serde(rename = "type")]
        media_type: String,
    }

    let mut resp = db
        .query("SELECT media_id.id AS id, media_id.name AS name, media_id.owner AS owner, media_id.views AS views, media_id.type AS type FROM list_items WHERE list_id = $lid ORDER BY position ASC")
        .bind(("lid", listid.clone()))
        .await
        .expect("Database error");

    let raw: Vec<ListItemMedia> = resp.take(0).expect("Deserialize error");
    let media: Vec<Medium> = raw.into_iter().map(|r| Medium {
        id: r.id,
        name: r.name,
        owner: r.owner,
        views: r.views,
        r#type: r.media_type,
        sprite_filename: None,
        sprite_x: 0,
        sprite_y: 0,
    }).collect();

    let template = HXMediumListTemplate {
        media,
        current_medium_id: mediumid,
        list_id: listid,
        config,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_list_modal(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let lists = fetch_list_modal_entries(&db, &user_info.login, &mediumid).await;
    let owner_groups = fetch_owner_groups(&db, &user_info.login).await;

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn fetch_list_modal_entries(db: &Db, owner: &str, mediumid: &str) -> Vec<ListModalEntry> {
    #[derive(Deserialize)]
    struct ListRow {
        id: String,
        name: String,
        already_added: bool,
    }

    let mut resp = db
        .query("SELECT id, name, (SELECT count() FROM list_items WHERE list_id = $parent.id AND media_id = $mid GROUP ALL)[0].count > 0 AS already_added FROM lists WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", owner.to_string()))
        .bind(("mid", mediumid.to_string()))
        .await
        .expect("Database error");

    let rows: Vec<ListRow> = resp.take(0).unwrap_or_default();
    rows.into_iter().map(|r| ListModalEntry {
        id: r.id,
        name: r.name,
        already_added: r.already_added,
    }).collect()
}

async fn fetch_owner_groups(db: &Db, owner: &str) -> Vec<UserGroup> {
    let mut groups = system_groups_for_owner(owner);

    let mut resp = db
        .query("SELECT id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", owner.to_string()))
        .await
        .expect("Database error");

    let user_groups: Vec<UserGroup> = resp.take(0).unwrap_or_default();
    groups.extend(user_groups);
    groups
}

async fn hx_create_list(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Form(form): Form<CreateListForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let list_id = generate_medium_id();
    let visibility = match form.visibility.as_deref() {
        Some("public") => "public",
        Some("restricted") => "restricted",
        _ => "hidden",
    };
    let is_public = visibility == "public";
    let restricted_to_group = if visibility == "restricted" {
        form.restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    let _ = db
        .query("CREATE type::thing('lists', $lid) SET name = $name, owner = $owner, public = $public, visibility = $visibility, restricted_to_group = $rtg, created = time::unix(time::now())")
        .bind(("lid", list_id.clone()))
        .bind(("name", form.name.clone()))
        .bind(("owner", user_info.login.clone()))
        .bind(("public", is_public))
        .bind(("visibility", visibility.to_string()))
        .bind(("rtg", restricted_to_group.clone()))
        .await
        .expect("Database error");

    let _ = db
        .query("CREATE list_items SET list_id = $lid, media_id = $mid, position = 0")
        .bind(("lid", list_id.clone()))
        .bind(("mid", mediumid.clone()))
        .await
        .expect("Database error");

    let lists = fetch_list_modal_entries(&db, &user_info.login, &mediumid).await;
    let owner_groups = fetch_owner_groups(&db, &user_info.login).await;

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_to_list(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let mut owner_resp = db
        .query("SELECT owner FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct OwnerRow { owner: String }
    let owner_row: Option<OwnerRow> = owner_resp.take(0).expect("Deserialize error");
    match owner_row {
        Some(r) if r.owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Get max position
    let mut pos_resp = db
        .query("SELECT VALUE math::max(position) FROM list_items WHERE list_id = $lid")
        .bind(("lid", listid.clone()))
        .await
        .expect("Database error");

    let max_pos: Option<i64> = pos_resp.take(0).unwrap_or(None);
    let next_pos = max_pos.unwrap_or(-1) + 1;

    let _ = db
        .query("CREATE list_items SET list_id = $lid, media_id = $mid, position = $pos")
        .bind(("lid", listid.clone()))
        .bind(("mid", mediumid.clone()))
        .bind(("pos", next_pos))
        .await
        .expect("Database error");

    let lists = fetch_list_modal_entries(&db, &user_info.login, &mediumid).await;
    let owner_groups = fetch_owner_groups(&db, &user_info.login).await;

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_from_list(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let mut owner_resp = db
        .query("SELECT owner FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct OwnerRow { owner: String }
    let owner_row: Option<OwnerRow> = owner_resp.take(0).expect("Deserialize error");
    match owner_row {
        Some(r) if r.owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    let _ = db
        .query("DELETE FROM list_items WHERE list_id = $lid AND media_id = $mid")
        .bind(("lid", listid.clone()))
        .bind(("mid", mediumid.clone()))
        .await
        .expect("Database error");

    let lists = fetch_list_modal_entries(&db, &user_info.login, &mediumid).await;
    let owner_groups = fetch_owner_groups(&db, &user_info.login).await;

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_list(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(listid): Path<String>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let mut owner_resp = db
        .query("SELECT owner FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await
        .expect("Database error");

    #[derive(Deserialize)]
    struct OwnerRow { owner: String }
    let owner_row: Option<OwnerRow> = owner_resp.take(0).expect("Deserialize error");
    match owner_row {
        Some(r) if r.owner == user_info.login => {}
        _ => return Html("<script>window.location.replace(\"/\");</script>".to_owned()),
    }

    let _ = db
        .query("DELETE FROM list_items WHERE list_id = $lid")
        .bind(("lid", listid.clone()))
        .await;

    let _ = db
        .query("DELETE FROM lists WHERE id = $id")
        .bind(("id", listid.clone()))
        .await;

    Html("<b class=\"text-success\">LIST DELETED</b><script>window.location.replace(\"/\");</script>".to_owned())
}

async fn hx_user_lists(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(userid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(db, redis, headers, userid, 0).await
}

async fn hx_user_lists_page(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((userid, page)): Path<(String, i64)>,
) -> axum::response::Html<Vec<u8>> {
    hx_user_lists_inner(db, redis, headers, userid, page).await
}

async fn hx_user_lists_inner(
    db: Db,
    redis: RedisConn,
    headers: HeaderMap,
    userid: String,
    page: i64,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let offset = page * 40;

    let mut resp = db
        .query("SELECT id, name, owner, visibility, restricted_to_group, (SELECT count() FROM list_items WHERE list_id = $parent.id GROUP ALL)[0].count AS item_count FROM lists WHERE owner = $owner AND fn::visible_to(visibility, restricted_to_group, owner, $viewer) ORDER BY created DESC LIMIT 41 START $offset")
        .bind(("owner", userid.clone()))
        .bind(("viewer", user_login.clone()))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut lists: Vec<ListWithCount> = resp.take(0).unwrap_or_default();

    let has_more = lists.len() == 41;
    if has_more {
        lists.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/userlists/{}/{}", userid, next_page);

    let template = HXUserListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
