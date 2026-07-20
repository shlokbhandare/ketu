use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use clap::Parser;
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tokio::sync::RwLock;
use std::time::Duration;
mod backend;
mod ollama;
use axum::http::HeaderMap;
use backend::{Backend, BackendPool};

async fn health() -> &'static str {
    "ok"
}

fn is_forwarded_header(headers: &HeaderMap) -> bool {
    headers
        .get("x-ketu-forwarded")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[derive(Deserialize, Serialize)]
struct RouteRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct Config {
    backends: Vec<Backend>,
}

#[derive(Parser)]
struct Args {
    /// Optional peer address to connect to (host:port)
    #[arg(long)]
    peer: Option<String>,

    /// Port to listen on
    #[arg(long, default_value = "3000")]
    port: u16,
}

#[derive(Serialize, Deserialize)]
struct RouteResponse {
    response: String,
}

#[derive(Deserialize)]
struct HealthUpdate {
    backend_url: String,
    slow: bool,
}

#[derive(Deserialize)]
struct RateSync {
    ip: String,
    increment: u32,
}

#[derive(Clone)]
struct AppState {
    backend_pool: Arc<BackendPool>,
    request_counts: Arc<Mutex<HashMap<String, (u32, std::time::Instant)>>>,
    peer_state: Arc<RwLock<Option<(String, std::time::Instant)>>>,
    backend_health: Arc<RwLock<HashMap<String, bool>>>,
    http_client: reqwest::Client,
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
    let is_forwarded = is_forwarded_header(&headers);

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

    if let Some((peer_url, _)) = state.peer_state.read().await.as_ref().cloned() {
        let http_client = state.http_client.clone();
        let ip_for_sync = ip.clone();
        tokio::spawn(async move {
            let payload = serde_json::json!({
                "ip": ip_for_sync,
                "increment": 1u32,
            });
            let _ = http_client
                .post(format!("http://{}/peer/rate-sync", peer_url))
                .json(&payload)
                .send()
                .await;
        });
    }

    if count > 10 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let forwarded_payload = serde_json::to_value(&payload).expect("route payload should serialize");

    let mut backend = state.backend_pool.next().await;

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

                if elapsed_ms > 2000 {
                    let backend_url = backend.url.clone();
                    {
                        let mut health = state.backend_health.write().await;
                        health.insert(backend_url.clone(), true);
                    }

                    if let Some((peer, _)) = state.peer_state.read().await.as_ref().cloned() {
                        let backend_url = backend.url.clone();
                        let http_client = state.http_client.clone();
                        let backend_health = state.backend_health.clone();
                        let backend_url_for_payload = backend_url.clone();
                        tokio::spawn(async move {
                            let payload = serde_json::json!({
                                "backend_url": backend_url_for_payload,
                                "slow": true,
                            });
                            let _ = http_client
                                .post(format!("http://{}/peer/health-update", peer))
                                .json(&payload)
                                .send()
                                .await;

                            tokio::time::sleep(Duration::from_secs(300)).await;

                            let mut health = backend_health.write().await;
                            health.insert(backend_url.clone(), false);
                            println!("Backend {} cooling period over, re-enabling for routing", backend_url);
                        });
                    }

                }

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
            backend = state.backend_pool.next_excluding(&backend.url).await;
        }
    }

    if !is_forwarded {
        if let Some(peer_url) = state.peer_state.read().await.as_ref().map(|(peer_url, _)| peer_url.clone()) {
            let mut req_headers = reqwest::header::HeaderMap::new();
            req_headers.insert("x-ketu-forwarded", reqwest::header::HeaderValue::from_static("true"));

            let peer_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|_| StatusCode::BAD_GATEWAY)?;

            match peer_client
                .post(format!("http://{}/route", peer_url))
                .headers(req_headers)
                .json(&forwarded_payload)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let peer_response = response
                        .json::<RouteResponse>()
                        .await
                        .map_err(|_| StatusCode::BAD_GATEWAY)?;
                    return Ok(Json(peer_response));
                }
                Ok(_) | Err(_) => {
                    return Err(StatusCode::BAD_GATEWAY);
                }
            }
        }
    }

    if is_forwarded {
        Err(StatusCode::BAD_GATEWAY)
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

#[axum::debug_handler]
async fn peer_health_update(
    State(state): State<AppState>,
    Json(update): Json<HealthUpdate>,
) -> StatusCode {
    let mut health = state.backend_health.write().await;
    health.insert(update.backend_url, update.slow);
    StatusCode::OK
}

#[axum::debug_handler]
async fn peer_rate_sync(
    State(state): State<AppState>,
    Json(sync): Json<RateSync>,
) -> StatusCode {
    let now = std::time::Instant::now();
    let mut counts = state.request_counts.lock().unwrap();
    counts.retain(|_, (_, timestamp)| now.duration_since(*timestamp).as_secs() < 60);

    let entry = counts.entry(sync.ip.clone()).or_insert((0, now));
    if now.duration_since(entry.1).as_secs() >= 60 {
        entry.0 = 0;
        entry.1 = now;
    }
    entry.0 += sync.increment;
    println!("Peer sync for IP {} increased count to {}", sync.ip, entry.0);
    StatusCode::OK
}

async fn stats(State(state): State<AppState>) -> Json<HashMap<String, u64>> {
    let stats = state.backend_pool.get_stats();
    Json(stats)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config_contents = std::fs::read_to_string("config.toml")
        .expect("failed to read config.toml");
    let config: Config = toml::from_str(&config_contents)
        .expect("failed to parse config.toml");

    let backend_health: Arc<RwLock<HashMap<String, bool>>> = Arc::new(RwLock::new(HashMap::new()));
    let backend_pool = Arc::new(BackendPool::new(config.backends, backend_health.clone()));
    let request_counts = Arc::new(Mutex::new(HashMap::new()));
    let peer_state: Arc<RwLock<Option<(String, std::time::Instant)>>> = Arc::new(RwLock::new(None));
    let http_client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(2))
    .build()
    .unwrap();
    let app_state = AppState {
        backend_pool: backend_pool.clone(),
        request_counts: request_counts.clone(),
        peer_state: peer_state.clone(),
        backend_health: backend_health.clone(),
        http_client: http_client.clone(),
    };

    if let Some(addr) = args.peer {
        let peer = addr.clone();
        let peer_state_clone = peer_state.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();

            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;

                let url = format!("http://{}/health", peer);
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let mut w = peer_state_clone.write().await;
                        let was_connected = w.is_some();
                        *w = Some((peer.clone(), std::time::Instant::now()));
                        if !was_connected {
                            println!("peer connected: {}", peer);
                        }
                    }
                    Ok(_) | Err(_) => {
                        let mut w = peer_state_clone.write().await;
                        if let Some((peer_url, last_seen)) = w.clone() {
                            if last_seen.elapsed() > Duration::from_secs(30) {
                                println!("peer lost: {}", peer_url);
                                *w = None;
                            }
                        }
                    }
                }
            }
        });
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/route", post(route))
        .route("/peer/health-update", post(peer_health_update))
        .route("/peer/rate-sync", post(peer_rate_sync))
        .route("/stats", get(stats))
        .with_state(app_state);

    let bind_addr = format!("0.0.0.0:{}", args.port);
    println!("Starting server on http://{}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect(&format!("failed to bind to port {}", args.port));

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_header_is_detected() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ketu-forwarded", "true".parse().unwrap());
        assert!(is_forwarded_header(&headers));
    }

    #[test]
    fn forwarded_header_is_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ketu-forwarded", "TRUE".parse().unwrap());
        assert!(is_forwarded_header(&headers));
    }
}

