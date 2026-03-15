async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(db): Extension<Db>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let user = get_user_login(headers, &db, redis.clone()).await;
    let user_login = user.map(|u| u.login).unwrap_or_default();

    #[derive(Deserialize, SurrealValue)]
    struct MediumRow {
        id: RecordId,
        name: String,
        owner: String,
        views: i64,
        #[serde(rename = "type")]
        r#type: String,
    }

    let mut response = db
        .query(
            "SELECT id, name, owner, views, type FROM media \
             WHERE fn::visible_to(visibility, restricted_to_group, owner, $user) \
             LIMIT 20",
        )
        .bind(("user", user_login.clone()))
        .await
        .map_err(|_| {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to fetch recommendations".into())
                .unwrap()
        })?;

    let rows: Vec<MediumRow> = response.take(0).map_err(|_| {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to fetch recommendations".into())
            .unwrap()
    })?;

    let media: Vec<Medium> = rows
        .into_iter()
        .map(|row| Medium {
            id: row.id.key_string(),
            name: row.name,
            owner: row.owner,
            views: row.views,
            r#type: row.r#type,
            sprite_filename: None,
            sprite_x: 0,
            sprite_y: 0,
        })
        .collect();

    let template = HXMediumListTemplate {
        current_medium_id: mediumid,
        list_id: String::new(),
        media,
        config,
    };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}
