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
    let snapshot = engine.snapshot();
    let limit = query.limit.unwrap_or(20);
    let events: Vec<_> = snapshot
        .recent_events
        .into_iter()
        .rev()
        .take(limit)
        .collect();
    Json(events)
}
