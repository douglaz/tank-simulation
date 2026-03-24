use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use tank_core::{SimulationEngine, TankSnapshot};

use crate::{error::ApiError, state::AppState};

const MAX_STEP_HOURS: u32 = 24 * 365; // 1 year

#[derive(Deserialize)]
pub struct StepRequest {
    pub hours: u32,
}

pub async fn post_step(
    State(state): State<AppState>,
    Json(req): Json<StepRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if req.hours > MAX_STEP_HOURS {
        return Err(ApiError::bad_request(format!(
            "hours must be at most {MAX_STEP_HOURS}, got {}",
            req.hours
        )));
    }
    let mut engine = state.engine.lock().unwrap();
    engine.step_hours(req.hours).map_err(ApiError::from)?;
    let snapshot: TankSnapshot = engine.snapshot();
    Ok(Json(snapshot))
}
