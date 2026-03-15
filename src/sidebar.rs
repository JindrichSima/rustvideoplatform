#[derive(Template)]
#[template(path = "pages/component-sidebar.html", escape = "none")]
struct SidebarComponentTemplate {
    config: Config,
    active_item: String,
}
fn generate_sidebar(config: &Config, active_item: String) -> String {
    let template = SidebarComponentTemplate {
        config: config.to_owned(),
        active_item,
    };
    template.render().unwrap()
}

#[derive(Template)]
#[template(path = "pages/hx-sidebar.html", escape = "none")]
struct HXSidebarTemplate {
    active_item: String,
}
async fn hx_sidebar(
    Extension(redis): Extension<RedisConn>,
    Extension(db): Extension<Db>,
    Path(active_item): Path<String>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    if user.is_some() {
        let template = HXSidebarTemplate {
            active_item,
        };
        Html(minifi_html(template.render().unwrap()))
    } else {
        Html("".as_bytes().to_vec())
    }
}
