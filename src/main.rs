use std::env;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

mod detect_filetype;
mod error;
mod libreoffice;
mod logging;
mod routes;

const DEFAULT_PORT: u16 = 1234;

#[tokio::main]
async fn main() {
    logging::setup_tracing_from_env();

    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let app = Router::new()
        .route("/health", get(routes::health::handler))
        .route("/ready", get(routes::ready::handler))
        .route(
            "/convert",
            post(routes::convert::handler).layer(DefaultBodyLimit::max(250 * 1024 * 1024)),
        )
        .layer(TraceLayer::new_for_http());

    let addr: String = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Starting server on {}", &addr);
    axum::serve(listener, app).await.unwrap();
}
