rustvideoplatform/src/playlists.rs
```rust
#[derive(Template)]
#[template(path = "pages/playlists.html", escape = "none")]
struct PlaylistsTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    playlists: Vec<PlaylistInfo>,
    is_logged_in: bool,
}

#[derive(Template)]
#[template(path = "pages/playlist.html", escape = "none")]
struct PlaylistTemplate {
    sidebar: String,
    config: Config,
    common_headers: CommonHeaders,
    playlist: PlaylistInfo,
    items: Vec<PlaylistItemDetail>,
    is_owner: bool,
    current_medium_id: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/hx-playlists.html", escape = "none")]
struct HXPlaylistsTemplate {
    playlists: Vec<PlaylistInfo>,
}

#[derive(Template)]
#[template(path = "pages/hx-playlist-items.html", escape = "none")]
struct HXPlaylistItemsTemplate {
    items: Vec<PlaylistItemDetail>,
    playlist_id: String,
    is_owner: bool,
    current_medium_id: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/hx-playlist-selector.html", escape = "none")]
struct HXPlaylistSelectorTemplate {
    playlists: Vec<PlaylistInfo>,
    medium_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct PlaylistInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub created_at: i64,
    pub item_count: i64,
}

#[derive(Serialize, Deserialize)]
pub struct PlaylistItemDetail {
    pub id: i64,
    pub media_id: String,
    pub media_name: String,
    pub media_owner: String,
    pub media_views: i64,
    pub media_type: String,
    pub item_order: i64,
}

#[derive(Deserialize)]
pub struct CreatePlaylistForm {
    pub name: String,
    pub description: String,
    pub public: Option<String>,
}

#[derive(Deserialize)]
pub struct AddToPlaylistForm {
    pub playlist_id: String,
    pub medium_id: String,
}

#[derive(Deserialize)]
pub struct RemoveFromPlaylistForm {
    pub item_id: i64,
}

async fn user_playlists_page(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store.clone()).await;
    let is_logged_in = is_logged(user_info.clone()).await;

    if !is_logged_in {
        return Html(minifi_html(
            "<script>window.location.replace(\"/login\");</script>".to_owned(),
        ));
    }

    let user = user_info.unwrap();
    let common_headers = extract_common_headers(&headers).unwrap();

    let playlists = sqlx::query_as!(
        PlaylistInfo,
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner,
            p.created_at,
            COUNT(pi.id) as item_count
        FROM playlists p
        LEFT JOIN playlist_items pi ON p.id = pi.playlist_id
        WHERE p.owner = $1
        GROUP BY p.id, p.name, p.description, p.owner, p.created_at
        ORDER BY p.created_at DESC
        "#,
        user.login
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let sidebar = generate_sidebar(&config, "playlists".to_owned());
    let template = PlaylistsTemplate {
        sidebar,
        config,
        common_headers,
        playlists,
        is_logged_in,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_user_playlists(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return Html("".to_owned());
    }

    let user = user_info.unwrap();

    let playlists = sqlx::query_as!(
        PlaylistInfo,
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner,
            p.created_at,
            COUNT(pi.id) as item_count
        FROM playlists p
        LEFT JOIN playlist_items pi ON p.id = pi.playlist_id
        WHERE p.owner = $1
        GROUP BY p.id, p.name, p.description, p.owner, p.created_at
        ORDER BY p.created_at DESC
        LIMIT 10
        "#,
        user.login
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let template = HXPlaylistsTemplate { playlists };
    Html(minifi_html(template.render().unwrap()))
}

async fn view_playlist(
    Extension(config): Extension<Config>,
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(playlist_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store.clone()).await;
    let common_headers = extract_common_headers(&headers).unwrap();

    let playlist = sqlx::query_as!(
        PlaylistInfo,
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner,
            p.created_at,
            COUNT(pi.id) as item_count
        FROM playlists p
        LEFT JOIN playlist_items pi ON p.id = pi.playlist_id
        WHERE p.id = $1
        GROUP BY p.id, p.name, p.description, p.owner, p.created_at
        "#,
        playlist_id.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await;

    if playlist.is_err() {
        return Html(minifi_html("<h1>Playlist not found</h1>".to_owned()));
    }

    let playlist = playlist.unwrap();
    let is_owner = user_info.as_ref().map(|u| u.login == playlist.owner).unwrap_or(false);

    let items = sqlx::query_as!(
        PlaylistItemDetail,
        r#"
        SELECT
            pi.id,
            pi.media_id,
            m.name as media_name,
            m.owner as media_owner,
            m.views as media_views,
            m.type as media_type,
            pi.item_order
        FROM playlist_items pi
        JOIN media m ON pi.media_id = m.id
        WHERE pi.playlist_id = $1
        ORDER BY pi.item_order ASC, pi.added_at ASC
        "#,
        playlist.id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let current_medium_id = params.get("current").cloned();
    let sidebar = generate_sidebar(&config, "playlists".to_owned());

    let template = PlaylistTemplate {
        sidebar,
        config,
        common_headers,
        playlist,
        items,
        is_owner,
        current_medium_id,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_create_playlist(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Form(form): Form<CreatePlaylistForm>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_owned());
    }

    let user = user_info.unwrap();
    let playlist_id = generate_medium_id();
    let is_public = form.public.is_some();

    let result = sqlx::query!(
        "INSERT INTO playlists (id, name, description, owner, public) VALUES ($1, $2, $3, $4, $5)",
        playlist_id,
        form.name,
        form.description,
        user.login,
        is_public
    )
    .execute(&pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, format!("<script>window.location.href='/playlist/{}';</script>", playlist_id)),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create playlist".to_owned()),
    }
}

async fn hx_delete_playlist(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(playlist_id): Path<String>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_owned());
    }

    let user = user_info.unwrap();

    let result = sqlx::query!(
        "DELETE FROM playlists WHERE id = $1 AND owner = $2",
        playlist_id.to_ascii_lowercase(),
        user.login
    )
    .execute(&pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, "<script>window.location.href='/playlists';</script>".to_owned()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete playlist".to_owned()),
    }
}

async fn hx_add_to_playlist(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Form(form): Form<AddToPlaylistForm>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_owned());
    }

    let user = user_info.unwrap();

    let playlist_owner = sqlx::query!(
        "SELECT owner FROM playlists WHERE id = $1",
        form.playlist_id.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await;

    if playlist_owner.is_err() || playlist_owner.unwrap().owner != user.login {
        return (StatusCode::FORBIDDEN, "You can only add to your own playlists".to_owned());
    }

    let max_order = sqlx::query!(
        "SELECT COALESCE(MAX(item_order), 0) as max_order FROM playlist_items WHERE playlist_id = $1",
        form.playlist_id.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await
    .map(|r| r.max_order.unwrap_or(0))
    .unwrap_or(0);

    let result = sqlx::query!(
        "INSERT INTO playlist_items (playlist_id, media_id, item_order) VALUES ($1, $2, $3)",
        form.playlist_id.to_ascii_lowercase(),
        form.medium_id.to_ascii_lowercase(),
        max_order + 1
    )
    .execute(&pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, "Added to playlist".to_owned()),
        Err(e) => {
            if e.to_string().contains("unique constraint") {
                (StatusCode::CONFLICT, "Media already in playlist".to_owned())
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to add to playlist".to_owned())
            }
        }
    }
}

async fn hx_remove_from_playlist(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Form(form): Form<RemoveFromPlaylistForm>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_owned());
    }

    let user = user_info.unwrap();

    let playlist_owner = sqlx::query!(
        r#"
        SELECT p.owner
        FROM playlists p
        JOIN playlist_items pi ON p.id = pi.playlist_id
        WHERE pi.id = $1
        "#,
        form.item_id
    )
    .fetch_one(&pool)
    .await;

    if playlist_owner.is_err() || playlist_owner.unwrap().owner != user.login {
        return (StatusCode::FORBIDDEN, "You can only remove from your own playlists".to_owned());
    }

    let result = sqlx::query!(
        "DELETE FROM playlist_items WHERE id = $1",
        form.item_id
    )
    .execute(&pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, "Removed from playlist".to_owned()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to remove from playlist".to_owned()),
    }
}

async fn hx_playlist_selector(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path(medium_id): Path<String>,
) -> axum::response::Html<Vec<u8>> {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return Html("".to_owned());
    }

    let user = user_info.unwrap();

    let playlists = sqlx::query_as!(
        PlaylistInfo,
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner,
            p.created_at,
            COUNT(pi.id) as item_count
        FROM playlists p
        LEFT JOIN playlist_items pi ON p.id = pi.playlist_id
        WHERE p.owner = $1
        GROUP BY p.id, p.name, p.description, p.owner, p.created_at
        ORDER BY p.created_at DESC
        "#,
        user.login
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let template = HXPlaylistSelectorTemplate {
        playlists,
        medium_id,
    };
    Html(minifi_html(template.render().unwrap()))
}

async fn hx_playlist_items_for_recommendations(
    Extension(pool): Extension<PgPool>,
    Path(playlist_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let current_medium_id = params.get("current").cloned().unwrap_or_default();

    let items = sqlx::query_as!(
        PlaylistItemDetail,
        r#"
        SELECT
            pi.id,
            pi.media_id,
            m.name as media_name,
            m.owner as media_owner,
            m.views as media_views,
            m.type as media_type,
            pi.item_order
        FROM playlist_items pi
        JOIN media m ON pi.media_id = m.id
        WHERE pi.playlist_id = $1 AND m.public = true
        ORDER BY pi.item_order ASC, pi.added_at ASC
        "#,
        playlist_id.to_ascii_lowercase()
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| {
        axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to fetch playlist items".into())
            .unwrap()
    })?;

    let template = HXPlaylistItemsTemplate {
        items,
        playlist_id: playlist_id.to_ascii_lowercase(),
        is_owner: false,
        current_medium_id: Some(current_medium_id),
    };

    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}

async fn hx_reorder_playlist_item(
    Extension(pool): Extension<PgPool>,
    Extension(session_store): Extension<Arc<Mutex<AHashMap<String, String>>>>,
    headers: HeaderMap,
    Path((playlist_id, item_id, direction)): Path<(String, i64, String)>,
) -> impl IntoResponse {
    let user_info = get_user_login(headers.clone(), &pool, session_store).await;

    if !is_logged(user_info.clone()).await {
        return (StatusCode::UNAUTHORIZED, "Unauthorized".to_owned());
    }

    let user = user_info.unwrap();

    let playlist_owner = sqlx::query!(
        "SELECT owner FROM playlists WHERE id = $1",
        playlist_id.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await;

    if playlist_owner.is_err() || playlist_owner.unwrap().owner != user.login {
        return (StatusCode::FORBIDDEN, "You can only reorder your own playlists".to_owned());
    }

    let current_item = sqlx::query!(
        "SELECT item_order FROM playlist_items WHERE id = $1 AND playlist_id = $2",
        item_id,
        playlist_id.to_ascii_lowercase()
    )
    .fetch_one(&pool)
    .await;

    if current_item.is_err() {
        return (StatusCode::NOT_FOUND, "Item not found".to_owned());
    }

    let current_order = current_item.unwrap().item_order;

    let swap_result = if direction == "up" {
        if current_order <= 0 {
            return (StatusCode::BAD_REQUEST, "Item is already at the top".to_owned());
        }
        sqlx::query!(
            r#"
            UPDATE playlist_items
            SET item_order = CASE
                WHEN item_order = $1 THEN $2
                WHEN item_order = $2 THEN $1
                ELSE item_order
            END
            WHERE playlist_id = $3 AND (item_order = $1 OR item_order = $2)
            "#,
            current_order,
            current_order - 1,
            playlist_id.to_ascii_lowercase()
        )
        .execute(&pool)
        .await
    } else {
        let max_order = sqlx::query!(
            "SELECT COALESCE(MAX(item_order), 0) as max_order FROM playlist_items WHERE playlist_id = $1",
            playlist_id.to_ascii_lowercase()
        )
        .fetch_one(&pool)
        .await
        .map(|r| r.max_order.unwrap_or(0))
        .unwrap_or(0);

        if current_order >= max_order {
            return (StatusCode::BAD_REQUEST, "Item is already at the bottom".to_owned());
        }

        sqlx::query!(
            r#"
            UPDATE playlist_items
            SET item_order = CASE
                WHEN item_order = $1 THEN $2
                WHEN item_order = $2 THEN $1
                ELSE item_order
            END
            WHERE playlist_id = $3 AND (item_order = $1 OR item_order = $2)
            "#,
            current_order,
            current_order + 1,
            playlist_id.to_ascii_lowercase()
        )
        .execute(&pool)
        .await
    };

    match swap_result {
        Ok(_) => (StatusCode::OK, "Reordered successfully".to_owned()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to reorder".to_owned()),
    }
}
