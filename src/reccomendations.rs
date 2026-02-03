#[derive(Template)]
#[template(path = "pages/hx-reccomended.html", escape = "none")]
struct HXReccomendedTemplate {
    recommendations: Vec<Medium>,
    is_playlist: bool,
    playlist_id: Option<String>,
    current_medium_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Medium {
    id: String,
    name: String,
    owner: String,
    views: i64,
    r#type: String,
}
async fn hx_recommended(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
    let playlist_id = params.get("playlist").cloned();

    if let Some(ref plid) = playlist_id {
        let items: Vec<PlaylistItemDetail> = sqlx::query_as!(
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
            WHERE pi.playlist_id = $1 AND m.public = true AND pi.media_id != $2
            ORDER BY pi.item_order ASC, pi.added_at ASC
            LIMIT 20
            "#,
            plid.to_ascii_lowercase(),
            mediumid.to_ascii_lowercase()
        )
        .fetch_all(&pool)
        .await
        .map_err(|_| {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to fetch playlist items".into())
                .unwrap()
        })?;
        let recommendations: Vec<Medium> = items
            .into_iter()
            .map(|item| Medium {
                id: item.media_id,
                name: item.media_name,
                owner: item.media_owner,
                views: item.media_views,
                r#type: item.media_type,
            })
            .collect();
        let template = HXReccomendedTemplate {
            recommendations,
            is_playlist: true,
            playlist_id: Some(plid.clone()),
            current_medium_id: Some(mediumid.to_string()),
        };
        match template.render() {
            Ok(rendered) => Ok(Html(minifi_html(rendered))),
            Err(_) => Err(axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to render template".into())
                .unwrap()),
        }
    } else {
        let recommendations: Vec<Medium> = sqlx::query_as!(
            Medium,
            "SELECT id, name, owner, views, type FROM media WHERE public = true AND id != $1 LIMIT 20;",
            mediumid
        )
        .fetch_all(&pool)
        .await
        .map_err(|_| {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to fetch recommendations".into())
                .unwrap()
        })?;
        let template = HXReccomendedTemplate {
            recommendations,
            is_playlist: false,
            playlist_id: None,
            current_medium_id: None,
        };
        match template.render() {
            Ok(rendered) => Ok(Html(minifi_html(rendered))),
            Err(_) => Err(axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to render template".into())
                .unwrap()),
        }
    }
}
