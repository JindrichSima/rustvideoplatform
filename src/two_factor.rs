// two_factor.rs – TOTP and WebAuthn (U2F) two-factor authentication handlers

use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;
use webauthn_rs::prelude::{
    Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential,
};

// ── helpers ─────────────────────────────────────────────────────────────────

/// Deterministic UUID for a user (avoids adding a uuid column to `users`).
fn user_uuid_from_login(login: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, login.as_bytes())
}

/// Generate a random 20-byte TOTP secret and return (raw bytes, base32 string).
fn generate_totp_secret() -> (Vec<u8>, String) {
    let bytes: Vec<u8> = (0..20).map(|_| rand::random::<u8>()).collect();
    let b32 = base32::encode(base32::Alphabet::RFC4648 { padding: false }, &bytes);
    (bytes, b32)
}

fn make_totp(
    secret_bytes: Vec<u8>,
    user_login: &str,
    instance_name: &str,
) -> Result<TOTP, totp_rs::TotpUrlError> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some(instance_name.to_owned()),
        user_login.to_owned(),
    )
}

/// Load all Passkey rows for a user from the database.
async fn load_passkeys(db: &Db, login: &str) -> Vec<(String, Passkey)> {
    #[derive(serde::Deserialize, SurrealValue)]
    struct PasskeyRow { id: RecordId, passkey: serde_json::Value }
    let mut resp = db
        .query("SELECT id, passkey FROM webauthn_credentials WHERE user_login = $user")
        .bind(("user", login.to_string()))
        .await
        .unwrap_or_else(|_| unreachable!());
    let rows: Vec<PasskeyRow> = resp.take(0).unwrap_or_default();
    rows.into_iter().filter_map(|row| {
        let id = row.id.key_string();
        let pk: Passkey = serde_json::from_value(row.passkey).ok()?;
        Some((id, pk))
    }).collect()
}

/// Build a JSON `axum::response::Response` with a given status code.
fn json_resp(status: StatusCode, body: serde_json::Value) -> axum::response::Response {
    use axum::response::IntoResponse;
    (status, axum::Json(body)).into_response()
}

// ── TOTP login verification ──────────────────────────────────────────────────

#[derive(Serialize, Deserialize, SurrealValue)]
struct TotpLoginForm {
    pending_token: String,
    totp_code: String,
}

async fn hx_login_2fa_totp(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Form(form): Form<TotpLoginForm>,
) -> (StatusCode, HeaderMap, String) {
    macro_rules! err_html {
        ($msg:expr) => {
            return (
                StatusCode::OK,
                HeaderMap::new(),
                format!("<b class=\"text-danger\">{}</b>", $msg),
            )
        };
    }

    // Look up pending login
    let login: Option<String> = redis
        .get(format!("pending_2fa:{}", form.pending_token))
        .await
        .ok();
    let login = match login {
        Some(l) if !l.is_empty() => l,
        _ => err_html!("Session expired. Please log in again."),
    };

    // Fetch TOTP secret
    #[derive(serde::Deserialize, SurrealValue)]
    struct TotpSecretRow { totp_secret: Option<String> }
    let mut _ts_resp = db
        .query("SELECT totp_secret FROM users WHERE id = $id AND totp_enabled = true")
        .bind(("id", RecordId::new("users", login.as_str())))
        .await
        .expect("db error");
    let totp_secret: Option<String> = _ts_resp.take::<Option<TotpSecretRow>>(0)
        .unwrap_or(None)
        .and_then(|r| r.totp_secret);

    let totp_secret = match totp_secret {
        Some(s) => s,
        None => err_html!("TOTP not configured."),
    };

    let secret_bytes = match Secret::Encoded(totp_secret).to_bytes() {
        Ok(b) => b,
        Err(_) => err_html!("Internal error (secret)."),
    };

    let totp = match make_totp(secret_bytes, &login, &config.instancename) {
        Ok(t) => t,
        Err(_) => err_html!("Internal error (totp)."),
    };

    if !totp.check_current(&form.totp_code).unwrap_or(false) {
        err_html!("Invalid code. Please try again.");
    }

    // Create real session
    let session_token = generate_secure_string();
    let session_restriction = match &config.custom_session_domain {
        Some(d) => format!("Path=/;Domain={}", d),
        None => "Path=/".to_owned(),
    };
    let _: () = redis
        .set(format!("session:{}", session_token), &login)
        .await
        .unwrap();
    let _: () = redis
        .del(format!("pending_2fa:{}", form.pending_token))
        .await
        .unwrap_or(());

    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        format!("session={}; {}", session_token, session_restriction)
            .parse()
            .unwrap(),
    );
    (
        StatusCode::OK,
        headers,
        "<b class=\"text-success\">Login successful!</b><script>window.location.replace('/');</script>".to_owned(),
    )
}

