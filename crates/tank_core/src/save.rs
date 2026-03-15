use serde::{Deserialize, Serialize};

use crate::{
    engine::{Engine, SimulationEngine},
    types::{PlayerAction, SimError, TankState},
};

pub const SCHEMA_VERSION: u32 = 1;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SaveFile {
    pub schema_version: u32,
    pub app_version: String,
    pub state: TankState,
    pub queued_actions: Vec<PlayerAction>,
}

impl SaveFile {
    pub fn new(state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app_version: APP_VERSION.to_string(),
            state,
            queued_actions,
        }
    }

    pub fn from_engine(engine: &Engine) -> Self {
        Self::new(engine.full_state().clone(), engine.queued_actions())
    }

    pub fn to_json_pretty(&self) -> Result<String, SimError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| SimError::Serialization(error.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, SimError> {
        let save: Self = serde_json::from_str(json)
            .map_err(|error| SimError::Deserialization(error.to_string()))?;
        if save.schema_version != SCHEMA_VERSION {
            return Err(SimError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                actual: save.schema_version,
            });
        }
        Ok(save)
    }

    pub fn into_engine(self) -> Engine {
        Engine::from_parts(self.state, self.queued_actions)
    }
}
