use serde::Deserialize;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Clone, Deserialize)]
pub struct Backend {
    pub url: String,
    pub model: String,
    pub weight: u32,
}

pub struct BackendPool {
    pub backends: Vec<Backend>,
    current_index: Arc<AtomicUsize>,
    weighted_order: Vec<usize>,
    pub token_counts: Arc<Mutex<HashMap<String, u64>>>,
    backend_health: Arc<RwLock<HashMap<String, bool>>>,
}

impl BackendPool {
    pub fn new(
        backends: Vec<Backend>,
        backend_health: Arc<RwLock<HashMap<String, bool>>>,
    ) -> Self {
        let mut weighted_order = Vec::new();
        for (index, backend) in backends.iter().enumerate() {
            weighted_order.extend(std::iter::repeat(index).take(backend.weight as usize));
        }
        Self {
            backends,
            current_index: Arc::new(AtomicUsize::new(0)),
            weighted_order,
            token_counts: Arc::new(Mutex::new(HashMap::new())),
            backend_health,
        }
    }

    async fn select_backend(
        &self,
        exclude_url: Option<&str>,
        skip_slow: bool,
    ) -> Backend {
        let start_index = self.current_index.load(Ordering::Relaxed);
        let len = self.weighted_order.len();
        let health = self.backend_health.read().await;

        for offset in 0..len {
            let candidate_index = (start_index + offset) % len;
            let backend_index = self.weighted_order[candidate_index];
            let backend = &self.backends[backend_index];

            if let Some(exclude) = exclude_url {
                if backend.url == exclude {
                    continue;
                }
            }

            if skip_slow {
                if let Some(is_slow) = health.get(&backend.url) {
                    if *is_slow {
                        continue;
                    }
                }
            }

            self.current_index.store((candidate_index + 1) % len, Ordering::Relaxed);
            return backend.clone();
        }

        for offset in 0..len {
            let candidate_index = (start_index + offset) % len;
            let backend_index = self.weighted_order[candidate_index];
            let backend = &self.backends[backend_index];

            if let Some(exclude) = exclude_url {
                if backend.url == exclude {
                    continue;
                }
            }

            self.current_index.store((candidate_index + 1) % len, Ordering::Relaxed);
            return backend.clone();
        }

        self.backends[0].clone()
    }

    pub async fn next(&self) -> Backend {
        self.select_backend(None, true).await
    }

    pub async fn next_excluding(&self, exclude_url: &str) -> Backend {
        self.select_backend(Some(exclude_url), true).await
    }

    pub fn record_tokens(&self, url: &str, count: u64) {
        let mut map = self.token_counts.lock().unwrap();
        let entry = map.entry(url.to_string()).or_insert(0);
        *entry += count;
    }

    pub fn get_stats(&self) -> HashMap<String, u64> {
        let map = self.token_counts.lock().unwrap();
        map.clone()
    }
}
