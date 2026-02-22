async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let media: Vec<Medium> = sqlx::query_as!(
        Medium,
        "SELECT id, name, owner, views, type FROM media WHERE public = true LIMIT 20;"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| {
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
