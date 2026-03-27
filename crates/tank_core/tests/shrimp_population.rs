use tank_core::{
    EggCohort, Engine, EventKind, PlayerAction, ProcessParams, SimError, SimSeed, SimulationEngine,
    TankState,
};

/// Creates a well-conditioned tank with shrimp for population tests.
fn shrimp_test_state(seed: SimSeed) -> TankState {
    let geometry = tank_core::TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = tank_core::WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol; // high buffer to avoid pH crash from nitrification
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.periphyton_biomass_g = 5.0;

    state.microbe.decomposer_biomass_g = 0.1;
    state.microbe.ammonia_oxidizer_biomass_g = 0.8;
    state.microbe.nitrite_oxidizer_biomass_g = 0.5;
    state.microbe.comammox_biomass_g = 0.15;
    state.filter_state.biofilter_maturity_index = 1.0;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    state.animal.adults_count = 10;
    state.animal.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.5;

    state.process_params = ProcessParams::default();
    // Mature biofilter: faster nitrification to keep TAN near zero
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 5.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 5.0;
    // Well-established tank with good colonizable surfaces for periphyton
    state.process_params.periphyton_capacity_g_per_m2 = 30.0;

    // Initialize stability tracker from actual state to avoid day-1 instability spike
    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    // pH will be computed on first tick; use a reasonable estimate
    state.stability_tracker.prev_ph = 7.5;

    state
}

#[test]
fn spawning_creates_berried_females() -> Result<(), SimError> {
    let mut engine = Engine::from_parts(shrimp_test_state(SimSeed(7000)), vec![]);

    // Run for 30 days with light feeding
    for _ in 0..30 {
        engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        engine.step_hours(24)?;
    }

    let state = engine.full_state();
    let events = &state.event_log;

    // Over 30 days at good conditions, we should see ShrimpBerried events
    let berried_events = events
        .iter()
        .filter(|e| e.kind == EventKind::ShrimpBerried)
        .count();
    assert!(
        berried_events > 0,
        "Should have at least one ShrimpBerried event in 30 days of good conditions"
    );

    // All ShrimpBerried events must have non-empty cause codes
    for e in events.iter().filter(|e| e.kind == EventKind::ShrimpBerried) {
        assert!(
            !e.cause_codes.is_empty(),
            "ShrimpBerried must have cause codes"
        );
    }

    Ok(())
}

#[test]
fn hatch_produces_juveniles() -> Result<(), SimError> {
    let mut engine = Engine::from_parts(shrimp_test_state(SimSeed(7100)), vec![]);

    // Run for 60 days - enough for spawning + egg development (21 days) + hatching
    for _ in 0..60 {
        engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        engine.step_hours(24)?;
    }

    let snap = engine.snapshot();
    // In good conditions over 60 days, at least some juveniles should appear
    // (10 adults, 21-day egg cycle, good conditions)
    assert!(
        snap.juveniles_count > 0 || snap.adult_shrimp_count > 10,
        "After 60 days in good conditions, population should have grown. \
         Adults: {}, Juveniles: {}",
        snap.adult_shrimp_count,
        snap.juveniles_count
    );

    Ok(())
}

#[test]
fn egg_failure_under_stress() -> Result<(), SimError> {
    let mut state = shrimp_test_state(SimSeed(7200));
    // Force berried state with cohort tracking
    state.animal.berried_females_count = 3;
    state.animal.egg_progress_days = 20.0; // About to hatch
    state.animal.egg_cohorts = vec![EggCohort {
        count: 3,
        progress_days: 20.0,
    }];

    // Create stressful conditions: low DO, high temp
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 2.0 * vol; // Low DO
    state.water.temperature_c = 32.0;
    state.environment.ambient_temp_c = 32.0;
    state.water.calcium_mg_total = 5.0 * vol; // Low minerals
    state.water.magnesium_mg_total = 1.0 * vol;

    let mut engine = Engine::from_parts(state, vec![]);
    // Run one day to trigger clutch resolution
    engine.step_hours(24)?;

    let events = &engine.full_state().event_log;
    let egg_failures = events
        .iter()
        .filter(|e| e.kind == EventKind::EggFailure)
        .collect::<Vec<_>>();

    // Under these stressful conditions, egg failures should occur
    assert!(
        !egg_failures.is_empty(),
        "Should have EggFailure events under stress"
    );

    // EggFailure must have cause codes tied to oxygen, temperature, minerals, or instability
    for e in &egg_failures {
        assert!(
            !e.cause_codes.is_empty(),
            "EggFailure must have non-empty cause_codes"
        );
    }

    // Failure invariant: berried_females_count resolved (back to 0)
    let state = engine.full_state();
    assert_eq!(state.animal.berried_females_count, 0);
    assert_eq!(state.animal.egg_progress_days, 0.0);

    Ok(())
}

