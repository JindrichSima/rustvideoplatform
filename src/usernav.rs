#[derive(Template)]
#[template(path = "pages/hx-usernav.html", escape = "none")]
struct HXUsernavTemplate {
    user: User,
    config: Config,
    user_theme: String,
}
async fn hx_usernav(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let try_user = get_user_login(headers, &pool, redis.clone()).await;
    if try_user.is_some() {
        let user = try_user.unwrap();
        let user_theme = user.theme.clone().unwrap_or_else(|| "default".to_owned());
        let template = HXUsernavTemplate { user, config, user_theme };
        return Html(minifi_html(template.render().unwrap()));
    } else {
        let result = format!("<a href=\"/login\"><button class=\"btn text-white\"><i class=\"fa-solid fa-user mx-2\" preload=\"mouseover\"></i>Log in</button></a>");
        return Html(minifi_html(result));
    }
}