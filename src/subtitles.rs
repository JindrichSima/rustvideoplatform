fn convert_srt_to_vtt(content: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(content);
    let text = text.replace('\r', "");
    let mut result = String::from("WEBVTT\n\n");
    for line in text.lines() {
        if line.contains(" --> ") {
            result.push_str(&line.replace(',', "."));
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result.into_bytes()
}

fn ass_time_to_vtt(time: &str) -> String {
    // ASS time: h:mm:ss.cs (centiseconds) → VTT: hh:mm:ss.mmm
    let parts: Vec<&str> = time.splitn(3, ':').collect();
    if parts.len() != 3 {
        return "00:00:00.000".to_string();
    }
    let h: u32 = parts[0].parse().unwrap_or(0);
    let m: u32 = parts[1].parse().unwrap_or(0);
    let sec_parts: Vec<&str> = parts[2].splitn(2, '.').collect();
    let s: u32 = sec_parts[0].parse().unwrap_or(0);
    let cs: u32 = sec_parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, cs * 10)
}

fn strip_ass_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.replace("\\N", "\n").replace("\\n", "\n").replace("\\h", "\u{00A0}")
}

fn convert_ass_to_vtt(content: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(content);
    let text = text.replace('\r', "");
    let mut result = String::from("WEBVTT\n\n");

    let mut in_events = false;
    let mut start_idx: usize = 1;
    let mut end_idx: usize = 2;
    let mut text_idx: usize = 9;
    let mut cue_number: u32 = 1;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[Events]" {
            in_events = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_events = false;
            continue;
        }
        if !in_events {
            continue;
        }
        if trimmed.starts_with("Format:") {
            let fields: Vec<&str> = trimmed["Format:".len()..].split(',').map(str::trim).collect();
            for (i, &f) in fields.iter().enumerate() {
                match f {
                    "Start" => start_idx = i,
                    "End" => end_idx = i,
                    "Text" => text_idx = i,
                    _ => {}
                }
            }
            continue;
        }
        if trimmed.starts_with("Dialogue:") {
            let rest = &trimmed["Dialogue:".len()..];
            let parts: Vec<&str> = rest.splitn(text_idx + 1, ',').collect();
            if parts.len() <= text_idx {
                continue;
            }
            let start_vtt = ass_time_to_vtt(parts[start_idx].trim());
            let end_vtt = ass_time_to_vtt(parts[end_idx].trim());
            let clean_text = strip_ass_tags(parts[text_idx].trim());
            if clean_text.trim().is_empty() {
                continue;
            }
            result.push_str(&format!("{}\n{} --> {}\n{}\n\n", cue_number, start_vtt, end_vtt, clean_text));
            cue_number += 1;
        }
    }
    result.into_bytes()
}

async fn studio_subtitles_get(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::Value::Array(vec![]));
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Json(serde_json::Value::Array(vec![]));
            }
        }
        Err(_) => {
            return Json(serde_json::Value::Array(vec![]));
        }
    }

    let list_path = format!("source/{}/captions/list.txt", mediumid);
    if std::path::Path::new(&list_path).exists() {
        let labels: Vec<serde_json::Value> = read_lines_to_vec(&list_path)
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let entry = l.trim();
                let label = if let Some(dot) = entry.rfind('.') {
                    entry[..dot].to_string()
                } else {
                    entry.to_string()
                };
                serde_json::json!({ "label": label })
            })
            .collect();
        Json(serde_json::Value::Array(labels))
    } else {
        Json(serde_json::Value::Array(vec![]))
    }
}

