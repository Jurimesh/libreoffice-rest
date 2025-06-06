use axum::{Router, routing::get};

mod routes;

const PORT: u16 = 1234;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/health", get(routes::health::handler))
        .route("/ready", get(routes::ready::handler));

    let addr: String = format!("0.0.0.0:{}", PORT);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Starting server on {}", &addr);
    axum::serve(listener, app).await.unwrap();
}
