async fn hx_new_view(
    Extension(db): Extension<ScyllaDb>,
    Extension(mut redis): Extension<RedisConn>,
    headers: HeaderMap,
    Path(mediumid): Path<String>,
) -> axum::response::Html<String> {
    // Increment counter in ScyllaDB
    let _ = db.session.execute_unpaged(&db.increment_view_count, (&mediumid,)).await;

    // Record per-user view history (deduplicated with 1-hour Redis key)
    if let Some(user) = get_user_login(headers, &db, redis.clone()).await {
        let dedup_key = format!("viewhist:{}:{}", user.login, mediumid);
        let already: bool = redis.exists(&dedup_key).await.unwrap_or(false);
        if !already {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let _ = db.session.execute_unpaged(&db.insert_view_history, (&user.login, &now, &mediumid)).await;
            let _: Result<(), _> = redis.set_ex(&dedup_key, 1i32, 3600).await;
        }
    }

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
