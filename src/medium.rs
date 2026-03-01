#[derive(Template)]
#[template(path = "pages/medium.html", escape = "none")]
struct MediumTemplate {
    sidebar: String,
    medium_id: String,
    medium_name: String,
    medium_owner: String,
    medium_upload: String,
    medium_views: i64,
    medium_type: String,
    medium_captions_exist: bool,
    medium_captions_list: Vec<String>,
    medium_chapters_exist: bool,
    medium_previews_exist: bool,
    config: Config,
    common_headers: CommonHeaders,
    is_logged_in: bool,
    list_id: String,
    list_name: String,
}

#[derive(Serialize, Deserialize)]
struct Medium {
    id: String,
    name: String,
    owner: String,
    views: i64,
    r#type: String,
}

#[derive(Deserialize)]
struct MediaRow {
    id: String,
    name: String,
    description: Option<serde_json::Value>,
    upload: i64,
    owner: String,
    views: i64,
    r#type: String,
    visibility: String,
    restricted_to_group: Option<String>,
}

async fn medium(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers.clone(), &db, redis.clone()).await;
    let is_logged_in = user.is_some();

    let mut result = db
        .query("SELECT record::id(id) AS id, name, description, upload, owner, views, type, visibility, restricted_to_group FROM type::thing('media', $id)")
        .bind(("id", mediumid.to_ascii_lowercase()))
        .await
        .expect("Database error");

    let medium: Option<MediaRow> = result.take(0).expect("Database error");

    let medium = match medium {
        Some(row) => row,
        None => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    if !can_access_restricted(&db, &medium.visibility, medium.restricted_to_group.as_deref(), &medium.owner, &user, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers).unwrap();

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
        list_id: String::new(),
        list_name: String::new(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn medium_previews_prepare(Path(mediumid): Path<String>) -> Response<Body> {
    let source_file_path = format!("source/{}/previews/previews.vtt", mediumid);

    match tokio::fs::read_to_string(&source_file_path).await {
        Ok(vtt_content) => {
            let fixed_vtt = fix_vtt_urls(&vtt_content, &mediumid);
            Response::builder()
                .header(axum::http::header::CONTENT_TYPE, "text/vtt")
                .body(Body::from(fixed_vtt))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

fn fix_vtt_urls(vtt_content: &str, mediumid: &str) -> String {
    let base_path = format!("/source/{}/previews/", mediumid);
    vtt_content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("WEBVTT")
                || trimmed.contains("-->")
                || trimmed.starts_with("NOTE")
            {
                return line.to_string();
            }
            let path_part = trimmed.split('#').next().unwrap_or(trimmed);
            let is_avif = path_part.to_lowercase().ends_with(".avif");
            let is_relative = !trimmed.starts_with('/')
                && !trimmed.starts_with("http://")
                && !trimmed.starts_with("https://");
            if is_avif && is_relative {
                if let Some(hash_pos) = trimmed.find('#') {
                    let (path, fragment) = trimmed.split_at(hash_pos);
                    return format!("{}{}{}", base_path, path, fragment);
                } else {
                    return format!("{}{}", base_path, trimmed);
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn medium_description_prepare(
    Extension(db): Extension<Db>,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    #[derive(Deserialize)]
    struct DescRow { description: Option<serde_json::Value> }

    let mut result = db
        .query("SELECT description FROM type::thing('media', $id)")
        .bind(("id", mediumid.to_ascii_lowercase()))
        .await
        .expect("Database error");

    let row: Option<DescRow> = result.take(0).expect("Database error");
    Json(row.and_then(|r| r.description).unwrap_or_default())
}

#[derive(Template)]
#[template(path = "pages/hx-mediumcard.html", escape = "none")]
struct HXMediumCardTemplate {
    media: Vec<Medium>,
    config: Config,
    page: i64,
    has_more: bool,
    next_url: String,
}

#[derive(Template)]
#[template(path = "pages/hx-mediumlist.html", escape = "none")]
struct HXMediumListTemplate {
    current_medium_id: String,
    media: Vec<Medium>,
    config: Config,
}
