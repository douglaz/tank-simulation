mod error;
mod handlers;
mod routes;
mod state;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tank_core::{Engine, SimSeed};
use tank_scenarios::seeded_state_with_full_overrides;

use state::AppState;

#[tokio::main]
async fn main() {
    let scenario_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "nano_cycle".to_string());
    let bind_addr = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:3000".to_string());

    let seed = SimSeed(seed_from_clock());
    let overrides = tank_scenarios::startup_defaults_for_scenario(&scenario_id)
        .unwrap_or_else(|e| panic!("failed to load defaults for `{scenario_id}`: {e}"));
    let tank_state = seeded_state_with_full_overrides(seed, &scenario_id, overrides)
        .unwrap_or_else(|e| panic!("failed to load scenario `{scenario_id}`: {e}"));

    let engine = Engine::from_parts(tank_state, vec![]);
    let app_state = AppState::new(engine, scenario_id.clone());

    let router = routes::build_router(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {bind_addr}: {e}"));

    eprintln!("tank_api listening on {bind_addr} (scenario: {scenario_id})");

    axum::serve(listener, router).await.unwrap();
}

fn seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}
