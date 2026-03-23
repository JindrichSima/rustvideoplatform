async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(_redis): Extension<RedisConn>,
    _headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let mediumid = mediumid.to_ascii_lowercase();

    // Look up the owner of the current medium, then fetch their other public media.
    let owner: Option<String> = db
        .session
        .execute_unpaged(&db.get_media_owner, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(String,)>().ok().flatten())
        .map(|r| r.0);

    let media: Vec<Medium> = if let Some(ref owner) = owner {
        db.session
            .execute_unpaged(&db.get_media_by_owner, (owner, 21i64))
            .await
            .ok()
            .and_then(|r| r.into_rows_result().ok())
            .map(|rows| {
                rows.rows::<(String, String, Option<String>, i64, String, i64, String, Option<String>)>()
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .filter(|(id, _, _, _, _, _, visibility, _)| {
                        id != &mediumid && visibility == "public"
                    })
                    .take(20)
                    .map(|(id, name, _, views, media_type, _, _, _)| Medium {
                        id,
                        name,
                        owner: owner.clone(),
                        views,
                        r#type: media_type,
                        sprite_filename: None,
                        sprite_x: 0,
                        sprite_y: 0,
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

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
