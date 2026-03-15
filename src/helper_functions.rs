trait RecordIdKeyExt {
    fn key_string(&self) -> String;
}

impl RecordIdKeyExt for RecordId {
    fn key_string(&self) -> String {
        use surrealdb::types::RecordIdKey;
        match &self.key {
            RecordIdKey::String(s) => s.clone(),
            RecordIdKey::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }
}

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

    (0..STRING_LEN)
        .map(|_| {
            let idx = rand::random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
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

#[derive(Serialize, Deserialize, SurrealValue)]
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

#[derive(Serialize, Deserialize, SurrealValue, Debug)]
struct UserRow {
    name: String,
    profile_picture: Option<String>,
}

async fn get_user_login(
    headers: HeaderMap,
    db: &Db,
    mut redis: RedisConn,
) -> Option<User> {
    let session_cookie = parse_cookie_header(headers.get("Cookie")?.to_str().ok()?)
        .get("session")?
        .to_owned();

    let login: String = redis
        .get(format!("session:{}", session_cookie))
        .await
        .ok()?;

    let mut resp = db
        .query("SELECT name, profile_picture FROM users WHERE id = $id")
        .bind(("id", RecordId::new("users", login.as_str())))
        .await
        .ok()?;

    let row: Option<UserRow> = resp.take(0).ok()?;
    let row = row?;

    Some(User {
        login,
        name: row.name,
        profile_picture: row.profile_picture,
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

    (0..10)
        .map(|_| {
            let idx = rand::random_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

fn detect_medium_type_mime(mime: String) -> String {
    let mime_type = mime.to_ascii_lowercase();

    if mime_type.contains("video")
        || matches!(
            mime_type.as_str(),
            "application/x-matroska" | "application/ogg"
        )
    {
        return "video".to_owned();
    }

    if mime_type.contains("audio")
        || matches!(
            mime_type.as_str(),
            "application/ogg"
                | "application/x-ogg"
                | "application/flac"
                | "application/x-flac"
                | "application/mp4"
        )
    {
        return "audio".to_owned();
    }

    if mime_type.contains("image")
        || matches!(mime_type.as_str(), "application/dicom")
    {
        return "picture".to_owned();
    }

    if mime_type == "application/pdf" {
        return "document_pdf".to_owned();
    }

    if matches!(
        mime_type.as_str(),
        "application/vnd.oasis.opendocument.text"
            | "application/vnd.oasis.opendocument.text-template"
            | "application/msword"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
            | "application/vnd.apple.pages"
            | "application/rtf"
            | "text/rtf"
    ) {
        return "document_writer".to_owned();
    }

    if matches!(
        mime_type.as_str(),
        "application/vnd.oasis.opendocument.spreadsheet"
            | "application/vnd.oasis.opendocument.spreadsheet-template"
            | "application/vnd.ms-excel"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
            | "application/vnd.apple.numbers"
    ) {
        return "document_spreadsheet".to_owned();
    }

    if matches!(
        mime_type.as_str(),
        "application/vnd.oasis.opendocument.presentation"
            | "application/vnd.oasis.opendocument.presentation-template"
            | "application/vnd.ms-powerpoint"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.openxmlformats-officedocument.presentationml.template"
            | "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
            | "application/vnd.apple.keynote"
    ) {
        return "document_presentation".to_owned();
    }

    "other".to_owned()
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

const SYSTEM_GROUP_ALL_REGISTERED: &str = "__all_registered__";
const SYSTEM_GROUP_SUBSCRIBERS: &str = "__subscribers__";

fn is_system_group(group_id: &str) -> bool {
    group_id == SYSTEM_GROUP_ALL_REGISTERED || group_id == SYSTEM_GROUP_SUBSCRIBERS
}

fn system_groups_for_owner(owner: &str) -> Vec<UserGroup> {
    vec![
        UserGroup {
            id: SYSTEM_GROUP_ALL_REGISTERED.to_owned(),
            name: "All Registered Users".to_owned(),
            owner: owner.to_owned(),
        },
        UserGroup {
            id: SYSTEM_GROUP_SUBSCRIBERS.to_owned(),
            name: "Subscribers Only".to_owned(),
            owner: owner.to_owned(),
        },
    ]
}

async fn is_subscribed(db: &Db, subscriber: &str, target: &str) -> bool {
    let mut resp = db
        .query("SELECT id FROM subscriptions WHERE subscriber = $subscriber AND target = $target LIMIT 1")
        .bind(("subscriber", subscriber.to_string()))
        .bind(("target", target.to_string()))
        .await
        .unwrap_or_else(|_| unreachable!());
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap_or_default();
    !rows.is_empty()
}

async fn is_group_member(db: &Db, group_id: &str, user_login: &str, mut redis: RedisConn) -> bool {
    let redis_key = format!("group:{}:members", group_id);

    let key_exists: bool = redis.exists(&redis_key).await.unwrap_or(false);

    if key_exists {
        return redis.sismember(&redis_key, user_login).await.unwrap_or(false);
    }

    #[derive(Deserialize, SurrealValue)]
    struct MemberRow { user_login: String }

    let mut resp = db
        .query("SELECT user_login FROM user_group_members WHERE group_id = $group_id")
        .bind(("group_id", group_id.to_string()))
        .await
        .unwrap_or_else(|_| unreachable!());
    let members: Vec<MemberRow> = resp.take(0).unwrap_or_default();
    let member_logins: Vec<String> = members.into_iter().map(|m| m.user_login).collect();

    let is_member = member_logins.contains(&user_login.to_owned());

    if !member_logins.is_empty() {
        let _: Result<(), _> = redis.sadd(&redis_key, &member_logins).await;
        let _: Result<(), _> = redis.expire(&redis_key, 3600).await;
    }

    is_member
}

async fn can_access_restricted(db: &Db, visibility: &str, restricted_to_group: Option<&str>, owner: &str, user: &Option<User>, redis: RedisConn) -> bool {
    match visibility {
        "public" => true,
        "restricted" => {
            if let Some(u) = user {
                if u.login == owner {
                    return true;
                }
                if let Some(group_id) = restricted_to_group {
                    if group_id == SYSTEM_GROUP_ALL_REGISTERED {
                        return true;
                    }
                    if group_id == SYSTEM_GROUP_SUBSCRIBERS {
                        return is_subscribed(db, &u.login, owner).await;
                    }
                    return is_group_member(db, group_id, &u.login, redis).await;
                }
            }
            false
        }
        _ => true
    }
}
