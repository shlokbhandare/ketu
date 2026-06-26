use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Backend {
    pub url: String,
    pub model: String,
}

pub struct BackendPool {
    pub backends: Vec<Backend>,
    current_index: Arc<Mutex<usize>>,
}

impl BackendPool {
    pub fn new(backends: Vec<Backend>) -> Self {
        Self {
            backends,
            current_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn next(&self) -> Backend {
        let mut current_index = self.current_index.lock().unwrap();
        let backend = self.backends[*current_index].clone();
        *current_index = (*current_index + 1) % self.backends.len();
        backend
    }
}
