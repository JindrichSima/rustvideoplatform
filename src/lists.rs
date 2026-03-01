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
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, visibility, restricted_to_group FROM type::thing('lists', $id)")
        .bind(("id", &listid))
        .await
        .expect("Database error");

    let list: Option<List> = result.take(0).expect("Database error");

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

    let sidebar = generate_sidebar(&config, "list".to_owned());
    let template = ListPageTemplate {
        sidebar,
        config,
        list,
        is_owner,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn medium_in_list(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((listid, mediumid)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, visibility, restricted_to_group FROM type::thing('lists', $id)")
        .bind(("id", &listid))
        .await
        .expect("Database error");

    let list: Option<List> = result.take(0).expect("Database error");

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

    // Also check media access
    let mut med_result = db
        .query("SELECT record::id(id) AS id, name, description, upload, owner, views, type, visibility, restricted_to_group FROM type::thing('media', $id)")
        .bind(("id", mediumid.to_ascii_lowercase()))
        .await
        .expect("Database error");

    let medium: Option<MediaRow> = med_result.take(0).expect("Database error");

    let medium = match medium {
        Some(row) => row,
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

    let medium_id = medium.id.clone();
    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<String> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", medium_id)).exists() {
        medium_captions_exist = true;
        for caption_name in read_lines_to_vec(&format!("source/{}/captions/list.txt", medium_id)) {
            medium_captions_list.push(caption_name);
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_chapters_exist =
        std::path::Path::new(&format!("source/{}/chapters.vtt", medium_id)).exists();
    let medium_previews_exist =
        std::path::Path::new(&format!("source/{}/previews/previews.vtt", medium_id)).exists();

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id,
        medium_name: medium.name,
        medium_owner: medium.owner,
        medium_upload: prettyunixtime(medium.upload).await,
        medium_views: medium.views,
        medium_type: medium.r#type,
        medium_captions_exist,
        medium_captions_list,
        medium_chapters_exist,
        medium_previews_exist,
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

    // Use graph traversal: list -> list_contains -> media
    let mut result = db
        .query("SELECT record::id(out.id) AS id, out.name AS name, out.owner AS owner, out.views AS views, out.type AS type FROM list_contains WHERE in = type::thing('lists', $listid) ORDER BY position ASC LIMIT 41 START $offset")
        .bind(("listid", &listid))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut items: Vec<Medium> = result.take(0).expect("Database error");

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
    let mut result = db
        .query("SELECT record::id(out.id) AS id, out.name AS name, out.owner AS owner, out.views AS views, out.type AS type FROM list_contains WHERE in = type::thing('lists', $listid) ORDER BY position ASC")
        .bind(("listid", &listid))
        .await
        .expect("Database error");

    let media: Vec<Medium> = result.take(0).expect("Database error");

    let template = HXMediumListTemplate {
        media,
        current_medium_id: mediumid,
        config
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

    let lists = get_list_modal_entries(&db, &user_info.login, &mediumid).await;

    let mut grp_result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

    let template = HXListModalTemplate {
        lists,
        medium_id: mediumid,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

/// Helper to build list modal entries with already_added flag
async fn get_list_modal_entries(db: &Db, owner: &str, mediumid: &str) -> Vec<ListModalEntry> {
    // Get all user's lists
    #[derive(Deserialize)]
    struct ListRow {
        id: String,
        name: String,
    }

    let mut result = db
        .query("SELECT record::id(id) AS id, name FROM lists WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", owner))
        .await
        .unwrap_or_else(|_| unreachable!());

    let lists: Vec<ListRow> = result.take(0).unwrap_or_default();

    // For each list, check if the medium is already added via graph edge
    let mut entries = Vec::new();
    for list in lists {
        let mut check = db
            .query("SELECT count() AS cnt FROM list_contains WHERE in = type::thing('lists', $listid) AND out = type::thing('media', $mediaid) GROUP ALL")
            .bind(("listid", &list.id))
            .bind(("mediaid", mediumid))
            .await
            .unwrap_or_else(|_| unreachable!());

        #[derive(Deserialize)]
        struct CntRow { cnt: i64 }

        let cnt: Option<CntRow> = check.take(0).unwrap_or(None);
        let already_added = cnt.map(|c| c.cnt > 0).unwrap_or(false);

        entries.push(ListModalEntry {
            id: list.id,
            name: list.name,
            already_added,
        });
    }

    entries
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
    let restricted_to_group = if visibility == "restricted" {
        form.restricted_group.clone().filter(|g| !g.is_empty())
    } else {
        None
    };

    // Create the list
    db.query("CREATE type::thing('lists', $id) SET name = $name, owner = $owner, visibility = $vis, restricted_to_group = $group")
        .bind(("id", &list_id))
        .bind(("name", &form.name))
        .bind(("owner", &user_info.login))
        .bind(("vis", visibility))
        .bind(("group", &restricted_to_group))
        .await
        .expect("Database error");

    // Add the medium to the list via graph edge
    db.query("RELATE type::thing('lists', $listid) -> list_contains -> type::thing('media', $mediaid) SET position = 0")
        .bind(("listid", &list_id))
        .bind(("mediaid", &mediumid))
        .await
        .expect("Database error");

    // Return updated modal
    let lists = get_list_modal_entries(&db, &user_info.login, &mediumid).await;

    let mut grp_result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

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

    // Verify list ownership
    #[derive(Deserialize)]
    struct OwnerRow { owner: String }

    let mut result = db
        .query("SELECT owner FROM type::thing('lists', $id)")
        .bind(("id", &listid))
        .await
        .expect("Database error");

    let owner_row: Option<OwnerRow> = result.take(0).expect("Database error");
    match owner_row {
        Some(row) if row.owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Get max position from existing edges
    #[derive(Deserialize)]
    struct PosRow { max_pos: Option<i64> }

    let mut pos_result = db
        .query("SELECT math::max(position) AS max_pos FROM list_contains WHERE in = type::thing('lists', $listid) GROUP ALL")
        .bind(("listid", &listid))
        .await
        .unwrap_or_else(|_| unreachable!());

    let pos: Option<PosRow> = pos_result.take(0).unwrap_or(None);
    let next_pos = pos.and_then(|p| p.max_pos).unwrap_or(-1) + 1;

    // Add item via graph edge
    db.query("RELATE type::thing('lists', $listid) -> list_contains -> type::thing('media', $mediaid) SET position = $pos")
        .bind(("listid", &listid))
        .bind(("mediaid", &mediumid))
        .bind(("pos", next_pos))
        .await
        .expect("Database error");

    // Return updated modal
    let lists = get_list_modal_entries(&db, &user_info.login, &mediumid).await;

    let mut grp_result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

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

    // Verify list ownership
    #[derive(Deserialize)]
    struct OwnerRow { owner: String }

    let mut result = db
        .query("SELECT owner FROM type::thing('lists', $id)")
        .bind(("id", &listid))
        .await
        .expect("Database error");

    let owner_row: Option<OwnerRow> = result.take(0).expect("Database error");
    match owner_row {
        Some(row) if row.owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Delete the graph edge
    db.query("DELETE FROM list_contains WHERE in = type::thing('lists', $listid) AND out = type::thing('media', $mediaid)")
        .bind(("listid", &listid))
        .bind(("mediaid", &mediumid))
        .await
        .expect("Database error");

    // Return updated modal
    let lists = get_list_modal_entries(&db, &user_info.login, &mediumid).await;

    let mut grp_result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

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

    // Verify ownership
    #[derive(Deserialize)]
    struct OwnerRow { owner: String }

    let mut result = db
        .query("SELECT owner FROM type::thing('lists', $id)")
        .bind(("id", &listid))
        .await
        .expect("Database error");

    let owner_row: Option<OwnerRow> = result.take(0).expect("Database error");
    match owner_row {
        Some(row) if row.owner == user_info.login => {}
        _ => {
            return Html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            );
        }
    }

    // Delete graph edges and the list record
    let _ = db
        .query("DELETE FROM list_contains WHERE in = type::thing('lists', $id); DELETE type::thing('lists', $id)")
        .bind(("id", &listid))
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

    // Get groups user belongs to for restricted list access
    let groups = if !user_login.is_empty() {
        get_user_group_ids(&db, &user_login).await
    } else {
        vec![]
    };

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, visibility, restricted_to_group, count(->list_contains) AS item_count FROM lists WHERE owner = $owner AND (visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN $groups)) ORDER BY created DESC LIMIT 41 START $offset")
        .bind(("owner", &userid))
        .bind(("groups", &groups))
        .bind(("offset", offset))
        .await
        .expect("Database error");

    let mut lists: Vec<ListWithCount> = result.take(0).expect("Database error");

    let has_more = lists.len() == 41;
    if has_more {
        lists.truncate(40);
    }
    let next_page = page + 1;
    let next_url = format!("/hx/userlists/{}/{}", userid, next_page);

    let template = HXUserListsTemplate { lists, page, has_more, next_url };
    Html(minifi_html(template.render().unwrap()))
}
