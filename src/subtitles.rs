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
                let (label, format) = if entry.ends_with(".srt") {
                    (entry.trim_end_matches(".srt").to_string(), "srt".to_string())
                } else if entry.ends_with(".ass") {
                    (entry.trim_end_matches(".ass").to_string(), "ass".to_string())
                } else if entry.ends_with(".vtt") {
                    (entry.trim_end_matches(".vtt").to_string(), "vtt".to_string())
                } else {
                    (entry.to_string(), "vtt".to_string())
                };
                serde_json::json!({ "label": label, "format": format })
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
    let mut file_ext = String::from("vtt");

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "label" => {
                label = field.text().await.unwrap_or_default().trim().to_string();
            }
            "file" => {
                let filename = field.file_name().unwrap_or("").to_lowercase();
                if filename.ends_with(".srt") {
                    file_ext = "srt".to_string();
                } else if filename.ends_with(".ass") {
                    file_ext = "ass".to_string();
                } else {
                    file_ext = "vtt".to_string();
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

    // Sanitize label: only allow alphanumeric, hyphens, underscores, spaces
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

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    let new_list_entry = format!("{}.{}", sanitized_label, file_ext);
    let subtitle_path = format!("{}/{}", captions_dir, new_list_entry);

    // Update list.txt - read existing entries
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

    // Remove any existing entries for this label (old format or different extension)
    let label_prefix = format!("{}.", sanitized_label);
    let old_entries: Vec<String> = existing
        .iter()
        .filter(|e| {
            let e_str = e.as_str();
            e_str == sanitized_label || e_str.starts_with(&label_prefix)
        })
        .cloned()
        .collect();

    for old_entry in &old_entries {
        let old_filename = if old_entry.contains('.') {
            old_entry.clone()
        } else {
            format!("{}.vtt", old_entry)
        };
        let old_file_path = format!("{}/{}", captions_dir, old_filename);
        // Delete old file only if it differs from the new one
        if old_file_path != subtitle_path {
            let _ = tokio::fs::remove_file(&old_file_path).await;
        }
    }

    existing.retain(|e| {
        let e_str = e.as_str();
        e_str != sanitized_label && !e_str.starts_with(&label_prefix)
    });
    existing.push(new_list_entry);

    // Write the subtitle file
    if let Err(_) = tokio::fs::write(&subtitle_path, &file_content).await {
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

        // Find the matching entry by label (with or without extension)
        let label_prefix = format!("{}.", label);
        let matched_entry = existing
            .iter()
            .find(|e| {
                let e_str = e.as_str();
                e_str == label || e_str.starts_with(&label_prefix)
            })
            .cloned();

        if let Some(entry) = matched_entry {
            // Determine the actual filename to delete
            let filename = if entry.contains('.') {
                entry.clone()
            } else {
                format!("{}.vtt", entry)
            };
            let file_path = format!("{}/{}", captions_dir, filename);
            let _ = tokio::fs::remove_file(&file_path).await;

            let remaining: Vec<String> = existing
                .into_iter()
                .filter(|e| e != &entry)
                .collect();

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
