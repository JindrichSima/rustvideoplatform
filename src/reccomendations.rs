struct MediumWithShowcase {
    medium: Medium,
    showcase_exists: bool,
}

#[derive(Template)]
#[template(path = "pages/hx-reccomended.html", escape = "none")]
struct HXReccomendedTemplate {
    recommendations: Vec<MediumWithShowcase>,
}

fn showcase_exists(medium_id: &str) -> bool {
    std::path::Path::new(&format!("source/{}/showcase.avif", medium_id)).exists()
}

async fn hx_recommended(
    Extension(pool): Extension<PgPool>,
    Path(mediumid): Path<String>,
) -> Result<Html<Vec<u8>>, axum::response::Response> {
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

    let recommendations: Vec<MediumWithShowcase> = recommendations
        .into_iter()
        .map(|m| {
            let has_showcase = showcase_exists(&m.id);
            MediumWithShowcase {
                medium: m,
                showcase_exists: has_showcase,
            }
        })
        .collect();

    let template = HXReccomendedTemplate { recommendations };
    match template.render() {
        Ok(rendered) => Ok(Html(minifi_html(rendered))),
        Err(_) => Err(axum::response::Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Failed to render template".into())
            .unwrap()),
    }
}
