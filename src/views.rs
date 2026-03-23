async fn hx_new_view(
    Extension(db): Extension<ScyllaDb>,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    // Increment counter in ScyllaDB
    let _ = db.session.execute_unpaged(&db.increment_view_count, (&mediumid,)).await;

    // Also update the main media table views (read counter, update main)
    let views: i64 = db.session.execute_unpaged(&db.get_view_count, (&mediumid,))
        .await
        .ok()
        .and_then(|r| r.into_rows_result().ok())
        .and_then(|rows| rows.maybe_first_row::<(scylla::value::CqlValue,)>().ok().flatten())
        .map(|r| match r.0 {
            scylla::value::CqlValue::Counter(c) => c.0,
            _ => 0,
        })
        .unwrap_or(0);

    Html(views.to_string())
}
