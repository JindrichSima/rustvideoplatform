async fn convert_subtitle_to_vtt(content: Vec<u8>, input_format: &str) -> Option<Vec<u8>> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel").arg("error")
        .arg("-f").arg(input_format)
        .arg("-i").arg("pipe:0")
        .arg("-f").arg("webvtt")
        .arg("pipe:1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&content).await.ok()?;
    }

    let output = child.wait_with_output().await.ok()?;
    if output.status.success() {
        Some(output.stdout)
    } else {
        None
    }
}

async fn studio_subtitles_get(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::Value::Array(vec![]));
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Json(serde_json::Value::Array(vec![])),
    }

    let list_path = format!("source/{}/captions/list.txt", mediumid);
    if std::path::Path::new(&list_path).exists() {
        let labels: Vec<serde_json::Value> = read_lines_to_vec(&list_path)
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let entry = l.trim();
                let is_ass = entry.ends_with(".ass") || entry.ends_with(".ssa");
                let label = if let Some(dot) = entry.rfind('.') {
                    entry[..dot].to_string()
                } else {
                    entry.to_string()
                };
                serde_json::json!({ "label": label, "format": if is_ass { "ass" } else { "vtt" } })
            })
            .collect();
        Json(serde_json::Value::Array(labels))
    } else {
        Json(serde_json::Value::Array(vec![]))
    }
}

async fn studio_subtitles_add(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    mut multipart: Multipart,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
    }

    let mut label = String::new();
    let mut file_content = Vec::new();
    let mut input_format = "webvtt";
    let mut is_ass_upload = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "label" => {
                label = field.text().await.unwrap_or_default().trim().to_string();
            }
            "file" => {
                let filename = field.file_name().unwrap_or("").to_lowercase();
                if filename.ends_with(".srt") {
                    input_format = "srt";
                } else if filename.ends_with(".ass") || filename.ends_with(".ssa") {
                    input_format = "ass";
                    is_ass_upload = true;
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

    // ASS/SSA files are stored natively to preserve styling; SRT is converted to VTT
    let (final_content, file_ext) = if is_ass_upload {
        (file_content, "ass")
    } else if input_format != "webvtt" {
        match convert_subtitle_to_vtt(file_content, input_format).await {
            Some(converted) => (converted, "vtt"),
            None => {
                return Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"subtitle conversion failed\"}"))
                    .unwrap();
            }
        }
    } else {
        (file_content, "vtt")
    };

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    let new_list_entry = format!("{}.{}", sanitized_label, file_ext);
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

    if let Err(_) = tokio::fs::write(&subtitle_path, &final_content).await {
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
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::json!({ "exists": false }));
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Json(serde_json::json!({ "exists": false })),
    }

    let font_path = format!("source/{}/captions/font.woff2", mediumid);
    let exists = std::path::Path::new(&font_path).exists();
    Json(serde_json::json!({ "exists": exists }))
}

async fn studio_subtitle_font_upload(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    mut multipart: Multipart,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
    }

    let mut font_content = Vec::new();
    let mut is_ttf = false;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "file" {
            let filename = field.file_name().unwrap_or("").to_lowercase();
            is_ttf = filename.ends_with(".ttf");
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

    let woff2_content = if is_ttf {
        use tokio::io::AsyncWriteExt;

        let mut child = match tokio::process::Command::new("woff2_compress")
            .arg("/dev/stdin")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                return Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"TTF to WOFF2 conversion failed\"}"))
                    .unwrap();
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            if stdin.write_all(&font_content).await.is_err() {
                return Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"TTF to WOFF2 conversion failed\"}"))
                    .unwrap();
            }
        } else {
            return Response::builder()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"TTF to WOFF2 conversion failed\"}"))
                .unwrap();
        }

        let status = child.wait().await;
        if !matches!(status, Ok(status) if status.success()) {
            return Response::builder()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"TTF to WOFF2 conversion failed\"}"))
                .unwrap();
        }

        let mut converted_path = std::env::temp_dir();
        converted_path.push(format!("stdin.woff2"));

        let converted = match tokio::fs::read(&converted_path).await {
            Ok(content) => content,
            Err(_) => {
                return Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"error\":\"TTF to WOFF2 conversion failed\"}"))
                    .unwrap();
            }
        };

        let _ = tokio::fs::remove_file(&converted_path).await;
        converted
    } else {
        font_content
    };

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    let font_path = format!("{}/font.woff2", captions_dir);
    if let Err(_) = tokio::fs::write(&font_path, &woff2_content).await {
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
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
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

async fn studio_subtitles_translate_status(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Json<serde_json::Value> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Json(serde_json::json!({ "in_progress": false }));
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => return Json(serde_json::json!({ "in_progress": false })),
    }

    let concept_id = format!("{}_translate", mediumid);
    let concept_row = db.session.execute_unpaged(&db.get_concept, (&concept_id,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String, String, String, bool)>().ok().flatten());

    let in_progress = match concept_row {
        Some((_id, _name, _type, processed)) => !processed,
        None => false,
    };

    Json(serde_json::json!({ "in_progress": in_progress }))
}

