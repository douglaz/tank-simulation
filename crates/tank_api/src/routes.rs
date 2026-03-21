use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::{handlers, state::AppState};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/snapshot", get(handlers::snapshot::get_snapshot))
        .route(
            "/snapshot/chemistry",
            get(handlers::snapshot::get_chemistry),
        )
        .route("/snapshot/biology", get(handlers::snapshot::get_biology))
        .route("/snapshot/hardware", get(handlers::snapshot::get_hardware))
        .route(
            "/snapshot/biofilter",
            get(handlers::snapshot::get_biofilter),
        )
        .route(
            "/snapshot/environment",
            get(handlers::snapshot::get_environment),
        )
        .route("/state", get(handlers::state::get_full_state))
        .route("/events", get(handlers::events::get_events))
        .route("/actions", post(handlers::actions::post_action))
        .route("/time/step", post(handlers::time::post_step))
        .route("/scenarios", get(handlers::scenarios::list_scenarios))
        .route("/scenarios/load", post(handlers::scenarios::load_scenario))
        .route("/source-water", get(handlers::scenarios::list_source_water))
        .route("/save", get(handlers::save::get_save))
        .route("/load", post(handlers::save::post_load))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
