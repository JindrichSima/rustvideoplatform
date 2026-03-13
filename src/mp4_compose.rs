async fn compose_mp4_sm(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    let medium_id = mediumid.to_ascii_lowercase();

    // Verify the medium exists and is a video, and that the user has access
    let medium_row = sqlx::query(
        "SELECT m.id, m.name, m.type, m.visibility, m.restricted_to_group, m.owner FROM media m WHERE m.id=$1;"
    )
    .bind(&medium_id)
    .fetch_optional(&pool)
    .await;

    let medium = match medium_row {
        Ok(Some(row)) => row,
        _ => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap();
        }
    };

    use sqlx::Row;
    let medium_type: String = medium.get("type");
    if medium_type != "video" {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }

    let visibility: String = medium.get("visibility");
    let restricted_to_group: Option<String> = medium.get("restricted_to_group");
    let owner: String = medium.get("owner");
    let user = get_user_login(headers, &pool, redis.clone()).await;

    if !can_access_restricted(&pool, &visibility, restricted_to_group.as_deref(), &owner, &user, redis.clone()).await {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::empty())
            .unwrap();
    }

    // Determine input source: prefer HLS (m3u8), fall back to DASH (mpd)
    let m3u8_path = format!("source/{}/video/video.m3u8", medium_id);
    let mpd_path = format!("source/{}/video/video.mpd", medium_id);

    let (input_path, is_hls) = if std::path::Path::new(&m3u8_path).exists() {
        (m3u8_path, true)
    } else if std::path::Path::new(&mpd_path).exists() {
        (mpd_path, false)
    } else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    };

    // Build ffmpeg command to transcode to lowest possible quality for Open Graph Protocol
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.arg("-hide_banner")
        .arg("-loglevel").arg("error");

    if is_hls {
        cmd.arg("-protocol_whitelist").arg("file,pipe,crypto,data")
            .arg("-allowed_extensions").arg("ALL");
    }

    cmd.arg("-i").arg(&input_path)
        .arg("-vf").arg("scale=480:-2")
        .arg("-c:v").arg("libx264")
        .arg("-crf").arg("28")
        .arg("-preset").arg("fast")
        .arg("-c:a").arg("aac")
        .arg("-b:a").arg("64k")
        .arg("-movflags").arg("frag_keyframe+empty_moov")
        .arg("-f").arg("mp4")
        .arg("pipe:1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to start video composition"))
                .unwrap();
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stream = tokio_util::io::ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    // Wait for the child process in the background to avoid zombies
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    // Build a safe filename from the medium name
    let medium_name: String = medium.get("name");
    let safe_filename: String = medium_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    Response::builder()
        .header("Content-Type", "video/mp4")
        .header(
            "Content-Disposition",
            format!("inline; filename=\"{}-sm.mp4\"", safe_filename),
        )
        .body(body)
        .unwrap()
}

async fn compose_mp4(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Response<Body> {
    let medium_id = mediumid.to_ascii_lowercase();

    // Verify the medium exists and is a video, and that the user has access
    let medium_row = sqlx::query(
        "SELECT m.id, m.name, m.type, m.visibility, m.restricted_to_group, m.owner FROM media m WHERE m.id=$1;"
    )
    .bind(&medium_id)
    .fetch_optional(&pool)
    .await;

    let medium = match medium_row {
        Ok(Some(row)) => row,
        _ => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap();
        }
    };

    use sqlx::Row;
    let medium_type: String = medium.get("type");
    if medium_type != "video" {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    }

    let visibility: String = medium.get("visibility");
    let restricted_to_group: Option<String> = medium.get("restricted_to_group");
    let owner: String = medium.get("owner");
    let user = get_user_login(headers, &pool, redis.clone()).await;

    if !can_access_restricted(&pool, &visibility, restricted_to_group.as_deref(), &owner, &user, redis.clone()).await {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::empty())
            .unwrap();
    }

    // Determine input source: prefer HLS (m3u8), fall back to DASH (mpd)
    let m3u8_path = format!("source/{}/video/video.m3u8", medium_id);
    let mpd_path = format!("source/{}/video/video.mpd", medium_id);

    let (input_path, is_hls) = if std::path::Path::new(&m3u8_path).exists() {
        (m3u8_path, true)
    } else if std::path::Path::new(&mpd_path).exists() {
        (mpd_path, false)
    } else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap();
    };

    // Build ffmpeg command to remux DASH/HLS CMAF segments into a streamable MP4
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.arg("-hide_banner")
        .arg("-loglevel").arg("error");

    if is_hls {
        cmd.arg("-protocol_whitelist").arg("file,pipe,crypto,data")
            .arg("-allowed_extensions").arg("ALL");
    }

    cmd.arg("-i").arg(&input_path)
        .arg("-c").arg("copy")
        .arg("-movflags").arg("frag_keyframe+empty_moov")
        .arg("-f").arg("mp4")
        .arg("pipe:1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to start video composition"))
                .unwrap();
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stream = tokio_util::io::ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    // Wait for the child process in the background to avoid zombies
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    // Build a safe filename from the medium name
    let medium_name: String = medium.get("name");
    let safe_filename: String = medium_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    Response::builder()
        .header("Content-Type", "video/mp4")
        .header(
            "Content-Disposition",
            format!("inline; filename=\"{}.mp4\"", safe_filename),
        )
        .body(body)
        .unwrap()
}
