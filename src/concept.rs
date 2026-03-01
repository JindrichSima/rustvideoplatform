#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
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

    let mut result = db
        .query("SELECT record::id(id) AS id, name, processed, type FROM media_concepts WHERE owner = $owner")
        .bind(("owner", &userinfo.login))
        .await
        .expect("Database error");

    let concepts: Vec<MediumConcept> = result.take(0).expect("Database error");
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

    let mut result = db
        .query("SELECT record::id(id) AS id, name, type, processed FROM media_concepts WHERE owner = $owner AND record::id(id) = $id AND processed = true")
        .bind(("owner", &user_info.login))
        .bind(("id", &conceptid))
        .await
        .expect("Database error");

    let concept: Option<MediumConcept> = result.take(0).expect("Database error");
    let concept = concept.expect("Concept not found");

    let mut grp_result = db
        .query("SELECT record::id(id) AS id, name, owner FROM user_groups WHERE owner = $owner ORDER BY created DESC")
        .bind(("owner", &user_info.login))
        .await
        .unwrap_or_else(|_| unreachable!());

    let owner_groups: Vec<UserGroup> = grp_result.take(0).unwrap_or_default();

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

    // Verify concept ownership and processed status
    let mut result = db
        .query("SELECT record::id(id) AS id, name, type, processed FROM media_concepts WHERE owner = $owner AND record::id(id) = $id AND processed = true")
        .bind(("owner", &user_info.login))
        .bind(("id", &conceptid))
        .await
        .expect("Database error");

    let concept: Option<MediumConcept> = result.take(0).expect("Database error");
    let concept = concept.expect("Concept not found");

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
        let restricted_to_group = if visibility == "restricted" {
            form.medium_restricted_group.clone().filter(|g| !g.is_empty())
        } else {
            None
        };
        let description: serde_json::Value =
            serde_json::from_str(&form.medium_description).unwrap();

        let _ = db
            .query("CREATE type::thing('media', $id) SET name = $name, description = $desc, owner = $owner, visibility = $vis, restricted_to_group = $group, type = $type")
            .bind(("id", form.medium_id.to_ascii_lowercase()))
            .bind(("name", &form.medium_name))
            .bind(("desc", &description))
            .bind(("owner", &user_info.login))
            .bind(("vis", &visibility))
            .bind(("group", &restricted_to_group))
            .bind(("type", &concept.r#type))
            .await;

        let _ = db
            .query("DELETE type::thing('media_concepts', $id)")
            .bind(("id", &concept.id))
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
