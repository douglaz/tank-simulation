use serde::{Deserialize, Serialize};

use crate::{
    engine::{Engine, SimulationEngine},
    types::{PlayerAction, SimError, TankState},
};

const LEGACY_SCHEMA_VERSION: u32 = 2;

// Schema 3 writes net-water chemistry semantics. Schema 2 saves are migrated
// on load by rescaling dissolved totals from the old gross-volume basis onto
// the canonical net-water volume derived from geometry + substrate.
pub const SCHEMA_VERSION: u32 = 3;
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
        let mut save: Self = serde_json::from_str(json)
            .map_err(|error| SimError::Deserialization(error.to_string()))?;
        match save.schema_version {
            SCHEMA_VERSION => Ok(save),
            LEGACY_SCHEMA_VERSION => {
                migrate_schema_v2_to_v3(&mut save.state);
                save.schema_version = SCHEMA_VERSION;
                Ok(save)
            }
            actual => Err(SimError::SchemaVersionMismatch {
                expected: SCHEMA_VERSION,
                actual,
            }),
        }
    }

    pub fn into_engine(self) -> Result<Engine, SimError> {
        // Validate state invariants before exposing the engine.
        let mut state = self.state;
        crate::invariants::enforce_invariants(&mut state)?;

        // Validate queued actions including state-aware checks (source
        // profile existence, shrimp removal counts) by routing through the
        // same path as live action submission.
        let mut engine = Engine::from_parts(state, vec![]);
        for action in self.queued_actions {
            engine.apply_action(action)?;
        }
        Ok(engine)
    }
}

fn migrate_schema_v2_to_v3(state: &mut TankState) {
    let gross_volume_l = state.geometry.water_volume_l();
    let net_volume_l = state.water_volume_l();
    state
        .water
        .rescale_totals_for_volume(gross_volume_l, net_volume_l);
}
