use axum::{extract::State, Json};
use serde::Deserialize;
use tank_core::{SimEvent, SimulationEngine};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct EventsQuery {
    pub limit: Option<usize>,
}

pub async fn get_events(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<EventsQuery>,
) -> Json<Vec<SimEvent>> {
    let engine = state.engine.lock().unwrap();
    let limit = query.limit.unwrap_or(20);
    let log = &engine.full_state().event_log;
    let skip = log.len().saturating_sub(limit);
    let events: Vec<_> = log.iter().skip(skip).cloned().collect();
    Json(events)
}
