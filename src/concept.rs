#[derive(Serialize, Deserialize)]
struct MediumConcept {
    id: String,
    name: String,
    processed: bool,
    r#type: String,
}

#[derive(Template)]
#[template(path = "pages/concepts.html", escape = "none")]
struct ConceptsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
}
async fn concepts(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
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
    let template = ConceptsTemplate {
        sidebar,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Template)]
#[template(path = "pages/hx-concepts.html", escape = "none")]
struct HXConceptsTemplate {
    concepts: Vec<MediumConcept>,
}
async fn hx_concepts(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let userinfo = get_user_login(headers, &pool, session_store).await.unwrap();

    let concepts = sqlx::query_as!(
        MediumConcept,
        "SELECT id,name,processed,type FROM media_concepts WHERE owner = $1;",
        userinfo.login
    )
    .fetch_all(&pool)
    .await
    .expect("Database error");
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
}
async fn concept(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    Path(conceptid): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let concept = sqlx::query_as!(MediumConcept,
        "SELECT id,name,type,processed FROM media_concepts WHERE owner = $1 AND id = $2 AND processed = true;", user_info.login, conceptid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let sidebar = generate_sidebar(&config, "studio".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = ConceptTemplate {
        sidebar,
        config,
        concept,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PublishForm {
    medium_id: String,
    medium_name: String,
    medium_description: String,
    medium_visibility: String,
}
async fn publish(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(conceptid): Path<String>,
    Form(form): Form<PublishForm>,
) -> axum::response::Html<String> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;
    if !is_logged(user_info.clone()).await {
        return Html("<script>window.location.replace(\"/login\");</script>".to_owned());
    }
    let user_info = user_info.unwrap();

    let concept = sqlx::query_as!(MediumConcept,
        "SELECT id,name,type,processed FROM media_concepts WHERE owner = $1 AND id = $2 AND processed = true;", user_info.login, conceptid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let concept_move = move_dir(
        format!("upload/{}_processing", conceptid).as_str(),
        format!("source/{}", form.medium_id.to_ascii_lowercase()).as_str(),
    )
    .await;
    if concept_move.is_ok() {
        let ispublic: bool;
        if form.medium_visibility == "public".to_owned() {
            ispublic = true;
        } else {
            ispublic = false;
        }
        let _ = sqlx::query!(
            "INSERT INTO media (id,name,description,owner,public,type) VALUES ($1,$2,$3,$4,$5,$6);",
            form.medium_id.to_ascii_lowercase(),
            form.medium_name,
            form.medium_description,
            user_info.login,
            ispublic,
            concept.r#type
        )
        .execute(&pool)
        .await;
        let _ = sqlx::query!("DELETE FROM media_concepts WHERE id=$1;", concept.id)
            .execute(&pool)
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
