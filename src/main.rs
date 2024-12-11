#![forbid(unsafe_code)]
#![allow(non_snake_case)]

use argon2::Argon2;
use argon2::PasswordVerifier;
use axum::http::StatusCode;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use ahash::AHashMap;
use argon2::password_hash::{rand_core::OsRng, PasswordHash};
use askama::Template;
use axum::{
    extract::{DefaultBodyLimit, Form, Multipart, Path},
    http::header::HeaderMap,
    http::header::{ACCEPT_LANGUAGE, COOKIE, HOST, USER_AGENT},
    response::{Html, IntoResponse},
    routing::get,
    routing::post,
    Extension, Json, Router,
};
use chrono::{DateTime, Datelike, Local, Timelike};
use memory_serve::{load_assets, MemoryServe};
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::io::BufRead;
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, sync::Mutex, io, fs};

#[derive(Deserialize, Clone)]
struct Config {
    dbconnection: String,
    instancename: String,
    welcome: String,
    custom_upload_url: Option<String>,
    custom_session_domain: Option<String>
}
#[tokio::main]
async fn main() {
    let config: Config = serde_json::from_str(&fs::read_to_string("config.json").await.unwrap()).unwrap();

    let pool = PgPoolOptions::new()
        .max_connections(100)
        .connect(&config.dbconnection)
        .await
        .unwrap();

    let memory_router = MemoryServe::new(load_assets!("assets/static")).into_router();

    let session_store: Arc<Mutex<AHashMap<String, String>>> =
        Arc::new(Mutex::new(AHashMap::default()));

    let app = Router::new()
        .route("/", get(home))
        .route("/login", get(login))
        .route("/trending", get(trending))
        .route("/hx/trending", get(hx_trending))
        .route("/m/:mediumid", get(medium))
        .route("/m/:mediumid/previews.json", get(medium_previews_prepare))
        .route("/hx/comments/:mediumid", get(hx_comments))
        .route("/hx/reccomended/:mediumid", get(hx_recommended))
        .route("/hx/new_view/:mediumid", get(hx_new_view))
        .route("/hx/like/:mediumid", get(hx_like))
        .route("/hx/dislike/:mediumid", get(hx_dislike))
        .route("/hx/subscribe/:userid", get(hx_subscribe))
        .route("/hx/unsubscribe/:userid", get(hx_unsubscribe))
        .route("/hx/subscribebutton/:userid", get(hx_subscribebutton))
        .route("/hx/login", post(hx_login))
        .route("/hx/logout", get(hx_logout))
        .route("/hx/usernav", get(hx_usernav))
        .route("/hx/sidebar/:active_item", get(hx_sidebar))
        .route("/hx/searchsuggestions", post(hx_search_suggestions))
        .route("/search", get(search))
        .route("/hx/search/:pageid", post(hx_search))
        .route("/channel/:userid", get(channel))
        .route("/hx/usermedia/:userid", get(hx_usermedia))
        .route("/studio", get(studio))
        .route("/hx/studio", get(hx_studio))
        .route("/studio/concepts", get(concepts))
        .route("/hx/studio/concepts", get(hx_concepts))
        .route("/studio/concept/:conceptid", get(concept))
        .route("/studio/concept/:conceptid/publish", post(publish))
        .route("/upload", get(upload))
        .route("/uploadform", get(uploadform))
        .route("/hx/upload", post(hx_upload))
        .nest("/source", axum_static::static_router("source"))
        .layer(Extension(pool))
        .layer(Extension(config))
        .layer(Extension(session_store))
        .layer(DefaultBodyLimit::disable())
        .merge(memory_router);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

include!("helper_functions.rs");
include!("sidebar.rs");
include!("medium.rs");
include!("comments.rs");
include!("reccomendations.rs");
include!("likes_dislikes.rs");
include!("subscriptions.rs");
include!("views.rs");
include!("login_handler.rs");
include!("usernav.rs");
include!("trending.rs");
include!("home.rs");
include!("search.rs");
include!("channel.rs");
include!("studio.rs");
include!("upload.rs");
include!("concept.rs");