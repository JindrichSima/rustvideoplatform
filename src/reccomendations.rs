async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();
    let group_ids = get_user_group_ids(&db, &user_login).await;

    let mut result = db
        .query("SELECT record::id(id) AS id, name, owner, views, type FROM media WHERE visibility = 'public' OR (visibility = 'restricted' AND restricted_to_group IN $groups) LIMIT 20")
        .bind(("groups", &group_ids))
        .await
        .map_err(|_| {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to fetch recommendations".into())
                .unwrap()
        })?;

    let media: Vec<Medium> = result.take(0).map_err(|_| {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to fetch recommendations".into())
            .unwrap()
    })?;

    let template = HXMediumListTemplate {
        current_medium_id: mediumid,
        media,
        config
    };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}
