fn minifi_html(html: String) -> Vec<u8> {
    let cfg = minify_html_onepass::Cfg {
        minify_css: true,
        minify_js: true,
        ..Default::default()
    };

    minify_html_onepass::copy(html.as_bytes(), &cfg).unwrap()
}

fn read_lines_to_vec(filepath: &str) -> Vec<String> {
    let file = std::fs::File::open(filepath).unwrap();
    let reader = std::io::BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .collect();
    
    lines
}

fn generate_secure_string() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const STRING_LEN: usize = 100;

    let mut rng = rng();
    let secure_string: String = (0..STRING_LEN)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    secure_string
}

fn parse_cookie_header(header: &str) -> AHashMap<String, String> {
    let mut cookies = AHashMap::new();
    for cookie in header.split(';').map(|s| s.trim()) {
        let mut parts = cookie.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            cookies.insert(key.to_string(), value.to_string());
        }
    }
    cookies
}

async fn prettyunixtime(unix_time: i64) -> String {
    let dt: DateTime<Local> = DateTime::from_timestamp(unix_time, 0).unwrap().into();
    format!(
        "{}:{} {}/{} {}",
        dt.hour(),
        dt.minute(),
        dt.day(),
        dt.month(),
        dt.year()
    )
}

fn get_header_value(
    headers: &HeaderMap,
    header_name: axum::http::header::HeaderName,
) -> Option<String> {
    headers
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[derive(Serialize, Deserialize)]
struct CommonHeaders {
    host: String,
    user_agent: Option<String>,
    accept_language: Option<String>,
    cookie: Option<String>,
}
fn extract_common_headers(headers: &HeaderMap) -> Result<CommonHeaders, &'static str> {
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or("Missing or invalid 'Host' header")?;

    let user_agent = get_header_value(headers, USER_AGENT);
    let accept_language = get_header_value(headers, ACCEPT_LANGUAGE);
    let cookie = get_header_value(headers, COOKIE);

    Ok(CommonHeaders {
        host,
        user_agent,
        accept_language,
        cookie,
    })
}

async fn get_user_login(
    headers: HeaderMap,
    pool: &PgPool,
    session_store: Arc<Mutex<AHashMap<String, String>>>,
) -> Option<User> {
    let session_cookie = parse_cookie_header(headers.get("Cookie")?.to_str().ok()?)
        .get("session")?
        .to_owned();

    let session_store_guard = session_store.lock().await;
    let login = session_store_guard.get(&session_cookie)?;

    let name = sqlx::query!("SELECT name FROM users WHERE login=$1;", login)
        .fetch_one(pool)
        .await
        .ok()?
        .name;

    Some(User {
        login: login.to_owned(),
        name,
    })
}

async fn is_logged(user: Option<User>) -> bool {
    let isloggedin: bool;
    if user.is_some() && user.unwrap().login != "".to_owned() {
        isloggedin = true;
    }
    else {
        isloggedin = false;
    }
    isloggedin
}

fn format_file_size(size_bytes: usize) -> String {
    let size = size_bytes as f64;
    if size >= 1_000_000_000.0 {
        format!("{:.2} GB", size / 1_000_000_000.0)
    } else if size >= 1_000_000.0 {
        format!("{:.2} MB", size / 1_000_000.0)
    } else if size >= 1_000.0 {
        format!("{:.2} KB", size / 1_000.0)
    } else {
        format!("{} bytes", size_bytes)
    }
}

fn generate_medium_id() -> String {
    let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    let random_string: String = (0..10)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect();
    random_string
}

fn detect_medium_type_mime(mime: String) -> String {
    let result;
    let mime_type = mime.to_ascii_lowercase();
    if mime_type.contains("video") {
        result = "video";
    }
    else if mime_type.contains("audio") {
        result = "audio"
    }
    else if mime_type.contains("image") {
        result = "picture"
    }
    else {
        result = "other"
    }
    result.to_owned()
}

async fn copy_dir(src: &str, dest: &str) -> io::Result<()> {
    let src_path = std::path::Path::new(src);
    let dest_path = std::path::Path::new(dest);

    if !dest_path.exists() {
        fs::create_dir_all(dest_path).await?;
    }

    let mut entries = fs::read_dir(src_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_entry_path = entry.path();
        let dest_entry_path = dest_path.join(entry.file_name());

        if src_entry_path.is_dir() {
            Box::pin(copy_dir(
                src_entry_path.to_str().unwrap(),
                dest_entry_path.to_str().unwrap(),
            ))
            .await?;
        } else {
            fs::copy(&src_entry_path, &dest_entry_path).await?;
        }
    }

    Ok(())
}

async fn move_dir(src: &str, dest: &str) -> io::Result<()> {
    copy_dir(src, dest).await?;
    fs::remove_dir_all(src).await?;
    Ok(())
}