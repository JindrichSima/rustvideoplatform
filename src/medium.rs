#[derive(Template)]
#[template(path = "pages/medium.html", escape = "none")]
struct MediumTemplate {
    sidebar: String,
    medium_id: String,
    medium_name: String,
    medium_description: String,
    medium_owner: String,
    medium_likes: i64,
    medium_dislikes: i64,
    medium_upload: String,
    medium_views: i64,
    medium_type: String,
    medium_captions_exist: bool,
    medium_captions_list: Vec<String>,
    medium_chapters_exist: bool,
    medium_previews_exist: bool,
    config: Config,
    common_headers: CommonHeaders,
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
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers).unwrap();
    let medium = sqlx::query!(
        "SELECT id,name,description,upload,owner,likes,dislikes,views,type FROM media WHERE id=$1;",
        mediumid
    )
    .fetch_one(&pool)
    .await
    .expect("Database error");

    let medium_captions_exist: bool;
    let mut medium_captions_list: Vec<String> = Vec::new();
    if std::path::Path::new(&format!("source/{}/captions/list.txt", mediumid)).exists() {
        medium_captions_exist = true;
        for caption_name in read_lines_to_vec(&format!("source/{}/captions/list.txt", mediumid)) {
            medium_captions_list.push(caption_name);
        }
    } else {
        medium_captions_exist = false;
    }

    let medium_chapters_exist: bool;
    if std::path::Path::new(&format!("source/{}/chapters.vtt", mediumid)).exists() {
        medium_chapters_exist = true;
    } else {
        medium_chapters_exist = false;
    }

    let medium_previews_exist: bool;
    if std::path::Path::new(&format!("source/{}/previews/previews.json", mediumid)).exists() {
        medium_previews_exist = true;
    } else {
        medium_previews_exist = false;
    }

    let sidebar = generate_sidebar(&config, "medium".to_owned());
    let template = MediumTemplate {
        sidebar,
        medium_id: medium.id,
        medium_name: medium.name,
        medium_description: medium.description,
        medium_owner: medium.owner,
        medium_likes: medium.likes,
        medium_dislikes: medium.dislikes,
        medium_upload: prettyunixtime(medium.upload).await,
        medium_views: medium.views,
        medium_type: medium.r#type,
        medium_captions_exist,
        medium_captions_list,
        medium_chapters_exist,
        medium_previews_exist,
        config,
        common_headers,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct Preview {
    startTime: u128,
    endTime: u128,
    text: String,
}
async fn medium_previews_prepare(Path(mediumid): Path<String>) -> Json<Vec<Preview>> {
    let source_file_path = format!("source/{}/previews/previews.json", mediumid);
    let parsed_preview_list: Vec<Preview> =
        serde_json::from_str(&fs::read_to_string(source_file_path).await.unwrap()).unwrap();
    let mut new_preview_list: Vec<Preview> = Vec::new();
    for preview in parsed_preview_list {
        let fixed_url = format!("/source/{}/{}", mediumid, preview.text);
        let new_preview: Preview = Preview {
            startTime: preview.startTime,
            endTime: preview.endTime,
            text: fixed_url,
        };
        new_preview_list.push(new_preview);
    }

    Json(new_preview_list)
}
