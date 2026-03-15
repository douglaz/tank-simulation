use tank_core::{
    EventKind, ProcessParams, SimSeed, SimulationEngine, TankState, Engine,
};

fn threshold_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    let volume_l = state.geometry.water_volume_l();
    state.water.alkalinity_meq_total = 8.0 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 0.5 * volume_l;
    state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
    state.water.nitrite_mg_n_total = 0.7 * volume_l;
    state.water.dissolved_oxygen_mg_total = 3.0 * volume_l;
    state.process_params = ProcessParams {
        reaeration_kla_base: 0.0,
        aeration_kla_boost: 0.0,
        background_bod_mg_o2_per_g_biomass_per_hour: 0.0,
        plant_photosynthesis_o2_mg_per_g_per_hour: 0.0,
        respiration_dic_rate_mg_c_per_g_per_hour: 0.0,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.0,
        ..ProcessParams::default()
    };
    state
}

#[test]
fn threshold_events_emit_with_cause_codes() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(threshold_state(SimSeed(5000)), vec![]);
    engine.step_hours(1)?;

    let events = &engine.full_state().event_log;
    assert!(events.iter().any(|event| event.kind == EventKind::AmmoniaWarning));
    assert!(events.iter().any(|event| event.kind == EventKind::NitriteWarning));
    assert!(events.iter().any(|event| event.kind == EventKind::OxygenDip));
    assert!(events.iter().all(|event| !event.cause_codes.is_empty()));

    Ok(())
}

#[test]
fn threshold_events_are_deduplicated_per_day() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(threshold_state(SimSeed(5100)), vec![]);

    engine.step_hours(6)?;
    let same_day_count = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::AmmoniaWarning)
        .count();
    assert_eq!(same_day_count, 1, "ammonia warning should dedupe within a day");

    engine.step_hours(24)?;
    let next_day_count = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::AmmoniaWarning)
        .count();
    assert_eq!(next_day_count, 2, "warning should emit again on a new day");

    Ok(())
}
