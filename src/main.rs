use axum::{routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

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
    backend: String,
    prompt_chars: usize,
}

async fn route(Json(payload): Json<RouteRequest>) -> Json<RouteResponse> {
    Json(RouteResponse {
        backend: format!("backend-for-{}", payload.model),
        prompt_chars: payload.prompt.len(),
    })
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
