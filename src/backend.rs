use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Clone, Deserialize)]
pub struct Backend {
    pub url: String,
    pub model: String,
    pub weight: u32,
}

pub struct BackendPool {
    pub backends: Vec<Backend>,
    current_index: Arc<Mutex<usize>>,
    weighted_order: Vec<usize>,
    pub token_counts: Arc<Mutex<HashMap<String, u64>>>,
}

impl BackendPool {
    pub fn new(backends: Vec<Backend>) -> Self {
        let mut weighted_order = Vec::new();
        for (index, backend) in backends.iter().enumerate() {
            weighted_order.extend(std::iter::repeat(index).take(backend.weight as usize));
        }
        Self {
            backends,
            current_index: Arc::new(Mutex::new(0)),
            weighted_order,
            token_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn next(&self) -> Backend {
        let mut current_index = self.current_index.lock().unwrap();
        let backend_index = self.weighted_order[*current_index];
        let backend = self.backends[backend_index].clone();
        *current_index = (*current_index + 1) % self.weighted_order.len();
        backend
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
