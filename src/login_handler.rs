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

#[derive(Serialize, Deserialize)]
struct LoginForm {
    login: String,
    password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    login: String,
    name: String,
    profile_picture: Option<String>,
}

async fn hx_login(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(mut redis): Extension<RedisConn>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    let password_hash_get = sqlx::query!(
        "SELECT password_hash FROM users WHERE login=$1;",
        form.login
    )
    .fetch_one(&pool)
    .await;

    if password_hash_get.is_err() {
        let response_headers = HeaderMap::new();
        let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();

        return (StatusCode::OK, response_headers, response_body);
    }

    let password_hash = password_hash_get.unwrap().password_hash;

    if Argon2::default()
        .verify_password(
            form.password.as_bytes(),
            &PasswordHash::new(&password_hash).unwrap(),
        )
        .is_ok()
    {
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
        let response_headers = HeaderMap::new();
        let response_body = "<b class=\"text-danger\">Wrong user name or password</b>".to_owned();

        return (StatusCode::OK, response_headers, response_body);
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
