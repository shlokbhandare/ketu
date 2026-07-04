use axum::{
    extract::{Request, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::{Arc, Mutex}};
mod backend;
mod ollama;
use axum::http::HeaderMap;
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

#[derive(Clone)]
struct AppState {
    backend_pool: Arc<BackendPool>,
    request_counts: Arc<Mutex<HashMap<String, (u32, std::time::Instant)>>>,
}
#[axum::debug_handler]
async fn route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RouteRequest>,
) -> Result<Json<RouteResponse>, StatusCode> {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();

    let now = std::time::Instant::now();
    let count = {
        let mut counts = state.request_counts.lock().unwrap();
        counts.retain(|_, (_, timestamp)| now.duration_since(*timestamp).as_secs() < 60);

        let entry = counts.entry(ip.clone()).or_insert((0, now));
        if now.duration_since(entry.1).as_secs() >= 60 {
            entry.0 = 0;
            entry.1 = now;
        }
        entry.0 += 1;
        println!("IP {} has made {} requests", ip, entry.0);
        entry.0
    };

    if count > 10 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let mut backend = state.backend_pool.next();

    for attempt in 1..=2 {
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            ollama::generate(&backend.url, &payload.prompt, &backend.model),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let token_count = (response.len() as u64) / 4;
                state.backend_pool.record_tokens(&backend.url, token_count);
                let elapsed_ms = start.elapsed().as_millis();
                println!("Backend {} responded in {}ms", backend.url, elapsed_ms);
                return Ok(Json(RouteResponse { response }));
            }
            Ok(Err(err)) => {
                let message = format!("Error: {}", err);
                println!("Backend {} failed on attempt {}: {}", backend.url, attempt, message);
            }
            Err(_) => {
                println!("Backend {} timed out on attempt {}", backend.url, attempt);
            }
        }

        if attempt < 3 {
            backend = state.backend_pool.next_excluding(&backend.url);
        }
    }

    Err(StatusCode::INTERNAL_SERVER_ERROR)
}

#[axum::debug_handler]
async fn stats(State(state): State<AppState>) -> Json<HashMap<String, u64>> {
    let stats = state.backend_pool.get_stats();
    Json(stats)
}

#[tokio::main]
async fn main() {
    let config_contents = std::fs::read_to_string("config.toml")
        .expect("failed to read config.toml");
    let config: Config = toml::from_str(&config_contents)
        .expect("failed to parse config.toml");

    let backend_pool = Arc::new(BackendPool::new(config.backends));
    let request_counts = Arc::new(Mutex::new(HashMap::new()));
    let app_state = AppState {
        backend_pool: backend_pool.clone(),
        request_counts: request_counts.clone(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .route("/stats", get(stats))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to port 3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

