use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

mod libreoffice;
mod routes;

const PORT: u16 = 1234;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .layer(DefaultBodyLimit::max(250 * 1024 * 1024))
        .route("/health", get(routes::health::handler))
        .route("/ready", get(routes::ready::handler))
        .route("/convert", post(routes::convert::handler))
        .layer(TraceLayer::new_for_http());

    let addr: String = format!("0.0.0.0:{}", PORT);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Starting server on {}", &addr);
    axum::serve(listener, app).await.unwrap();
}
