async fn sitemap_xml(
    Extension(pool): Extension<PgPool>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let host = headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let base = format!("https://{}", host);

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    // Home
    xml.push_str(&format!(
        "  <url><loc>{}/</loc><changefreq>daily</changefreq><priority>1.0</priority></url>\n",
        base
    ));
    // Trending
    xml.push_str(&format!(
        "  <url><loc>{}/trending</loc><changefreq>daily</changefreq><priority>0.8</priority></url>\n",
        base
    ));
    // Search
    xml.push_str(&format!(
        "  <url><loc>{}/search</loc><changefreq>weekly</changefreq><priority>0.5</priority></url>\n",
        base
    ));

    // All users
    let users: Vec<String> = sqlx::query_scalar("SELECT login FROM users ORDER BY login")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    for login in &users {
        let escaped = html_escape(login);
        xml.push_str(&format!(
            "  <url><loc>{}/u/{}</loc><changefreq>weekly</changefreq><priority>0.6</priority></url>\n",
            base, escaped
        ));
    }

    // All public videos/media
    let media: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM media WHERE visibility = 'public' ORDER BY upload DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (id,) in &media {
        let escaped = html_escape(id);
        xml.push_str(&format!(
            "  <url><loc>{}/m/{}</loc><changefreq>monthly</changefreq><priority>0.7</priority></url>\n",
            base, escaped
        ));
    }

    // All public lists
    let lists: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM lists WHERE visibility = 'public' ORDER BY created DESC",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (id,) in &lists {
        let escaped = html_escape(id);
        xml.push_str(&format!(
            "  <url><loc>{}/l/{}</loc><changefreq>weekly</changefreq><priority>0.6</priority></url>\n",
            base, escaped
        ));
    }

    xml.push_str("</urlset>\n");

    let _ = config; // suppress unused warning

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
