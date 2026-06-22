use std::sync::{Arc, Mutex};

pub struct BackendPool {
    pub backends: Vec<String>,
    current_index: Arc<Mutex<usize>>,
}

impl BackendPool {
    pub fn new(backends: Vec<String>) -> Self {
        Self {
            backends,
            current_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn next(&self) -> String {
        let mut current_index = self.current_index.lock().unwrap();
        let backend = self.backends[*current_index].clone();
        *current_index = (*current_index + 1) % self.backends.len();
        backend
    }
}