// ── TOTP settings ────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/hx-settings-2fa-totp-setup.html", escape = "none")]
struct HXSettings2FATotpSetupTemplate {
    qr_base64: String,
    secret_base32: String,
    totp_url: String,
    setup_token: String,
}

async fn hx_settings_2fa_totp_setup(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let (secret_bytes, secret_base32) = generate_totp_secret();
    let totp = match make_totp(secret_bytes, &user_info.login, &config.instancename) {
        Ok(t) => t,
        Err(_) => {
            return Html(minifi_html(
                "<b class=\"text-danger\">Failed to generate TOTP secret.</b>".to_owned(),
            ))
        }
    };

    let qr_base64 = match totp.get_qr_base64() {
        Ok(q) => q,
        Err(_) => {
            return Html(minifi_html(
                "<b class=\"text-danger\">Failed to generate QR code.</b>".to_owned(),
            ))
        }
    };
    let totp_url = totp.get_url();

    // Store pending setup (5-minute TTL)
    let setup_token = generate_secure_string();
    let _: () = redis
        .set_ex(
            format!("totp_setup:{}", setup_token),
            format!("{}:{}", user_info.login, secret_base32),
            300u64,
        )
        .await
        .unwrap_or(());

    let template = HXSettings2FATotpSetupTemplate {
        qr_base64,
        secret_base32,
        totp_url,
        setup_token,
    };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct TotpVerifySetupForm {
    setup_token: String,
    totp_code: String,
}

async fn hx_settings_2fa_totp_verify_setup(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    headers: HeaderMap,
    Form(form): Form<TotpVerifySetupForm>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let setup_data: Option<String> = redis
        .get(format!("totp_setup:{}", form.setup_token))
        .await
        .ok();
    let setup_data = match setup_data {
        Some(d) => d,
        None => {
            return Html(minifi_html(
                "<b class=\"text-danger\">Setup session expired. Please try again.</b>"
                    .to_owned(),
            ))
        }
    };

    let parts: Vec<&str> = setup_data.splitn(2, ':').collect();
    if parts.len() != 2 || parts[0] != user_info.login {
        return Html(minifi_html(
            "<b class=\"text-danger\">Invalid setup session.</b>".to_owned(),
        ));
    }
    let secret_base32 = parts[1].to_string();

    let secret_bytes = match Secret::Encoded(secret_base32.clone()).to_bytes() {
        Ok(b) => b,
        Err(_) => {
            return Html(minifi_html(
                "<b class=\"text-danger\">Invalid secret.</b>".to_owned(),
            ))
        }
    };

    let totp = match make_totp(secret_bytes, &user_info.login, &config.instancename) {
        Ok(t) => t,
        Err(_) => {
            return Html(minifi_html(
                "<b class=\"text-danger\">Failed to create TOTP.</b>".to_owned(),
            ))
        }
    };

    if !totp.check_current(&form.totp_code).unwrap_or(false) {
        return Html(minifi_html(
            "<b class=\"text-danger\">Invalid code. Check your authenticator and try again.</b>"
                .to_owned(),
        ));
    }

    let result = db
        .query("UPDATE users SET totp_secret = $secret, totp_enabled = true WHERE id = $id")
        .bind(("secret", secret_base32.clone()))
        .bind(("id", RecordId::new("users", user_info.login.as_str())))
        .await;

    if result.is_err() {
        return Html(minifi_html(
            "<b class=\"text-danger\">Failed to save TOTP configuration.</b>".to_owned(),
        ));
    }

    let _: () = redis
        .del(format!("totp_setup:{}", form.setup_token))
        .await
        .unwrap_or(());

    Html(minifi_html(
        "<b class=\"text-success\">TOTP authenticator enabled!</b>\
        <script>setTimeout(()=>{htmx.ajax('GET','/hx/settings/2fa',{target:'#tab-content',swap:'innerHTML'})},1500)</script>"
            .to_owned(),
    ))
}

async fn hx_settings_2fa_totp_disable(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let result =
        db
            .query("UPDATE users SET totp_secret = NULL, totp_enabled = false WHERE id = $id")
            .bind(("id", RecordId::new("users", user_info.login.as_str())))
            .await;

    if result.is_err() {
        return Html(minifi_html(
            "<b class=\"text-danger\">Failed to disable TOTP.</b>".to_owned(),
        ));
    }

    Html(minifi_html(
        "<b class=\"text-success\">TOTP disabled.</b>\
        <script>setTimeout(()=>{htmx.ajax('GET','/hx/settings/2fa',{target:'#tab-content',swap:'innerHTML'})},1500)</script>"
            .to_owned(),
    ))
}

// ── 2FA settings overview ────────────────────────────────────────────────────

struct WebauthnCredInfo {
    id: String,
    name: String,
    created: i64,
}

#[derive(Template)]
#[template(path = "pages/hx-settings-2fa.html", escape = "none")]
struct HXSettings2FATemplate {
    totp_enabled: bool,
    webauthn_creds: Vec<WebauthnCredInfo>,
    webauthn_available: bool,
}

async fn hx_settings_2fa(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    Extension(webauthn_lock): Extension<
        std::sync::Arc<std::sync::RwLock<Option<webauthn_rs::Webauthn>>>,
    >,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let totp_enabled: bool =
        {
            #[derive(serde::Deserialize, SurrealValue)] struct TotpEnabledRow2 { totp_enabled: bool }
            let mut _te_resp = db
                .query("SELECT totp_enabled FROM users WHERE id = $id")
                .bind(("id", RecordId::new("users", user_info.login.as_str())))
                .await
                .unwrap_or_else(|_| unreachable!());
            let _te_row: Option<TotpEnabledRow2> = _te_resp.take(0).unwrap_or(None);
            _te_row.map(|r| r.totp_enabled).unwrap_or(false)
        };

    #[derive(serde::Deserialize, SurrealValue)]
    struct WebauthnCredRow { id: String, credential_name: String, created: i64 }
    let mut _wc_resp = db
        .query("SELECT id, credential_name, created FROM webauthn_credentials WHERE user_login = $user ORDER BY created ASC")
        .bind(("user", user_info.login.clone()))
        .await
        .expect("db error");
    let webauthn_creds: Vec<WebauthnCredInfo> = _wc_resp
        .take::<Vec<WebauthnCredRow>>(0)
        .unwrap_or_default()
        .into_iter()
        .map(|r| WebauthnCredInfo { id: r.id, name: r.credential_name, created: r.created })
        .collect();

    let webauthn_available = webauthn_lock.read().map(|g| g.is_some()).unwrap_or(false);

    let template = HXSettings2FATemplate {
        totp_enabled,
        webauthn_creds,
        webauthn_available,
    };
    Html(minifi_html(template.render().unwrap()))
}

// ── WebAuthn registration ────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, SurrealValue)]
struct StartRegisterRequest {
    credential_name: Option<String>,
}

async fn hx_webauthn_register_start(
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Extension(webauthn_lock): Extension<
        std::sync::Arc<std::sync::RwLock<Option<webauthn_rs::Webauthn>>>,
    >,
    headers: HeaderMap,
    Json(req): Json<StartRegisterRequest>,
) -> axum::response::Response {
    let user_info = get_user_login(headers, &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return json_resp(StatusCode::UNAUTHORIZED, serde_json::json!({"error": "Not logged in"}));
    }
    let user_info = user_info.unwrap();

    // Load existing passkeys BEFORE acquiring the RwLock (await before lock)
    let existing = load_passkeys(&db, &user_info.login).await;
    let exclude_creds: Option<Vec<_>> = if existing.is_empty() {
        None
    } else {
        Some(existing.iter().map(|(_, pk)| pk.cred_id().clone()).collect())
    };

    let user_uuid = user_uuid_from_login(&user_info.login);
    let cred_name = req
        .credential_name
        .unwrap_or_else(|| "Security Key".to_owned());

    // Acquire lock, do sync webauthn work, drop lock – no awaits while held
    let webauthn_result = {
        let guard = webauthn_lock.read().unwrap();
        match guard.as_ref() {
            None => Err("WebAuthn not configured on this server".to_owned()),
            Some(w) => w
                .start_passkey_registration(
                    user_uuid,
                    &user_info.login,
                    &user_info.name,
                    exclude_creds,
                )
                .map_err(|e| format!("Registration start failed: {}", e)),
        }
    }; // lock dropped here

    let (ccr, reg_state) = match webauthn_result {
        Ok(r) => r,
        Err(e) => {
            let status = if e.contains("not configured") {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            return json_resp(status, serde_json::json!({"error": e}));
        }
    };

    // Awaits after lock is dropped
    let token = generate_secure_string();
    let state_json = serde_json::to_string(&reg_state).unwrap_or_default();
    let meta = serde_json::json!({
        "login": user_info.login,
        "cred_name": cred_name,
        "state": state_json,
    });
    let _: () = redis
        .set_ex(format!("webauthn_reg:{}", token), meta.to_string(), 300u64)
        .await
        .unwrap_or(());

    json_resp(
        StatusCode::OK,
        serde_json::json!({"token": token, "challenge": ccr}),
    )
}

async fn hx_webauthn_register_finish(
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Extension(webauthn_lock): Extension<
        std::sync::Arc<std::sync::RwLock<Option<webauthn_rs::Webauthn>>>,
    >,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    Json(reg_response): Json<RegisterPublicKeyCredential>,
) -> axum::response::Response {
    let user_info = get_user_login(headers, &db, redis.clone()).await;
    if !is_logged(user_info.clone()).await {
        return json_resp(StatusCode::UNAUTHORIZED, serde_json::json!({"error": "Not logged in"}));
    }
    let user_info = user_info.unwrap();

    let token = match params.get("token") {
        Some(t) => t.clone(),
        None => {
            return json_resp(StatusCode::BAD_REQUEST, serde_json::json!({"error": "Missing token"}))
        }
    };

    let meta_str: Option<String> = redis.get(format!("webauthn_reg:{}", token)).await.ok();
    let meta_str = match meta_str {
        Some(s) => s,
        None => {
            return json_resp(
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error": "Registration session expired"}),
            )
        }
    };

    let meta: serde_json::Value = match serde_json::from_str(&meta_str) {
        Ok(v) => v,
        Err(_) => {
            return json_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({"error": "Internal error"}),
            )
        }
    };

    let stored_login = meta["login"].as_str().unwrap_or_default();
    if stored_login != user_info.login {
        return json_resp(StatusCode::FORBIDDEN, serde_json::json!({"error": "Login mismatch"}));
    }

    let cred_name = meta["cred_name"]
        .as_str()
        .unwrap_or("Security Key")
        .to_owned();
    let state_str = meta["state"].as_str().unwrap_or_default();
    let reg_state: PasskeyRegistration = match serde_json::from_str(state_str) {
        Ok(s) => s,
        Err(_) => {
            return json_resp(
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({"error": "Failed to parse registration state"}),
            )
        }
    };

    // Sync webauthn work in a block – no awaits while guard is held
    let passkey_result = {
        let guard = webauthn_lock.read().unwrap();
        match guard.as_ref() {
            None => Err((StatusCode::SERVICE_UNAVAILABLE, "WebAuthn not configured".to_owned())),
            Some(w) => w
                .finish_passkey_registration(&reg_response, &reg_state)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Registration failed: {}", e))),
        }
    }; // lock dropped here

    let passkey = match passkey_result {
        Ok(p) => p,
        Err((status, msg)) => return json_resp(status, serde_json::json!({"error": msg})),
    };

    let record_id = generate_secure_string();
    let passkey_json = serde_json::to_value(&passkey).unwrap_or_default();
    let result = db
        .query("CREATE type::thing('webauthn_credentials', $id) SET user_login = $user, credential_name = $cname, passkey = $passkey, created = time::unix(time::now())")
        .bind(("id", record_id.clone()))
        .bind(("user", user_info.login.clone()))
        .bind(("cname", cred_name.clone()))
        .bind(("passkey", passkey_json.clone()))
        .await;

    let _: () = redis
        .del(format!("webauthn_reg:{}", token))
        .await
        .unwrap_or(());

    if result.is_err() {
        return json_resp(
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({"error": "Failed to save credential"}),
        );
    }

    json_resp(StatusCode::OK, serde_json::json!({"success": true}))
}

async fn hx_settings_2fa_webauthn_delete(
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(cred_id): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers, &db, redis).await;
    if !is_logged(user_info.clone()).await {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }
    let user_info = user_info.unwrap();

    let mut _del_resp = db
        .query("DELETE FROM webauthn_credentials WHERE id = $cid AND user_login = $user RETURN id")
        .bind(("cid", cred_id.clone()))
        .bind(("user", user_info.login.clone()))
        .await
        .expect("db error");
    let deleted: Vec<serde_json::Value> = _del_resp.take(0).unwrap_or_default();
    if deleted.is_empty() {
        return Html(minifi_html(
            "<b class=\"text-danger\">Credential not found or not yours.</b>".to_owned(),
        ));
    }

    Html(minifi_html(
        "<b class=\"text-success\">Security key removed.</b>\
        <script>setTimeout(()=>{htmx.ajax('GET','/hx/settings/2fa',{target:'#tab-content',swap:'innerHTML'})},1000)</script>"
            .to_owned(),
    ))
}

