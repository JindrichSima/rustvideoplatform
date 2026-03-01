async fn hx_new_view(
    Extension(db): Extension<Db>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    #[derive(Deserialize)]
    struct ViewsRow { views: i64 }

    let mut result = db
        .query("UPDATE type::thing('media', $id) SET views += 1")
        .bind(("id", &mediumid))
        .await
        .expect("Database error");

    let row: Option<ViewsRow> = result.take(0).unwrap_or(None);
    Html(row.map(|r| r.views.to_string()).unwrap_or("0".to_owned()))
}
