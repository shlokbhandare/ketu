use axum::{extract::State, routing::get, routing::post, Json, Router};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use backend::Backend;
mod backend;
mod ollama;

use backend::BackendPool;

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct RouteRequest {
    model: String,
    prompt: String,
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
        let backend_pool = Arc::new(BackendPool::new(vec![
    Backend { url: "http://localhost:11434".to_string(), model: "llama3.2:3b".to_string() },
    Backend { url: "http://localhost:11435".to_string(), model: "qwen2.5:7b".to_string() },
]));

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
