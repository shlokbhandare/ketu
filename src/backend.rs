pub struct BackendPool {
    pub backends: Vec<String>,
}

impl BackendPool {
    pub fn new(backends: Vec<String>) -> Self {
        Self { backends }
    }
}