#[derive(Deserialize)]
struct SubtitleTranslateForm {
    source_label: String,
    target_language: String,
}

async fn studio_subtitles_translate(
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Json(form): Json<SubtitleTranslateForm>,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
                .unwrap();
        }
    }

    let source_label = form.source_label.trim().to_string();
    let target_language = form.target_language.trim().to_lowercase();

    if source_label.is_empty() || target_language.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"source_label and target_language are required\"}"))
            .unwrap();
    }

    // Verify source subtitle exists (only VTT subtitles can be used as translation source)
    let source_path = format!("source/{}/captions/{}.vtt", mediumid, source_label);
    let source_ass_path = format!("source/{}/captions/{}.ass", mediumid, source_label);
    if std::path::Path::new(&source_ass_path).exists() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"ASS/SSA subtitles cannot be used as translation source\"}"))
            .unwrap();
    }
    if !std::path::Path::new(&source_path).exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"source subtitle not found\"}"))
            .unwrap();
    }

    // Create a concept entry of type vtt_translate with metadata as the upload file
    let concept_id = format!("{}_translate", mediumid);

    let metadata = serde_json::json!({
        "medium_id": mediumid,
        "source_label": source_label,
        "target_language": target_language,
    });

    let upload_dir = std::path::Path::new("upload");
    let _ = tokio::fs::create_dir_all(upload_dir).await;
    let meta_path = upload_dir.join(&concept_id);

    if let Err(_) = tokio::fs::write(&meta_path, metadata.to_string()).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to create translation job\"}"))
            .unwrap();
    }

    // Upsert concept: delete any existing translation job for this medium, then insert new one
    let _ = db.session.execute_unpaged(&db.delete_concept, (&concept_id,)).await;
    let _ = db.session.execute_unpaged(&db.delete_concept_by_owner, (&user_info.login, &concept_id)).await;
    let _ = db.session.execute_unpaged(&db.delete_unprocessed_concept, (&concept_id,)).await;

    let concept_name = format!("Translate {} -> {}", source_label, target_language);
    let concept_type = "vtt_translate";

    let insert_result = db.session.execute_unpaged(
        &db.insert_concept,
        (&concept_id, &concept_name, &user_info.login, concept_type),
    ).await;

    if insert_result.is_err() {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to queue translation job\"}"))
            .unwrap();
    }

    let _ = db.session.execute_unpaged(
        &db.insert_concept_by_owner,
        (&user_info.login, &concept_id, &concept_name, concept_type),
    ).await;

    let _ = db.session.execute_unpaged(
        &db.insert_unprocessed_concept,
        (&concept_id, concept_type),
    ).await;

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
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
    Json(form): Json<SubtitleDeleteForm>,
) -> Response<Body> {
    let user_info = get_user_login(headers.clone(), &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"not logged in\"}"))
            .unwrap();
    }
    let user_info = user_info.unwrap();

    let owner_row = db.session.execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten());

    match owner_row {
        Some((owner,)) if owner == user_info.login => {}
        _ => {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"error\":\"not authorized\"}"))
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
