use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::Value;
use tank_core::{Engine, SaveFile, SimulationEngine};

use crate::{error::ApiError, handlers::snapshot::snapshot_response_json, state::AppState};

pub async fn get_save(State(state): State<AppState>) -> Json<SaveFile> {
    let engine = state.engine.lock().unwrap();
    Json(SaveFile::from_engine(&engine))
}

#[derive(Serialize)]
pub struct LoadResponse {
    pub status: &'static str,
    pub snapshot: Value,
}

pub async fn post_load(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<impl IntoResponse, ApiError> {
    let save = SaveFile::from_value(payload).map_err(ApiError::from)?;

    let engine: Engine = save.into_engine().map_err(ApiError::from)?;
    let snapshot = engine.snapshot();
    *state.engine.lock().unwrap() = engine;

    Ok((
        StatusCode::OK,
        Json(LoadResponse {
            status: "loaded",
            snapshot: snapshot_response_json(&snapshot),
        }),
    ))
}
