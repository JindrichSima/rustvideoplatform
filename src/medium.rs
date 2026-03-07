struct CaptionEntry {
    label: String,
    filename: String,
}

fn parse_caption_entry(entry: &str) -> CaptionEntry {
    let entry = entry.trim();
    if entry.ends_with(".srt") || entry.ends_with(".ass") || entry.ends_with(".vtt") {
        let dot_pos = entry.rfind('.').unwrap();
        CaptionEntry {
            label: entry[..dot_pos].to_string(),
            filename: entry.to_string(),
        }
    } else {
        CaptionEntry {
            label: entry.to_string(),
            filename: format!("{}.vtt", entry),
        }
    }
}

#[derive(Template)]
#[template(path = "pages/medium.html", escape = "none")]
struct MediumTemplate {
    sidebar: String,
    medium_id: String,
    medium_name: String,
    medium_owner: String,
    medium_owner_name: String,
    medium_owner_picture: Option<String>,
    medium_upload: String,
    medium_views: i64,
    medium_type: String,
    medium_captions_exist: bool,
    medium_captions_list: Vec<CaptionEntry>,
    medium_custom_font: bool,
    medium_chapters_exist: bool,
    medium_previews_exist: bool,
    is_cmaf: bool,
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

async fn medium(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers.clone(), &pool, redis.clone()).await;
    let is_logged_in = user.is_some();

    // Fetch media with visibility info and owner profile
    let medium_row = sqlx::query(
        "SELECT m.id, m.name, m.description, m.upload, m.owner, m.views, m.type, m.visibility, m.restricted_to_group, u.name as owner_name, u.profile_picture as owner_picture FROM media m LEFT JOIN users u ON m.owner = u.login WHERE m.id=$1;"
    )
    .bind(mediumid.to_ascii_lowercase())
    .fetch_one(&pool)
    .await;

    let medium = match medium_row {
        Ok(row) => row,
        Err(_) => {
            return Html(minifi_html(
                "<script>window.location.replace(\"/\");</script>".to_owned(),
            ));
        }
    };

    use sqlx::Row;
    let visibility: String = medium.get("visibility");
    let restricted_to_group: Option<String> = medium.get("restricted_to_group");
    let owner: String = medium.get("owner");

    // Access control for restricted content
    if !can_access_restricted(&pool, &visibility, restricted_to_group.as_deref(), &owner, &user, redis.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/\");</script>".to_owned(),
        ));
    }

    let common_headers = extract_common_headers(&headers).unwrap();

    let medium_id: String = medium.get("id");
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
        medium_name: medium.get("name"),
        medium_owner: medium.get("owner"),
        medium_owner_name: medium.get("owner_name"),
        medium_owner_picture: medium.get("owner_picture"),
        medium_upload: prettyunixtime(medium.get("upload")).await,
        medium_views: medium.get("views"),
        medium_type: medium.get("type"),
        medium_captions_exist,
        medium_captions_list,
        medium_custom_font,
        medium_chapters_exist,
        medium_previews_exist,
        is_cmaf,
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
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    Json(
        sqlx::query!(
            "SELECT description FROM media WHERE id=$1;",
            mediumid.to_ascii_lowercase()
        )
        .fetch_one(&pool)
        .await
        .expect("Database error")
        .description
        .unwrap_or_default(),
    )
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
