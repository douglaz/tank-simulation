use std::sync::{Arc, Mutex};

use tank_core::Engine;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    #[allow(dead_code)]
    pub scenario_label: String,
}

impl AppState {
    pub fn new(engine: Engine, scenario_label: String) -> Self {
        Self {
            engine: Arc::new(Mutex::new(engine)),
            scenario_label,
        }
    }
}
