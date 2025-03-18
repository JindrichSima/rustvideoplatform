pub fn static_router<P: AsRef<std::path::Path>>(path: P) -> Router {
    let serve_dir = ServeDir::new(path);

    Router::new()
        .fallback_service(serve_dir)
}