async fn studio_subtitles_add(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    mut multipart: Multipart,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"not authorized\"}"))
                    .unwrap();
            }
        }
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let mut label = String::new();
    let mut file_content = Vec::new();
    let mut input_ext = String::from("vtt");

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "label" => {
                label = field.text().await.unwrap_or_default().trim().to_string();
            }
            "file" => {
                let filename = field.file_name().unwrap_or("").to_lowercase();
                if filename.ends_with(".srt") {
                    input_ext = "srt".to_string();
                } else if filename.ends_with(".ass") {
                    input_ext = "ass".to_string();
                }
                file_content = field.bytes().await.unwrap_or_default().to_vec();
            }
            _ => {}
        }
    }

    if label.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"label is required\"}"))
            .unwrap();
    }

    let sanitized_label: String = label
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect();

    if sanitized_label.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"invalid label\"}"))
            .unwrap();
    }

    if file_content.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"subtitle file is required\"}"))
            .unwrap();
    }

    // Convert to WebVTT if needed
    let vtt_content = match input_ext.as_str() {
        "srt" => convert_srt_to_vtt(&file_content),
        "ass" => convert_ass_to_vtt(&file_content),
        _ => file_content,
    };

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    let new_list_entry = format!("{}.vtt", sanitized_label);
    let subtitle_path = format!("{}/{}", captions_dir, new_list_entry);

    let list_path = format!("{}/list.txt", captions_dir);
    let mut existing: Vec<String> = if std::path::Path::new(&list_path).exists() {
        read_lines_to_vec(&list_path)
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect()
    } else {
        Vec::new()
    };

    // Remove any existing entries for this label
    let label_prefix = format!("{}.", sanitized_label);
    let old_entries: Vec<String> = existing
        .iter()
        .filter(|e| e.as_str() == sanitized_label || e.starts_with(&label_prefix))
        .cloned()
        .collect();

    for old_entry in &old_entries {
        let old_file_path = format!("{}/{}", captions_dir, old_entry);
        if old_file_path != subtitle_path {
            let _ = tokio::fs::remove_file(&old_file_path).await;
        }
    }

    existing.retain(|e| e.as_str() != sanitized_label && !e.starts_with(&label_prefix));
    existing.push(new_list_entry);

    if let Err(_) = tokio::fs::write(&subtitle_path, &vtt_content).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to write subtitle file\"}"))
            .unwrap();
    }

    let list_content = existing.join("\n") + "\n";
    if let Err(_) = tokio::fs::write(&list_path, list_content).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to update list\"}"))
            .unwrap();
    }

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}

async fn studio_subtitle_font_get(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::json!({ "exists": false }));
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) if record.owner == user_info.login => {}
        _ => return Json(serde_json::json!({ "exists": false })),
    }

    let font_path = format!("source/{}/captions/font.woff2", mediumid);
    let exists = std::path::Path::new(&font_path).exists();
    Json(serde_json::json!({ "exists": exists }))
}

async fn studio_subtitle_font_upload(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    mut multipart: Multipart,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"not authorized\"}"))
                    .unwrap();
            }
        }
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let mut font_content = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            font_content = field.bytes().await.unwrap_or_default().to_vec();
            break;
        }
    }

    if font_content.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"font file is required\"}"))
            .unwrap();
    }

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    let font_path = format!("{}/font.woff2", captions_dir);
    if let Err(_) = tokio::fs::write(&font_path, &font_content).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to write font file\"}"))
            .unwrap();
    }

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}

async fn studio_subtitle_font_delete(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"not authorized\"}"))
                    .unwrap();
            }
        }
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let font_path = format!("source/{}/captions/font.woff2", mediumid);
    let _ = tokio::fs::remove_file(&font_path).await;

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}

#[derive(Deserialize)]
struct SubtitleDeleteForm {
    label: String,
}

async fn studio_subtitles_delete(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Json(form): Json<SubtitleDeleteForm>,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let media_owner = sqlx::query!("SELECT owner FROM media WHERE id=$1;", mediumid)
        .fetch_one(&pool)
        .await;

    match media_owner {
        Ok(record) => {
            if record.owner != user_info.login {
                return Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"not authorized\"}"))
                    .unwrap();
            }
        }
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"media not found\"}"))
                .unwrap();
        }
    }

    let label = form.label.trim().to_string();
    if label.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"label is required\"}"))
            .unwrap();
    }

    let captions_dir = format!("source/{}/captions", mediumid);
    let list_path = format!("{}/list.txt", captions_dir);

    if std::path::Path::new(&list_path).exists() {
        let existing: Vec<String> = read_lines_to_vec(&list_path)
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect();

        let label_prefix = format!("{}.", label);
        let matched_entry = existing
            .iter()
            .find(|e| e.as_str() == label || e.starts_with(&label_prefix))
            .cloned();

        if let Some(entry) = matched_entry {
            let file_path = format!("{}/{}", captions_dir, entry);
            let _ = tokio::fs::remove_file(&file_path).await;

            let remaining: Vec<String> = existing.into_iter().filter(|e| e != &entry).collect();

            if remaining.is_empty() {
                let _ = tokio::fs::remove_file(&list_path).await;
            } else {
                let list_content = remaining.join("\n") + "\n";
                let _ = tokio::fs::write(&list_path, list_content).await;
            }
        }
    }

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}