// ── WebAuthn authentication (passwordless login) ─────────────────────────────

#[derive(Serialize, Deserialize, SurrealValue)]
struct StartAuthRequest {
    username: String,
}

async fn hx_webauthn_auth_start(
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Extension(webauthn_lock): Extension<
        std::sync::Arc<std::sync::RwLock<Option<webauthn_rs::Webauthn>>>,
    >,
    Json(req): Json<StartAuthRequest>,
) -> axum::response::Response {
    let passkeys_with_ids = load_passkeys(&db, &req.username).await;
    if passkeys_with_ids.is_empty() {
        return json_resp(
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "No security keys registered for this user"}),
        );
    }

    let passkeys: Vec<Passkey> = passkeys_with_ids.into_iter().map(|(_, pk)| pk).collect();

    // Sync webauthn work – no awaits while guard is held
    let auth_result = {
        let guard = webauthn_lock.read().unwrap();
        match guard.as_ref() {
            None => Err((StatusCode::SERVICE_UNAVAILABLE, "WebAuthn not configured on this server".to_owned())),
            Some(w) => w
                .start_passkey_authentication(&passkeys)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Auth start failed: {}", e))),
        }
    }; // lock dropped here

    let (rcr, auth_state) = match auth_result {
        Ok(r) => r,
        Err((status, msg)) => return json_resp(status, serde_json::json!({"error": msg})),
    };

    let token = generate_secure_string();
    let state_json = serde_json::to_string(&auth_state).unwrap_or_default();
    let meta = serde_json::json!({
        "login": req.username,
        "state": state_json,
    });
    let _: () = redis
        .set_ex(format!("webauthn_auth:{}", token), meta.to_string(), 300u64)
        .await
        .unwrap_or(());

    json_resp(
        StatusCode::OK,
        serde_json::json!({"token": token, "challenge": rcr}),
    )
}

