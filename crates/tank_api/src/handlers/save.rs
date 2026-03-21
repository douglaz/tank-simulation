use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use tank_core::{Engine, SaveFile, SimulationEngine, TankSnapshot};

use crate::{error::ApiError, state::AppState};

pub async fn get_save(State(state): State<AppState>) -> Json<SaveFile> {
    let engine = state.engine.lock().unwrap();
    Json(SaveFile::from_engine(&engine))
}

#[derive(Serialize)]
pub struct LoadResponse {
    pub status: &'static str,
    pub snapshot: TankSnapshot,
}

pub async fn post_load(
    State(state): State<AppState>,
    Json(save): Json<SaveFile>,
) -> Result<impl IntoResponse, ApiError> {
    if save.schema_version != tank_core::SCHEMA_VERSION {
        return Err(ApiError::bad_request(format!(
            "schema version mismatch: expected {}, got {}",
            tank_core::SCHEMA_VERSION,
            save.schema_version,
        )));
    }

    let engine: Engine = save.into_engine();
    let snapshot: TankSnapshot = engine.snapshot();
    *state.engine.lock().unwrap() = engine;

    Ok((
        StatusCode::OK,
        Json(LoadResponse {
            status: "loaded",
            snapshot,
        }),
    ))
}
