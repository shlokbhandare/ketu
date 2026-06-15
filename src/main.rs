use axum::{routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

mod ollama;

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

async fn route(Json(payload): Json<RouteRequest>) -> Json<RouteResponse> {
    let response = ollama::generate(&payload.prompt, "llama3.2:3b")
        .await
        .unwrap_or_else(|e| format!("Error: {}", e));

    Json(RouteResponse { response })
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/route", post(route));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
