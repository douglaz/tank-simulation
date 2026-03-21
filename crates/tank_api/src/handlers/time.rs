use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use tank_core::{SimulationEngine, TankSnapshot};

use crate::{error::ApiError, state::AppState};

#[derive(Deserialize)]
pub struct StepRequest {
    pub hours: u32,
}

pub async fn post_step(
    State(state): State<AppState>,
    Json(req): Json<StepRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut engine = state.engine.lock().unwrap();
    engine.step_hours(req.hours).map_err(ApiError::from)?;
    let snapshot: TankSnapshot = engine.snapshot();
    Ok(Json(snapshot))
}
