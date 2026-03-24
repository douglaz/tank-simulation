use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use tank_core::{PlayerAction, SimulationEngine};

use crate::{error::ApiError, state::AppState};

#[derive(Serialize)]
struct ActionResponse {
    status: &'static str,
}

pub async fn post_action(
    State(state): State<AppState>,
    Json(action): Json<PlayerAction>,
) -> Result<impl IntoResponse, ApiError> {
    let mut engine = state.engine.lock().unwrap();
    engine.apply_action(action).map_err(ApiError::from)?;
    Ok((StatusCode::OK, Json(ActionResponse { status: "queued" })))
}
