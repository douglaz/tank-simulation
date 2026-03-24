use axum::{extract::State, Json};
use tank_core::{SimulationEngine, TankState};

use crate::state::AppState;

pub async fn get_full_state(State(state): State<AppState>) -> Json<TankState> {
    let engine = state.engine.lock().unwrap();
    Json(engine.full_state().clone())
}
