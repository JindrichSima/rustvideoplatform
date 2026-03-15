#[derive(Deserialize, SurrealValue)]
struct ViewsRow {
    views: i64,
}

async fn hx_new_view(
    Extension(db): Extension<Db>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    let mut resp = db
        .query("UPDATE media SET views += 1 WHERE id = $id RETURN views")
        .bind(("id", RecordId::new("media", mediumid.as_str())))
        .await
        .expect("Database error");
    let rows: Vec<ViewsRow> = resp.take(0).unwrap_or_default();
    let views = rows.first().map(|r| r.views).unwrap_or(0);
    Html(views.to_string())
}
