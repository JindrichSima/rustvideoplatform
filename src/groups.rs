#[derive(Serialize, Deserialize, SurrealValue, Clone)]
struct UserGroup {
    id: String,
    name: String,
    owner: String,
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct UserGroupWithCount {
    id: String,
    name: String,
    owner: String,
    member_count: Option<i64>,
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct GroupMember {
    user_login: String,
}

#[derive(Deserialize, SurrealValue)]
struct CreateGroupForm {
    name: String,
}

#[derive(Deserialize, SurrealValue)]
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

#[derive(Template)]
#[template(path = "pages/hx-groups-list.html", escape = "none")]
struct HXGroupsListTemplate {
    groups: Vec<UserGroupWithCount>,
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

    let mut groups: Vec<UserGroupWithCount> = vec![
        UserGroupWithCount {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
        UserGroupWithCount {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
    ];

    let mut resp = db
        .query("SELECT id, name, owner, (SELECT count() FROM user_group_members WHERE group_id = $parent.id GROUP ALL)[0].count AS member_count FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    let user_groups: Vec<UserGroupWithCount> = resp.take(0).unwrap_or_default();

    groups.extend(user_groups);

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

    db.query("CREATE user_groups:[$id] SET name = $name, owner = $owner, created = time::unix(time::now());")
        .bind(("id", group_id.clone()))
        .bind(("name", form.name.clone()))
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    // Return updated groups list (with system groups prepended)
    let mut groups: Vec<UserGroupWithCount> = vec![
        UserGroupWithCount {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
        UserGroupWithCount {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
    ];

    let mut resp = db
        .query("SELECT id, name, owner, (SELECT count() FROM user_group_members WHERE group_id = $parent.id GROUP ALL)[0].count AS member_count FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    let user_groups: Vec<UserGroupWithCount> = resp.take(0).unwrap_or_default();

    groups.extend(user_groups);

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

    // Prevent deletion of system groups
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify ownership
    #[derive(Deserialize, SurrealValue)]
    struct OwnerRecord {
        owner: String,
    }

    let mut resp = db
        .query("SELECT owner FROM user_groups WHERE id = $id;")
        .bind(("id", groupid.clone()))
        .await
        .expect("Database error");

    let owner_check: Option<OwnerRecord> = resp.take(0).unwrap_or_default();

    if let Some(row) = owner_check {
        if row.owner != user_info.login {
            return Html("".as_bytes().to_vec());
        }
    } else {
        return Html("".as_bytes().to_vec());
    }

    // Clear group references and delete members/group in a single round-trip
    db.query(
        "UPDATE media SET visibility = 'hidden', restricted_to_group = NONE WHERE restricted_to_group = $gid; \
         UPDATE lists SET visibility = 'hidden', restricted_to_group = NONE WHERE restricted_to_group = $gid; \
         DELETE FROM user_group_members WHERE group_id = $gid; \
         DELETE FROM user_groups WHERE id = $id;"
    )
    .bind(("gid", groupid.clone()))
    .bind(("id", groupid.clone()))
    .await
    .expect("Database error");

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated groups list (with system groups prepended)
    let mut groups: Vec<UserGroupWithCount> = vec![
        UserGroupWithCount {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
        UserGroupWithCount {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: user_info.login.clone(),
            member_count: None,
        },
    ];

    let mut resp = db
        .query("SELECT id, name, owner, (SELECT count() FROM user_group_members WHERE group_id = $parent.id GROUP ALL)[0].count AS member_count FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    let user_groups: Vec<UserGroupWithCount> = resp.take(0).unwrap_or_default();

    groups.extend(user_groups);

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

    // System groups are automatically managed - show info instead of member list
    if is_system_group(&groupid) {
        let group = if groupid == SYSTEM_GROUP_ALL_REGISTERED {
            UserGroup { id: groupid, name: "All Registered Users".to_owned(), owner: user_info.login.clone() }
        } else {
            UserGroup { id: groupid, name: "Subscribers Only".to_owned(), owner: user_info.login.clone() }
        };
        let template = HXGroupMembersTemplate {
            group,
            members: vec![],
            is_owner: false, // prevents showing add/remove controls
        };
        return Html(minifi_html(template.render().unwrap()));
    }

    let mut resp = db
        .query("SELECT id, name, owner FROM user_groups WHERE id = $id;")
        .bind(("id", groupid.clone()))
        .await
        .expect("Database error");

    let group_row: Option<UserGroup> = resp.take(0).unwrap_or_default();

    let group = match group_row {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    let is_owner = group.owner == user_info.login;

    let mut members_resp = db
        .query("SELECT user_login FROM user_group_members WHERE group_id = $gid ORDER BY user_login;")
        .bind(("gid", groupid.clone()))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = members_resp.take(0).unwrap_or_default();

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

    // System groups cannot have members manually added
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify group ownership
    let mut resp = db
        .query("SELECT id, name, owner FROM user_groups WHERE id = $id;")
        .bind(("id", groupid.clone()))
        .await
        .expect("Database error");

    let group_row: Option<UserGroup> = resp.take(0).unwrap_or_default();

    let group = match group_row {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    // Check that the user exists
    #[derive(Deserialize, SurrealValue)]
    struct LoginRecord {
        login: String,
    }

    let mut user_resp = db
        .query("SELECT login FROM users WHERE login = $login;")
        .bind(("login", form.user_login.clone()))
        .await
        .expect("Database error");

    let user_exists: Option<LoginRecord> = user_resp.take(0).unwrap_or_default();

    if user_exists.is_some() {
        // Upsert member (ignore if already exists)
        let _ = db
            .query("UPSERT user_group_members:[$gid, $user] SET group_id = $gid, user_login = $user;")
            .bind(("gid", groupid.clone()))
            .bind(("user", form.user_login.clone()))
            .await;

        // Invalidate Redis group membership cache
        let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;
    }

    // Return updated members list
    let mut members_resp = db
        .query("SELECT user_login FROM user_group_members WHERE group_id = $gid ORDER BY user_login;")
        .bind(("gid", groupid.clone()))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = members_resp.take(0).unwrap_or_default();

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

    // System groups cannot have members manually removed
    if is_system_group(&groupid) {
        return Html("".as_bytes().to_vec());
    }

    // Verify group ownership
    let mut resp = db
        .query("SELECT id, name, owner FROM user_groups WHERE id = $id;")
        .bind(("id", groupid.clone()))
        .await
        .expect("Database error");

    let group_row: Option<UserGroup> = resp.take(0).unwrap_or_default();

    let group = match group_row {
        Some(g) => g,
        None => return Html("".as_bytes().to_vec()),
    };

    if group.owner != user_info.login {
        return Html("".as_bytes().to_vec());
    }

    db.query("DELETE FROM user_group_members WHERE group_id = $gid AND user_login = $user;")
        .bind(("gid", groupid.clone()))
        .bind(("user", login.clone()))
        .await
        .expect("Database error");

    // Invalidate Redis group membership cache
    let _: Result<(), _> = redis.clone().del(format!("group:{}:members", groupid)).await;

    // Return updated members list
    let mut members_resp = db
        .query("SELECT user_login FROM user_group_members WHERE group_id = $gid ORDER BY user_login;")
        .bind(("gid", groupid.clone()))
        .await
        .expect("Database error");

    let members: Vec<GroupMember> = members_resp.take(0).unwrap_or_default();

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

    let mut groups = system_groups_for_owner(&user_info.login);

    let mut resp = db
        .query("SELECT id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
        .bind(("owner", user_info.login.clone()))
        .await
        .expect("Database error");

    let user_groups: Vec<UserGroup> = resp.take(0).unwrap_or_default();

    groups.extend(user_groups);

    Json(groups)
}