async fn hx_webauthn_auth_finish(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Extension(webauthn_lock): Extension<
        std::sync::Arc<std::sync::RwLock<Option<webauthn_rs::Webauthn>>>,
    >,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    Json(auth_response): Json<PublicKeyCredential>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    macro_rules! json_err {
        ($status:expr, $msg:expr) => {
            return json_resp($status, serde_json::json!({"error": $msg}))
        };
    }

    let token = match params.get("token") {
        Some(t) => t.clone(),
        None => json_err!(StatusCode::BAD_REQUEST, "Missing token"),
    };

    let meta_str: Option<String> = redis.get(format!("webauthn_auth:{}", token)).await.ok();
    let meta_str = match meta_str {
        Some(s) => s,
        None => json_err!(StatusCode::BAD_REQUEST, "Authentication session expired"),
    };

    let meta: serde_json::Value = match serde_json::from_str(&meta_str) {
        Ok(v) => v,
        Err(_) => json_err!(StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
    };

    let login = meta["login"].as_str().unwrap_or_default().to_owned();
    let state_str = meta["state"].as_str().unwrap_or_default();
    let auth_state: PasskeyAuthentication = match serde_json::from_str(state_str) {
        Ok(s) => s,
        Err(_) => json_err!(StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse auth state"),
    };

    // Sync webauthn work – no awaits while guard is held
    let finish_result = {
        let guard = webauthn_lock.read().unwrap();
        match guard.as_ref() {
            None => Err((StatusCode::SERVICE_UNAVAILABLE, "WebAuthn not configured".to_owned())),
            Some(w) => w
                .finish_passkey_authentication(&auth_response, &auth_state)
                .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Authentication failed: {}", e))),
        }
    }; // lock dropped here

    let auth_result = match finish_result {
        Ok(r) => r,
        Err((status, msg)) => return json_resp(status, serde_json::json!({"error": msg})),
    };

    // Update passkey counter
    let passkeys_with_ids = load_passkeys(&db, &login).await;
    for (record_id, mut pk) in passkeys_with_ids {
        if pk.update_credential(&auth_result) == Some(true) {
            let passkey_json = serde_json::to_value(&pk).unwrap_or_default();
            let _ = db
                .query("UPDATE webauthn_credentials SET passkey = $passkey WHERE id = $id")
                .bind(("passkey", passkey_json.clone()))
                .bind(("id", record_id.clone()))
                .await;
        }
    }

    let _: () = redis
        .del(format!("webauthn_auth:{}", token))
        .await
        .unwrap_or(());

    // Create session and set cookie
    let session_token = generate_secure_string();
    let session_restriction = match &config.custom_session_domain {
        Some(d) => format!("Path=/;Domain={}", d),
        None => "Path=/".to_owned(),
    };
    let _: () = redis
        .set(format!("session:{}", session_token), &login)
        .await
        .unwrap();

    let cookie = format!("session={}; {}", session_token, session_restriction);
    let mut response = (StatusCode::OK, axum::Json(serde_json::json!({"success": true})))
        .into_response();
    response
        .headers_mut()
        .insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());
    response
}