#[test]
fn mortality_under_combined_stress() -> Result<(), SimError> {
    let mut state = shrimp_test_state(SimSeed(7300));
    state.animal.adults_count = 20;
    state.animal.juveniles_count = 10;

    // Create combined low-DO + high-NH3 stress
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 1.5 * vol; // Very low DO
    state.water.ammonia_total_mg_n_total = 2.0 * vol; // Very high ammonia
    state.water.alkalinity_meq_total = 8.0 * vol; // High pH -> more NH3
    state.process_params.reaeration_kla_base = 0.05; // Slow recovery

    let mut engine = Engine::from_parts(state, vec![]);

    // Run 14 days under stress
    for _ in 0..14 {
        engine.step_hours(24)?;
    }

    let snap = engine.snapshot();
    let total_survivors = snap.adult_shrimp_count + snap.juveniles_count;
    assert!(
        total_survivors < 30,
        "Population should decline under combined stress. Survivors: {}",
        total_survivors
    );

    Ok(())
}

#[test]
fn microfauna_suppression_under_heavy_grazing() -> Result<(), SimError> {
    // Tank with many shrimp should suppress microfauna
    let mut state = shrimp_test_state(SimSeed(7400));
    state.animal.adults_count = 50; // Heavy stocking
    state.microfauna.population_index = 0.8; // Start with healthy microfauna

    let initial_pop = state.microfauna.population_index;

    let mut engine = Engine::from_parts(state, vec![]);
    for _ in 0..30 {
        engine.apply_action(PlayerAction::Feed { grams: 0.3 })?;
        engine.step_hours(24)?;
    }

    let snap = engine.snapshot();
    assert!(
        snap.microfauna_population_index < initial_pop,
        "Heavy shrimp grazing should suppress microfauna. Start: {}, End: {}",
        initial_pop,
        snap.microfauna_population_index
    );

    Ok(())
}

#[test]
fn shrimp_removal_validation() -> Result<(), SimError> {
    let mut state = shrimp_test_state(SimSeed(7500));
    state.animal.adults_count = 5;
    state.animal.berried_females_count = 2;

    let mut engine = Engine::from_parts(state, vec![]);

    // Try to remove more than available
    let result = engine.apply_action(PlayerAction::RemoveShrimp { count: 10 });
    assert!(
        matches!(result, Err(SimError::ShrimpRemovalExceedsAvailable { .. })),
        "Should fail when removing more shrimp than available"
    );

    // Verify state is unchanged after failed removal
    assert_eq!(engine.full_state().animal.adults_count, 5);
    assert_eq!(engine.full_state().animal.berried_females_count, 2);

    // Remove valid amount
    engine.apply_action(PlayerAction::RemoveShrimp { count: 3 })?;
    engine.step_hours(1)?;

    let state = engine.full_state();
    assert_eq!(state.animal.adults_count, 2);
    // berried_females_count must be <= adults_count
    assert!(
        state.animal.berried_females_count <= state.animal.adults_count,
        "berried_females ({}) must be <= adults ({})",
        state.animal.berried_females_count,
        state.animal.adults_count
    );

    Ok(())
}

#[test]
fn shrimp_removal_accounts_for_queued_actions() -> Result<(), SimError> {
    let mut state = shrimp_test_state(SimSeed(7600));
    state.animal.adults_count = 5;

    let mut engine = Engine::from_parts(state, vec![]);

    // Queue removal of 3
    engine.apply_action(PlayerAction::RemoveShrimp { count: 3 })?;

    // Try to remove 3 more (only 2 should be available)
    let result = engine.apply_action(PlayerAction::RemoveShrimp { count: 3 });
    assert!(
        matches!(result, Err(SimError::ShrimpRemovalExceedsAvailable { .. })),
        "Should fail because only 2 adults remain after pending removal"
    );

    // But removing 2 should succeed
    engine.apply_action(PlayerAction::RemoveShrimp { count: 2 })?;

    Ok(())
}

#[test]
fn population_fields_never_negative() -> Result<(), SimError> {
    let mut state = shrimp_test_state(SimSeed(7700));
    state.animal.adults_count = 3;
    state.animal.juveniles_count = 2;

    // Very hostile environment
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 0.5 * vol;
    state.water.ammonia_total_mg_n_total = 5.0 * vol;

    let mut engine = Engine::from_parts(state, vec![]);

    // Run many days under extreme stress
    for _ in 0..60 {
        engine.step_hours(24)?;
    }

    let state = engine.full_state();
    // All population counts must be non-negative (u32 can't be negative, but check invariants)
    assert!(state.animal.condition_index >= 0.0);
    assert!(state.animal.condition_index <= 1.0);
    assert!(state.animal.molt_stress_index >= 0.0);
    assert!(state.animal.molt_stress_index <= 1.0);
    assert!(state.animal.reproductive_readiness_index >= 0.0);
    assert!(state.animal.reproductive_readiness_index <= 1.0);
    assert!(state.animal.berried_females_count <= state.animal.adults_count);

    Ok(())
}
