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
            .map(|l| serde_json::json!({ "label": l.trim() }))
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
    let mut vtt_content = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "label" => {
                label = field.text().await.unwrap_or_default().trim().to_string();
            }
            "file" => {
                vtt_content = field.bytes().await.unwrap_or_default().to_vec();
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

    if vtt_content.is_empty() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"VTT file is required\"}"))
            .unwrap();
    }

    let captions_dir = format!("source/{}/captions", mediumid);
    let _ = tokio::fs::create_dir_all(&captions_dir).await;

    // Write the VTT file
    let vtt_path = format!("{}/{}.vtt", captions_dir, sanitized_label);
    if let Err(_) = tokio::fs::write(&vtt_path, &vtt_content).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from("{\"error\":\"failed to write VTT file\"}"))
            .unwrap();
    }

    // Update list.txt - read existing, add if not present, write back
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

    if !existing.contains(&sanitized_label) {
        existing.push(sanitized_label.clone());
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
    let vtt_path = format!("{}/{}.vtt", captions_dir, label);

    // Remove the VTT file
    let _ = tokio::fs::remove_file(&vtt_path).await;

    // Update list.txt
    if std::path::Path::new(&list_path).exists() {
        let existing: Vec<String> = read_lines_to_vec(&list_path)
            .into_iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .filter(|l| l != &label)
            .collect();

        if existing.is_empty() {
            let _ = tokio::fs::remove_file(&list_path).await;
        } else {
            let list_content = existing.join("\n") + "\n";
            let _ = tokio::fs::write(&list_path, list_content).await;
        }
    }

    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"ok\":true}"))
        .unwrap()
}
