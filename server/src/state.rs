use crate::config::Config;
use std::sync::Arc;

#[derive(Clone)]
pub struct ServerState {
    pub config: Arc<Config>,
}

impl ServerState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}
