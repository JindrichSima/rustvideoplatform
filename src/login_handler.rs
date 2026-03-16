#[derive(Template)]
#[template(path = "pages/login.html", escape = "none")]
struct LoginTemplate {
    config: Config,
    common_headers: CommonHeaders,
}
async fn login(
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let common_headers = extract_common_headers(&headers).unwrap_or(CommonHeaders {
        host: String::new(),
        user_agent: None,
        accept_language: None,
        cookie: None,
    });
    let template = LoginTemplate { config, common_headers };
    Html(minifi_html(template.render().unwrap()))
}

#[derive(Serialize, Deserialize, SurrealValue)]
struct LoginForm {
    login: String,
    password: String,
}

#[derive(Serialize, Deserialize, SurrealValue, Clone, Debug)]
struct User {
    login: String,
    name: String,
    profile_picture: Option<String>,
}

#[derive(Deserialize, SurrealValue)]
struct UserAuthRow {
    password_hash: String,
    totp_enabled: Option<bool>,
}

async fn hx_login(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(mut redis): Extension<RedisConn>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Single query fetches both password_hash and totp_enabled
    let mut resp = match db
        .query("SELECT password_hash, totp_enabled FROM users WHERE id = $id")
        .bind(("id", RecordId::new("users", form.login.as_str())))
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return (StatusCode::OK, HeaderMap::new(), "<b class=\"text-danger\">Wrong user name or password</b>".to_owned());
        }
    };

    let row: Option<UserAuthRow> = match resp.take(0) {
        Ok(r) => r,
        Err(_) => None,
    };

    let auth_row = match row {
        Some(r) => r,
        None => {
            return (StatusCode::OK, HeaderMap::new(), "<b class=\"text-danger\">Wrong user name or password</b>".to_owned());
        }
    };

    if Argon2::default()
        .verify_password(
            form.password.as_bytes(),
            &PasswordHash::new(&auth_row.password_hash).unwrap(),
        )
        .is_ok()
    {
        let totp_enabled = auth_row.totp_enabled.unwrap_or(false);

        if totp_enabled {
            let pending_token = generate_secure_string();
            let _: () = redis
                .set_ex(
                    format!("pending_2fa:{}", pending_token),
                    &form.login,
                    300u64,
                )
                .await
                .unwrap_or(());

            let response_body = "<p class=\"mt-2\">Enter your authenticator code:</p>\
                <form hx-post=\"/hx/login/2fa/totp\" hx-target=\"#logininfo\" hx-swap=\"innerHTML\">\
                <input type=\"hidden\" name=\"pending_token\" value=\"".to_owned()
                + &pending_token
                + "\">\
                <input class=\"form-control\" style=\"text-align:center\" type=\"text\" name=\"totp_code\"\
                 placeholder=\"000000\" maxlength=\"6\" pattern=\"[0-9]{6}\" autocomplete=\"one-time-code\"\
                 autofocus inputmode=\"numeric\">\
                <button class=\"btn btn-primary mt-3 w-100\" type=\"submit\">Verify Code</button>\
                </form>";
            return (StatusCode::OK, HeaderMap::new(), response_body);
        }

        let session_cookie_value = generate_secure_string();
        let session_restriction: String;
        if config.custom_session_domain.is_some() {
            session_restriction =
                format!("Path=/;Domain={}", config.custom_session_domain.clone().unwrap());
        } else {
            session_restriction = "Path=/".to_owned()
        }
        let session_cookie_set = format!("session={}; {}", session_cookie_value, session_restriction);
        let _: () = redis
            .set(format!("session:{}", session_cookie_value), &form.login)
            .await
            .unwrap();

        let mut response_headers = HeaderMap::new();
        response_headers.insert("Set-Cookie", session_cookie_set.parse().unwrap());
        let response_body = "<b class=\"text-sucess\">LOGIN SUCESS</b><script>window.location.replace(\"/\");</script>".to_owned();
        return (StatusCode::OK, response_headers, response_body);
    } else {
        return (StatusCode::OK, HeaderMap::new(), "<b class=\"text-danger\">Wrong user name or password</b>".to_owned());
    }
}

async fn hx_logout(
    headers: HeaderMap,
    Extension(mut redis): Extension<RedisConn>,
) -> axum::response::Html<String> {
    let session_cookie = parse_cookie_header(headers.get("Cookie").unwrap().to_str().unwrap())
        .get("session")
        .unwrap()
        .to_owned();
    let _: () = redis.del(format!("session:{}", session_cookie)).await.unwrap();
    Html("<h1>LOGOUT SUCESS</h1><script>window.location.replace(\"/\");</script>".to_owned())
}
