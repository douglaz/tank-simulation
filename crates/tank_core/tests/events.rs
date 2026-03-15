use tank_core::{
    Engine, EventKind, PlayerAction, ProcessParams, SimSeed, SimulationEngine, TankState,
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

#[test]
fn event_generation_warm_overfed_weak_aeration() -> Result<(), tank_core::SimError> {
    // Warm, overfed, weak-aeration setup — designed to trigger AmmoniaWarning or OxygenDip
    let mut state = TankState::new(SimSeed(5200));
    let vol = state.geometry.water_volume_l();
    state.water.temperature_c = 30.0;
    state.environment.ambient_temp_c = 30.0;
    // Higher pH pushes NH3 fraction up at 30°C
    state.water.alkalinity_meq_total = 4.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    // Weak aeration
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.05;
    // Low reaeration to allow DO to drop
    state.process_params = ProcessParams {
        reaeration_kla_base: 0.05,
        aeration_kla_boost: 0.1,
        ..ProcessParams::default()
    };
    // Strong decomposer biomass to mineralize feed quickly into TAN
    state.microbe.decomposer_biomass_g = 0.5;
    // Minimal nitrification so TAN accumulates
    state.microbe.ammonia_oxidizer_biomass_g = 0.005;
    state.microbe.nitrite_oxidizer_biomass_g = 0.005;
    state.microbe.comammox_biomass_g = 0.001;
    state.filter_state.biofilter_maturity_index = 0.05;

    let mut engine = Engine::from_parts(state, vec![]);

    // Heavy overfeeding for 14 days
    for _ in 0..14 {
        engine.apply_action(PlayerAction::Feed { grams: 3.0 })?;
        engine.step_hours(24)?;
    }

    let events = &engine.full_state().event_log;

    // Must emit at least one of AmmoniaWarning or OxygenDip
    let has_ammonia = events.iter().any(|e| e.kind == EventKind::AmmoniaWarning);
    let has_oxygen = events.iter().any(|e| e.kind == EventKind::OxygenDip);
    assert!(
        has_ammonia || has_oxygen,
        "Warm overfed tank should emit AmmoniaWarning or OxygenDip. Events: {:?}",
        events.iter().map(|e| &e.kind).collect::<Vec<_>>()
    );

    // All emitted events must have non-empty cause_codes
    let relevant: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::AmmoniaWarning | EventKind::OxygenDip | EventKind::NitriteWarning
            )
        })
        .collect();
    assert!(
        relevant.iter().all(|e| !e.cause_codes.is_empty()),
        "Warning events must have non-empty cause_codes"
    );

    Ok(())
}
