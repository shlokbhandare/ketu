use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use clap::Parser;
use rand::Rng;
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tokio::sync::{Mutex as TokioMutex, RwLock};
use std::time::Duration;
mod analyzer;
mod backend;
mod ollama;
use axum::http::{HeaderMap, HeaderValue};
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

#[derive(Debug, Clone, PartialEq)]
enum NodeRole {
    Leader,
    Follower,
}

#[derive(Clone)]
struct AppState {
    backend_pool: Arc<BackendPool>,
    request_counts: Arc<Mutex<HashMap<String, (u32, std::time::Instant)>>>,
    peer_state: Arc<RwLock<Option<String>>>,
    backend_health: Arc<RwLock<HashMap<String, bool>>>,
    role: Arc<TokioMutex<NodeRole>>,
    http_client: reqwest::Client,
}

#[axum::debug_handler]
async fn route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<(HeaderMap, Json<RouteResponse>), StatusCode> {
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

        let entry = counts.entry(ip.clone()).or_insert((0, now));
        entry.0 += 1;
        println!("IP {} has made {} requests", ip, entry.0);
        entry.0
    };

    if state.peer_state.read().await.is_some() {
        if let Some(peer_url) = state.peer_state.read().await.as_ref().cloned() {
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
    }

    if count > 100 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let extracted_text = payload
        .get("prompt")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| serde_json::to_string(&payload).unwrap_or_default());

    let complexity = analyzer::classify(&extracted_text);
    let mut backend_for_request = None;
    let target_url = match complexity {
        analyzer::PromptComplexity::LowLatency => Some("http://localhost:11434".to_string()),
        analyzer::PromptComplexity::HighCapacity => Some("http://localhost:11435".to_string()),
        analyzer::PromptComplexity::Uncertain => {
            println!("[INFO] Ambiguous prompt detected, falling back to round-robin");
            let backend = state.backend_pool.next().await;
            backend_for_request = Some(backend);
            None
        }
    };

    let mut request_model = if let Some(backend) = backend_for_request.as_ref() {
        backend.model.clone()
    } else {
        payload
            .get("model")
            .and_then(|value| value.as_str())
            .unwrap_or("default")
            .to_string()
    };

    if request_model == "default" {
        println!("Using fallback model 'llama3.2:3b' for requests");
        request_model = "llama3.2:3b".to_string();
    }

    let target_url = target_url.unwrap_or_else(|| {
        backend_for_request
            .as_ref()
            .map(|backend| backend.url.clone())
            .unwrap_or_else(|| "http://localhost:11434".to_string())
    });

    println!(
        "[INFO] Semantic routing: Classified as {:?} -> targeting {}",
        complexity,
        target_url
    );

    // track the last error encountered while trying backends/peers
    let mut last_err: Option<String> = None;

    let forwarded_payload = payload.clone();

    let start = std::time::Instant::now();
    println!("[DEBUG] Sending request to {} for model {}", target_url, request_model);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        ollama::generate(&target_url, &extracted_text, &request_model),
    )
    .await;

    match result {
        Ok(Ok(response)) => {
            let token_count = (response.len() as u64) / 4;
            state.backend_pool.record_tokens(&target_url, token_count);
            let elapsed_ms = start.elapsed().as_millis();
            println!("Target {} responded in {}ms", target_url, elapsed_ms);

            let mut resp_headers = HeaderMap::new();
            resp_headers.insert(
                "x-ketu-target",
                HeaderValue::from_str(&target_url)
                    .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
            );

            return Ok((resp_headers, Json(RouteResponse { response })));
        }
        Ok(Err(err)) => {
            println!("Target {} failed: {}", target_url, err);
            last_err = Some(format!("target_error: {}", err));
        }
        Err(_) => {
            println!("Target {} timed out", target_url);
            last_err = Some("timeout".to_string());
        }
    }

    if !is_forwarded {
        if let Some(peer_url) = state.peer_state.read().await.as_ref().cloned() {
            let mut req_headers = reqwest::header::HeaderMap::new();
            req_headers.insert("x-ketu-forwarded", reqwest::header::HeaderValue::from_static("true"));

            let client = state.http_client.clone();

            match client
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

                    let mut resp_headers = HeaderMap::new();
                    resp_headers.insert(
                        "x-ketu-target",
                        HeaderValue::from_str(&peer_url)
                            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                    );

                    return Ok((resp_headers, Json(peer_response)));
                }
                Ok(_) | Err(_) => {
                    last_err = Some("peer_forward_failed".to_string());
                    return Err(StatusCode::BAD_GATEWAY);
                }
            }
        }
    }

    if is_forwarded {
        Err(StatusCode::BAD_GATEWAY)
    } else {
        println!("[ERROR] Route failed because: {:?}", last_err);
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
    let peer_state: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));
    let role = Arc::new(TokioMutex::new(if args.peer.is_some() {
        NodeRole::Follower
    } else {
        NodeRole::Leader
    }));
    let initial_role = match *role.lock().await {
        NodeRole::Leader => "Leader",
        NodeRole::Follower => "Follower",
    };
    println!("Starting as {}", initial_role);
    let http_client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(2))
    .build()
    .unwrap();
    let app_state = AppState {
        backend_pool: backend_pool.clone(),
        request_counts: request_counts.clone(),
        peer_state: peer_state.clone(),
        backend_health: backend_health.clone(),
        role: role.clone(),
        http_client: http_client.clone(),
    };

    if let Some(addr) = args.peer {
        let peer = addr.clone();
        let peer_state_clone = peer_state.clone();
        let role_for_monitor = role.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut missed_heartbeats = 0u32;

            loop {
                if *role_for_monitor.lock().await != NodeRole::Follower {
                    break;
                }

                let timeout = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(5..12)
                };
                println!("[DEBUG] Election timer set to {}s", timeout);
                tokio::time::sleep(Duration::from_secs(timeout)).await;

                let url = format!("http://{}/health", peer);
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        missed_heartbeats = 0;
                        let mut w = peer_state_clone.write().await;
                        let was_connected = w.is_some();
                        *w = Some(peer.clone());
                        if !was_connected {
                            println!("peer connected: {}", peer);
                        }
                    }
                    Ok(_) | Err(_) => {
                        missed_heartbeats += 1;
                        let mut w = peer_state_clone.write().await;
                        if missed_heartbeats >= 3 {
                            if w.is_some() {
                                println!("peer lost: {}", peer);
                                *w = None;
                            }

                            let mut role = role_for_monitor.lock().await;
                            if *role == NodeRole::Follower {
                                *role = NodeRole::Leader;
                                println!("[RAFT] Term limit exceeded. Promoting self to Leader..");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    let request_counts_for_cleanup = request_counts.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let now = std::time::Instant::now();
            let mut counts = request_counts_for_cleanup.lock().unwrap();
            counts.retain(|_, (_, timestamp)| now.duration_since(*timestamp).as_secs() < 60);
        }
    });

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

    #[test]
    fn missing_forwarded_header_returns_false() {
        let headers = HeaderMap::new();
        assert!(!is_forwarded_header(&headers));
    }

    #[test]
    fn non_true_forwarded_header_value_returns_false() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ketu-forwarded", "yes".parse().unwrap());
        assert!(!is_forwarded_header(&headers));
    }

    #[test]
    fn numeric_forwarded_header_value_returns_false() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ketu-forwarded", "1".parse().unwrap());
        assert!(!is_forwarded_header(&headers));
    }
}

