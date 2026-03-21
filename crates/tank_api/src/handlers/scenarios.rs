use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tank_core::{Engine, SimSeed, SimulationEngine, TankSnapshot};
use tank_scenarios::seeded_state_with_full_overrides;

use crate::{error::ApiError, state::AppState};

#[derive(Serialize)]
pub struct ScenarioInfo {
    pub id: &'static str,
}

pub async fn list_scenarios() -> Json<Vec<ScenarioInfo>> {
    let scenarios: Vec<_> = tank_data::scenario_ids()
        .iter()
        .map(|id| ScenarioInfo { id })
        .collect();
    Json(scenarios)
}

pub async fn list_source_water() -> Json<Vec<&'static str>> {
    Json(tank_data::source_water_ids().to_vec())
}

#[derive(Deserialize)]
pub struct LoadScenarioRequest {
    pub scenario_id: String,
    pub seed: Option<u64>,
}

#[derive(Serialize)]
pub struct LoadScenarioResponse {
    pub status: &'static str,
    pub snapshot: TankSnapshot,
}

pub async fn load_scenario(
    State(state): State<AppState>,
    Json(req): Json<LoadScenarioRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let seed = SimSeed(req.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }));

    let overrides = tank_scenarios::StartupOverrides::default();
    let tank_state = seeded_state_with_full_overrides(seed, &req.scenario_id, overrides)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let engine = Engine::from_parts(tank_state, vec![]);
    let snapshot = engine.snapshot();

    *state.engine.lock().unwrap() = engine;

    Ok((
        StatusCode::OK,
        Json(LoadScenarioResponse {
            status: "loaded",
            snapshot,
        }),
    ))
}
