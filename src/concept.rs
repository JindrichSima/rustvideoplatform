#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
    r#type: String,
}

#[derive(Deserialize)]
struct MediumConceptRow {
    id: surrealdb::RecordId,
    name: String,
    processed: bool,
    #[serde(rename = "type")]
    r#type: String,
}

async fn concepts(
    Extension(db): Extension<Db>,
    Extension(config): Extension<Config>,
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
        active_tab: "concepts".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html", escape = "none")]
struct HXConceptsTemplate {
    concepts: Vec<MediumConcept>,
}

async fn hx_concepts(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let userinfo = get_user_login(headers, &db, redis.clone()).await.unwrap();

    let mut response = db
        .query("SELECT id, name, processed, type FROM media_concepts WHERE owner = $owner;")
        .bind(("owner", userinfo.login.clone()))
        .await
        .expect("Database error");

    let rows: Vec<MediumConceptRow> = response.take(0).expect("Database error");
    let concepts: Vec<MediumConcept> = rows
        .into_iter()
        .map(|row| MediumConcept {
            id: row.id.key().to_string(),
            name: row.name,
            processed: row.processed,
            r#type: row.r#type,
        })
        .collect();

    let template = HXConceptsTemplate { concepts };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/concept.html", escape = "none")]
struct ConceptTemplate {
    sidebar: String,
    config: Config,
    concept: MediumConcept,
    common_headers: CommonHeaders,
    owner_groups: Vec<UserGroup>,
}

async fn concept(
    Extension(db): Extension<Db>,
    Extension(config): Extension<Config>,
    Extension(redis): Extension<RedisConn>,
    Path(conceptid): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let mut response = db
        .query(
            "SELECT id, name, type, processed FROM media_concepts WHERE owner = $owner AND id = $id AND processed = true;",
        )
        .bind(("owner", user_info.login.clone()))
        .bind(("id", surrealdb::RecordId::from_table_key("media_concepts", &conceptid)))
        .await
        .expect("Database error");

    let row: Option<MediumConceptRow> = response.take(0).expect("Database error");
    let row = row.expect("Concept not found");
    let concept = MediumConcept {
        id: row.id.key().to_string(),
        name: row.name,
        processed: row.processed,
        r#type: row.r#type,
    };

    // Fetch user's groups for the dropdown (system groups + user groups)
    let mut owner_groups = system_groups_for_owner(&user_info.login);

    #[derive(Deserialize)]
    struct UserGroupRow {
        id: surrealdb::RecordId,
        name: String,
        owner: String,
    }

    let mut grp_response = db
        .query("SELECT id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC;")
        .bind(("owner", user_info.login.clone()))
        .await
        .unwrap_or_else(|_| panic!("Database error"));

    let grp_rows: Vec<UserGroupRow> = grp_response.take(0).unwrap_or_default();
    let user_groups: Vec<UserGroup> = grp_rows
        .into_iter()
        .map(|row| UserGroup {
            id: row.id.key().to_string(),
            name: row.name,
            owner: row.owner,
        })
        .collect();
    owner_groups.extend(user_groups);

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ConceptTemplate {
        sidebar,
        config,
        concept,
        common_headers,
        owner_groups,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PublishForm {
    medium_id: String,
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
    medium_restricted_group: Option<String>,
}

async fn publish(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(conceptid): Path<String>,
    Form(form): Form<PublishForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let mut response = db
        .query(
            "SELECT id, name, type, processed FROM media_concepts WHERE owner = $owner AND id = $id AND processed = true;",
        )
        .bind(("owner", user_info.login.clone()))
        .bind(("id", surrealdb::RecordId::from_table_key("media_concepts", &conceptid)))
        .await
        .expect("Database error");

    let row: Option<MediumConceptRow> = response.take(0).expect("Database error");
    let row = row.expect("Concept not found");
    let concept = MediumConcept {
        id: row.id.key().to_string(),
        name: row.name,
        processed: row.processed,
        r#type: row.r#type,
    };

    let concept_move = move_dir(
        format!("upload/{}_processing", conceptid).as_str(),
        format!("source/{}", form.medium_id.to_ascii_lowercase()).as_str(),
    )
    .await;
    if concept_move.is_ok() {
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
        let description: serde_json::Value =
            serde_json::from_str(&form.medium_description).unwrap();

        let medium_id = form.medium_id.to_ascii_lowercase();
        let media_record_id = surrealdb::RecordId::from_table_key("media", &medium_id);
        let _ = db
            .query(
                "CREATE $id SET
    name = $name,
    description = $description,
    owner = $owner,
    public = $public,
    visibility = $visibility,
    restricted_to_group = $restricted_to_group,
    type = $type,
    upload = time::unix(time::now());",
            )
            .bind(("id", media_record_id))
            .bind(("name", form.medium_name.clone()))
            .bind(("description", description.clone()))
            .bind(("owner", user_info.login.clone()))
            .bind(("public", ispublic))
            .bind(("visibility", visibility.clone()))
            .bind(("restricted_to_group", restricted_to_group.clone()))
            .bind(("type", concept.r#type.clone()))
            .await;

        let concept_record_id =
            surrealdb::RecordId::from_table_key("media_concepts", &concept.id);
        let _ = db
            .query("DELETE $id;")
            .bind(("id", concept_record_id))
            .await;

        return Html(format!(
            "<script>window.location.replace(\"/m/{}\");</script>",
            form.medium_id
        ));
    } else {
        return Html(format!(
            "<h1>ERROR MOVING PROCESSED CONCEPT TO MEDIA!</h1><br>{:?}",
            concept_move
        ));
    }
}
