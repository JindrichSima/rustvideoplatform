async fn hx_recommended(
    Extension(config): Extension<Config>,
    Extension(db): Extension<ScyllaDb>,
    Extension(redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    // Recommendations will come from Meilisearch or the Redis cache in production.
    // There is no efficient ScyllaDB query for "random public media" without knowing an owner,
    // so we return an empty list here — the template handles empty gracefully.
    let media: Vec<Medium> = Vec::new();

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
