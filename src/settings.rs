#[derive(Template)]
#[template(path = "pages/settings.html", escape = "none")]
struct SettingsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    active_tab: String,
}

async fn settings(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let sidebar = generate_sidebar(&config, "settings".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SettingsTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "channel_name".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_password(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let sidebar = generate_sidebar(&config, "settings".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SettingsTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "password".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_profile_picture(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let sidebar = generate_sidebar(&config, "settings".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SettingsTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "profile_picture".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_channel_picture(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let sidebar = generate_sidebar(&config, "settings".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SettingsTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "channel_picture".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn settings_diagnostics(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    if !is_logged(get_user_login(headers.clone(), &pool, redis.clone()).await).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let sidebar = generate_sidebar(&config, "settings".to_owned());
    let common_headers = extract_common_headers(&headers).unwrap();
    let template = SettingsTemplate {
        sidebar,
        config,
        common_headers,
        active_tab: "diagnostics".to_owned(),
    };
    Html(minifi_html(template.render().unwrap()))
}

// --- HTMX tab content handlers ---

#[derive(Template)]
#[template(path = "pages/hx-settings-channel-name.html", escape = "none")]
struct HXSettingsChannelNameTemplate {
    current_name: String,
}
async fn hx_settings_channel_name(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let template = HXSettingsChannelNameTemplate {
        current_name: user_info.name,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct ChannelNameForm {
    channel_name: String,
}
async fn hx_settings_channel_name_save(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<ChannelNameForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();
    let new_name = form.channel_name.trim();
    if new_name.is_empty() || new_name.len() > 100 {
        return Html(minifi_html("<b class=\"text-danger\">Channel name must be between 1 and 100 characters.</b>".to_owned()));
    }
    let result = sqlx::query("UPDATE users SET name=$1 WHERE login=$2;")
        .bind(new_name)
        .bind(&user_info.login)
        .execute(&pool)
        .await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update channel name.</b>".to_owned()));
    }
    Html(minifi_html(format!("<b class=\"text-success\">Channel name updated to \"{}\".</b>", askama::filters::escape(new_name, askama::filters::Html).unwrap())))
}

#[derive(Template)]
#[template(path = "pages/hx-settings-password.html", escape = "none")]
struct HXSettingsPasswordTemplate {}
async fn hx_settings_password(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let template = HXSettingsPasswordTemplate {};
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}
async fn hx_settings_password_save(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PasswordForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    if form.new_password != form.confirm_password {
        return Html(minifi_html("<b class=\"text-danger\">New passwords do not match.</b>".to_owned()));
    }
    if form.new_password.len() < 8 {
        return Html(minifi_html("<b class=\"text-danger\">New password must be at least 8 characters.</b>".to_owned()));
    }

    // Verify current password
    let stored = sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE login=$1")
        .bind(&user_info.login)
        .fetch_one(&pool)
        .await;
    if stored.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to verify current password.</b>".to_owned()));
    }
    let stored_hash = stored.unwrap();
    if Argon2::default()
        .verify_password(form.current_password.as_bytes(), &PasswordHash::new(&stored_hash).unwrap())
        .is_err()
    {
        return Html(minifi_html("<b class=\"text-danger\">Current password is incorrect.</b>".to_owned()));
    }

    // Hash new password with Argon2id
    let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let new_hash = Argon2::default()
        .hash_password(form.new_password.as_bytes(), &salt);
    if new_hash.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to hash new password.</b>".to_owned()));
    }
    let new_hash_string = new_hash.unwrap().to_string();

    let result = sqlx::query("UPDATE users SET password_hash=$1 WHERE login=$2;")
        .bind(&new_hash_string)
        .bind(&user_info.login)
        .execute(&pool)
        .await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update password.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Password updated successfully.</b>".to_owned()))
}

// --- Profile Picture ---

#[derive(Serialize, Deserialize)]
struct PictureMedium {
    id: String,
    name: String,
    visibility: String,
}
#[derive(Template)]
#[template(path = "pages/hx-settings-profile-picture.html", escape = "none")]
struct HXSettingsProfilePictureTemplate {
    media: Vec<PictureMedium>,
    current_picture: Option<String>,
    config: Config,
}
async fn hx_settings_profile_picture(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    let current_picture: Option<String> = sqlx::query_scalar("SELECT profile_picture FROM users WHERE login=$1")
        .bind(&user_info.login)
        .fetch_one(&pool)
        .await
        .unwrap_or(None);

    let media: Vec<PictureMedium> = sqlx::query(
        "SELECT id, name, visibility FROM media WHERE owner=$1 AND type='picture' AND (visibility='public' OR visibility='hidden') ORDER BY upload DESC;"
    )
    .bind(&user_info.login)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        PictureMedium {
            id: row.get("id"),
            name: row.get("name"),
            visibility: row.get("visibility"),
        }
    })
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let template = HXSettingsProfilePictureTemplate { media, current_picture, config };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize)]
struct PictureForm {
    medium_id: String,
}
async fn hx_settings_profile_picture_save(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PictureForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Verify the medium belongs to this user, is an image, and is public or hidden
    let medium = sqlx::query("SELECT owner, visibility, type FROM media WHERE id=$1")
        .bind(&form.medium_id)
        .fetch_one(&pool)
        .await;
    match medium {
        Ok(record) => {
            use sqlx::Row;
            let owner: String = record.get("owner");
            let visibility: String = record.get("visibility");
            let medium_type: String = record.get("type");
            if owner != user_info.login {
                return Html(minifi_html("<b class=\"text-danger\">You can only use your own media.</b>".to_owned()));
            }
            if medium_type != "picture" {
                return Html(minifi_html("<b class=\"text-danger\">Only image media can be used as a profile picture.</b>".to_owned()));
            }
            if visibility != "public" && visibility != "hidden" {
                return Html(minifi_html("<b class=\"text-danger\">Media must be public or hidden.</b>".to_owned()));
            }
        }
        Err(_) => {
            return Html(minifi_html("<b class=\"text-danger\">Medium not found.</b>".to_owned()));
        }
    }

    let result = sqlx::query("UPDATE users SET profile_picture=$1 WHERE login=$2;")
        .bind(&form.medium_id)
        .bind(&user_info.login)
        .execute(&pool)
        .await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update profile picture.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Profile picture updated.</b>".to_owned()))
}

// --- Channel Picture ---

#[derive(Template)]
#[template(path = "pages/hx-settings-channel-picture.html", escape = "none")]
struct HXSettingsChannelPictureTemplate {
    media: Vec<PictureMedium>,
    current_picture: Option<String>,
    config: Config,
}
async fn hx_settings_channel_picture(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    let current_picture: Option<String> = sqlx::query_scalar("SELECT channel_picture FROM users WHERE login=$1")
        .bind(&user_info.login)
        .fetch_one(&pool)
        .await
        .unwrap_or(None);

    let media: Vec<PictureMedium> = sqlx::query(
        "SELECT id, name, visibility FROM media WHERE owner=$1 AND type='picture' AND (visibility='public' OR visibility='hidden') ORDER BY upload DESC;"
    )
    .bind(&user_info.login)
    .map(|row: sqlx::postgres::PgRow| {
        use sqlx::Row;
        PictureMedium {
            id: row.get("id"),
            name: row.get("name"),
            visibility: row.get("visibility"),
        }
    })
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let template = HXSettingsChannelPictureTemplate { media, current_picture, config };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_settings_channel_picture_save(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<PictureForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }
    let user_info = user_info.unwrap();

    // Verify the medium belongs to this user, is an image, and is public or hidden
    let medium = sqlx::query("SELECT owner, visibility, type FROM media WHERE id=$1")
        .bind(&form.medium_id)
        .fetch_one(&pool)
        .await;
    match medium {
        Ok(record) => {
            use sqlx::Row;
            let owner: String = record.get("owner");
            let visibility: String = record.get("visibility");
            let medium_type: String = record.get("type");
            if owner != user_info.login {
                return Html(minifi_html("<b class=\"text-danger\">You can only use your own media.</b>".to_owned()));
            }
            if medium_type != "picture" {
                return Html(minifi_html("<b class=\"text-danger\">Only image media can be used as a channel picture.</b>".to_owned()));
            }
            if visibility != "public" && visibility != "hidden" {
                return Html(minifi_html("<b class=\"text-danger\">Media must be public or hidden.</b>".to_owned()));
            }
        }
        Err(_) => {
            return Html(minifi_html("<b class=\"text-danger\">Medium not found.</b>".to_owned()));
        }
    }

    let result = sqlx::query("UPDATE users SET channel_picture=$1 WHERE login=$2;")
        .bind(&form.medium_id)
        .bind(&user_info.login)
        .execute(&pool)
        .await;
    if result.is_err() {
        return Html(minifi_html("<b class=\"text-danger\">Failed to update channel picture.</b>".to_owned()));
    }
    Html(minifi_html("<b class=\"text-success\">Channel picture updated.</b>".to_owned()))
}

// --- Diagnostics ---

fn get_os_distro() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                return line[12..].trim_matches('"').to_owned();
            }
        }
    }
    std::env::consts::OS.to_owned()
}

fn get_kernel_version() -> String {
    let output = std::process::Command::new("uname")
        .arg("-r")
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8(o.stdout).unwrap_or_default().trim().to_owned();
            if v.is_empty() { "unknown".to_owned() } else { v }
        }
        _ => "unknown".to_owned(),
    }
}

#[derive(Template)]
#[template(path = "pages/hx-settings-diagnostics.html", escape = "none")]
struct HXSettingsDiagnosticsTemplate {
    git_commit: String,
    version: String,
    os_distro: String,
    os_kernel: String,
    os_arch: String,
}
async fn hx_settings_diagnostics(
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html("<script>window.location.replace(\"/login\");</script>".to_owned()));
    }

    let git_commit = env!("GIT_COMMIT_HASH").to_owned();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let os_distro = get_os_distro();
    let os_kernel = get_kernel_version();
    let os_arch = std::env::consts::ARCH.to_owned();

    let template = HXSettingsDiagnosticsTemplate {
        git_commit,
        version,
        os_distro,
        os_kernel,
        os_arch,
    };
    Html(minifi_html(template.render().unwrap()))
}
