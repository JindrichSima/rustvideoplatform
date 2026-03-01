#[derive(Serialize, Deserialize, Clone)]
struct UserGroup {
    id: String,
    name: String,
    owner: String,
}

#[derive(Serialize, Deserialize)]
struct UserGroupWithCount {
    id: String,
    name: String,
    owner: String,
    member_count: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct GroupMember {
    user_login: String,
}

#[derive(Deserialize)]
struct CreateGroupForm {
    name: String,
}

#[derive(Deserialize)]
struct AddMemberForm {
    user_login: String,
}

async fn studio_groups(
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
        active_tab: "groups".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-studio-groups.html", escape = "none")]
struct HXStudioGroupsTemplate {
    groups: Vec<UserGroupWithCount>,
}

/// Helper to fetch groups with member counts for the current user
async fn fetch_groups_with_counts(db: &Db, owner: &str) -> Vec<UserGroupWithCount> {
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, count(<-has_member) AS member_count FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", owner))
        .await
        .expect("Database error");

    result.take(0).expect("Database error")
}

async fn hx_studio_groups(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_group(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<CreateGroupForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let group_id = generate_medium_id();

    db.query("CREATE type::thing('user_groups', $id) SET name = $name, owner = $owner")
        .bind(("id", &group_id))
        .bind(("name", &form.name))
        .bind(("owner", &user_info.login))
        .await
        .expect("Database error");

    // Return updated groups list
    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_delete_group(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify ownership
    #[derive(Deserialize)]
    struct OwnerRow { owner: String }

    let mut result = db
        .query("SELECT owner FROM type::thing('user_groups', $id)")
        .bind(("id", &groupid))
        .await
        .expect("Database error");

    let owner_row: Option<OwnerRow> = result.take(0).expect("Database error");
    match owner_row {
        Some(row) if row.owner == user_info.login => {}
        _ => return Html("".as_bytes().to_vec()),
    }

    // Clear group references from media and lists, delete member edges, then delete group
    let _ = db
        .query("UPDATE media SET visibility = 'hidden', restricted_to_group = NONE WHERE restricted_to_group = $gid; UPDATE lists SET visibility = 'hidden', restricted_to_group = NONE WHERE restricted_to_group = $gid; DELETE FROM has_member WHERE in = type::thing('user_groups', $gid); DELETE type::thing('user_groups', $gid)")
        .bind(("gid", &groupid))
        .await;

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated groups list
    let groups = fetch_groups_with_counts(&db, &user_info.login).await;

    let template = HXStudioGroupsTemplate { groups };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-group-members.html", escape = "none")]
struct HXGroupMembersTemplate {
    group: UserGroup,
    members: Vec<GroupMember>,
    is_owner: bool,
}

async fn hx_group_members(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner FROM type::thing('user_groups', $id)")
        .bind(("id", &groupid))
        .await
        .expect("Database error");

    let group: Option<UserGroup> = result.take(0).expect("Database error");

    let group = match group {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    let is_owner = group.owner == user_info.login;

    // Get members via graph edge traversal
    let mut mem_result = db
        .query("SELECT record::id(out) AS user_login FROM has_member WHERE in = type::thing('user_groups', $gid) ORDER BY user_login")
        .bind(("gid", &groupid))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = mem_result.take(0).expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_add_group_member(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(groupid): Path<String>,
    Form(form): Form<AddMemberForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify group ownership
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner FROM type::thing('user_groups', $id)")
        .bind(("id", &groupid))
        .await
        .expect("Database error");

    let group: Option<UserGroup> = result.take(0).expect("Database error");

    let group = match group {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Check that the user exists
    #[derive(Deserialize)]
    struct LoginRow { login: String }

    let mut user_check = db
        .query("SELECT VALUE record::id(id) FROM type::thing('users', $login)")
        .bind(("login", &form.user_login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let exists: Option<String> = user_check.take(0).unwrap_or(None);

    if exists.is_some() {
        // Delete existing edge first (idempotent), then create new one
        let _ = db
            .query("DELETE FROM has_member WHERE in = type::thing('user_groups', $gid) AND out = type::thing('users', $login); RELATE type::thing('user_groups', $gid) -> has_member -> type::thing('users', $login)")
            .bind(("gid", &groupid))
            .bind(("login", &form.user_login))
            .await;

        // Invalidate Redis group membership cache
        let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;
    }

    // Return updated members list
    let mut mem_result = db
        .query("SELECT record::id(out) AS user_login FROM has_member WHERE in = type::thing('user_groups', $gid) ORDER BY user_login")
        .bind(("gid", &groupid))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = mem_result.take(0).expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_remove_group_member(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path((groupid, login)): Path<(String, String)>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("".as_bytes().to_vec());
    }
    let user_info = user_info.unwrap();

    // Verify group ownership
    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner FROM type::thing('user_groups', $id)")
        .bind(("id", &groupid))
        .await
        .expect("Database error");

    let group: Option<UserGroup> = result.take(0).expect("Database error");

    let group = match group {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Delete the graph edge
    db.query("DELETE FROM has_member WHERE in = type::thing('user_groups', $gid) AND out = type::thing('users', $login)")
        .bind(("gid", &groupid))
        .bind(("login", &login))
        .await
        .expect("Database error");

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated members list
    let mut mem_result = db
        .query("SELECT record::id(out) AS user_login FROM has_member WHERE in = type::thing('user_groups', $gid) ORDER BY user_login")
        .bind(("gid", &groupid))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = mem_result.take(0).expect("Database error");

    let template = HXGroupMembersTemplate {
        group,
        members,
        is_owner: true,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_user_groups_json(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> Json<Vec<UserGroup>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(vec![]);
    }
    let user_info = user_info.unwrap();

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let groups: Vec<UserGroup> = result.take(0).unwrap_or_default();

    Json(groups)
}
