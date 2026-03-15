use tank_core::{SimMeta, SimSeed, TankState};
use tank_data::{load_scenario, ScenarioPreset};

pub fn default_scenario_ids() -> &'static [&'static str] {
    tank_data::scenario_ids()
}

pub fn load_named_scenario(id: &str) -> Result<ScenarioPreset, tank_data::PresetError> {
    load_scenario(id)
}

pub fn seeded_state(seed: SimSeed, scenario_id: &str) -> Result<TankState, tank_data::PresetError> {
    let scenario = load_named_scenario(scenario_id)?;
    let mut state = TankState::new(seed);
    state.meta = SimMeta {
        scenario_id: Some(scenario.id),
        notes: Some(scenario.name),
    };
    Ok(state)
}
