use axum::{extract::State, routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
mod backend;
mod ollama;

use backend::{Backend, BackendPool};

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct RouteRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct Config {
    backends: Vec<Backend>,
}

#[derive(Serialize)]
struct RouteResponse {
    response: String,
}

async fn route(
    State(pool): State<Arc<BackendPool>>,
    Json(payload): Json<RouteRequest>,
) -> Json<RouteResponse> {
    let backend = pool.next();
    let response = ollama::generate(&backend.url, &payload.prompt, &backend.model)
        .await
        .unwrap_or_else(|e| format!("Error: {}", e));

    Json(RouteResponse { response })
}

#[tokio::main]
async fn main() {
    let config_contents = std::fs::read_to_string("config.toml")
        .expect("failed to read config.toml");
    let config: Config = toml::from_str(&config_contents)
        .expect("failed to parse config.toml");

    let backend_pool = Arc::new(BackendPool::new(config.backends));

    let app = Router::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .with_state(backend_pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
