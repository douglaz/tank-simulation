use tank_core::systems::shrimp::{molt_mineral_modifier, step_daily_shrimp};
use tank_core::{
    Engine, EventKind, PlayerAction, ProcessParams, ShrimpRuntimeParams, SimError, SimSeed,
    SimulationEngine, TankState, WaterState,
};

/// Creates a well-conditioned tank with shrimp for molt tests.
fn molt_test_state(seed: SimSeed) -> TankState {
    let geometry = tank_core::TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.set_periphyton_total(5.0);

    state.microbe.set_decomposer_total(0.1);
    state.microbe.ammonia_oxidizer_biomass_g = 0.8;
    state.microbe.nitrite_oxidizer_biomass_g = 1.2;
    state.microbe.comammox_biomass_g = 0.4;
    state.filter_state.biofilter_maturity_index = 1.0;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 5.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 8.0;
    state.process_params.comammox_vmax_fraction = 0.8;
    state.process_params.periphyton_capacity_g_per_m2 = 30.0;

    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = 7.5;

    state
}

/// Set up minerals for a given GH band.
fn set_minerals(state: &mut TankState, ca_mg_per_l: f64, mg_mg_per_l: f64) {
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = ca_mg_per_l * vol;
    state.water.magnesium_mg_total = mg_mg_per_l * vol;
    // Re-seed stability tracker so the GH change doesn't register as instability
    state.stability_tracker.prev_gh_d = state.gh_d();
}

// ── Test 1: GH monotonicity ──────────────────────────────────────────────

#[test]
fn test_molt_success_increases_with_gh() -> Result<()> {
    let params = ShrimpRuntimeParams::default();

    // GH 2° → low modifier
    let low = molt_mineral_modifier(2.0, 20.0, 5.0, &params);
    // GH 6° → normal modifier
    let normal = molt_mineral_modifier(6.0, 20.0, 5.0, &params);
    // GH 10° → also 1.0 (in optimal band)
    let high = molt_mineral_modifier(10.0, 20.0, 5.0, &params);

    // Monotonic increase up to optimal range
    assert!(
        low < normal,
        "GH 2 ({low:.3}) should give lower modifier than GH 6 ({normal:.3})"
    );
    assert!(
        normal <= high,
        "GH 6 ({normal:.3}) should not exceed GH 10 ({high:.3})"
    );

    // GH 2° should be clearly penalized
    assert!(
        low < 0.8,
        "GH 2° should produce a reduced modifier, got {low:.3}"
    );
    // GH 6-10° should be at or near optimal
    assert!(
        normal >= 0.9,
        "GH 6° (within optimal band) should be near 1.0, got {normal:.3}"
    );
    assert!(
        (high - 1.0).abs() < 0.01,
        "GH 10° (top of optimal band) should be ~1.0, got {high:.3}"
    );

    Ok(())
}

// ── Test 2: Both Ca and Mg contribute ────────────────────────────────────

#[test]
fn test_molt_success_formula_uses_ca_and_mg() -> Result<()> {
    let params = ShrimpRuntimeParams::default();
    let gh = 7.0; // in optimal band, gh_factor = 1.0

    // Full minerals
    let full = molt_mineral_modifier(gh, 25.0, 8.0, &params);
    // Low calcium only
    let low_ca = molt_mineral_modifier(gh, 5.0, 8.0, &params);
    // Low magnesium only
    let low_mg = molt_mineral_modifier(gh, 25.0, 1.0, &params);
    // Both low
    let both_low = molt_mineral_modifier(gh, 5.0, 1.0, &params);

    assert!(
        low_ca < full,
        "Low Ca ({low_ca:.3}) should reduce modifier vs full ({full:.3})"
    );
    assert!(
        low_mg < full,
        "Low Mg ({low_mg:.3}) should reduce modifier vs full ({full:.3})"
    );
    assert!(
        both_low < low_ca && both_low < low_mg,
        "Both low ({both_low:.3}) should be worse than either alone (Ca={low_ca:.3}, Mg={low_mg:.3})"
    );

    Ok(())
}

// ── Test 3: Condition affects molt success ───────────────────────────────

#[test]
fn test_molt_success_affected_by_condition() -> Result<()> {
    // Set up two scenarios: high condition vs low condition
    // Run one daily step with timer at the molt threshold
    let run_molt_with_condition = |condition: f64| -> bool {
        let mut state = molt_test_state(SimSeed(8100));
        set_minerals(&mut state, 40.0, 10.0); // good minerals, GH ~12
        state.animal.adult.count = 10;
        state.animal.set_population_condition_index(condition);
        state.animal.inter_molt_timer_days = state.shrimp_params.base_molt_interval_days;
        state.process_params.shrimp_condition_smoothing = 0.0; // freeze condition at set value
        state.process_params.shrimp_base_mortality_per_day = 0.0;
        state.process_params.shrimp_stress_mortality_scale = 0.0;
        state.shrimp_params.base_spawn_rate = 0.0;

        step_daily_shrimp(&mut state);
        state.animal.last_molt_success
    };

    let high_condition_succeeds = run_molt_with_condition(0.9);
    let low_condition_succeeds = run_molt_with_condition(0.05);

    assert!(
        high_condition_succeeds,
        "Well-fed shrimp (condition 0.9) should molt successfully"
    );
    assert!(
        !low_condition_succeeds,
        "Starved shrimp (condition 0.05) should fail molt"
    );

    Ok(())
}

// ── Test 4: Failed molts increase mortality ──────────────────────────────

#[test]
fn test_failed_molt_increases_mortality() -> std::result::Result<(), SimError> {
    // Tank with very low minerals → repeated molt failures → increased mortality
    let mut state = molt_test_state(SimSeed(8200));
    set_minerals(&mut state, 1.0, 0.1); // very low → GH ~0.2
    state.animal.adult.count = 50;
    state.animal.set_population_condition_index(0.3);
    // Short molt interval so failures accumulate quickly
    state.shrimp_params.base_molt_interval_days = 7.0;
    state.shrimp_params.juvenile_molt_interval_days = 3.0;
    state.shrimp_params.sub_adult_molt_interval_days = 5.0;

    let mut engine = Engine::from_parts(state, vec![]);

    for _ in 0..30 {
        engine.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        engine.step_hours(24)?;
    }

    let state = engine.full_state();
    assert!(
        state.animal.failed_molt_accum > 0.0,
        "Should have accumulated molt failures, got {}",
        state.animal.failed_molt_accum
    );
    assert!(
        state.animal.total_count() < 50,
        "Population should have declined from molt-failure mortality, got {}",
        state.animal.total_count()
    );

    Ok(())
}

// ── Test 5: Very low GH causes molt deaths ──────────────────────────────

#[test]
fn test_very_low_gh_causes_molt_deaths() -> std::result::Result<(), SimError> {
    let mut state = molt_test_state(SimSeed(8300));
    set_minerals(&mut state, 1.5, 0.3); // GH < 2°
    state.animal.adult.count = 30;
    state.animal.set_population_condition_index(0.5);
    state.shrimp_params.base_molt_interval_days = 7.0;

    let mut engine = Engine::from_parts(state, vec![]);

    for _ in 0..500 {
        engine.apply_action(PlayerAction::Feed { grams: 0.03 })?;
        engine.step_hours(1)?;
    }

    let state = engine.full_state();

    // Check for MoltFailure events
    let molt_failures: Vec<_> = state
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::MoltFailure)
        .collect();
    assert!(
        !molt_failures.is_empty(),
        "Should have MoltFailure events in very soft water (GH < 2)"
    );

    // Verify events have LowMinerals cause
    let has_low_minerals = molt_failures.iter().any(|e| {
        e.cause_codes
            .iter()
            .any(|c| *c == tank_core::EventCause::LowMinerals)
    });
    assert!(
        has_low_minerals,
        "MoltFailure events should cite LowMinerals as cause"
    );

    // Population should have declined
    assert!(
        state.animal.total_count() < 30,
        "Population should decline in very soft water, got {}",
        state.animal.total_count()
    );

    Ok(())
}

// ── Test 6: Thresholds are named parameters ──────────────────────────────

#[test]
fn test_mineral_modifier_named_parameters() -> Result<()> {
    // All thresholds should be overridable via ShrimpRuntimeParams
    let mut params = ShrimpRuntimeParams::default();

    // Default thresholds
    let default_result = molt_mineral_modifier(3.0, 10.0, 3.0, &params);

    // Override GH threshold to be more permissive
    params.gh_min_d = 2.0;
    let permissive_gh = molt_mineral_modifier(3.0, 10.0, 3.0, &params);
    assert!(
        permissive_gh > default_result,
        "Lowering gh_min_d should increase modifier at GH 3 (default={default_result:.3}, permissive={permissive_gh:.3})"
    );
    params.gh_min_d = 5.0; // restore

    // Override Ca threshold
    params.ca_min_mg_per_l = 5.0; // more permissive
    let permissive_ca = molt_mineral_modifier(3.0, 10.0, 3.0, &params);
    assert!(
        permissive_ca > default_result,
        "Lowering ca_min_mg_per_l should increase modifier (default={default_result:.3}, permissive={permissive_ca:.3})"
    );
    params.ca_min_mg_per_l = 20.0; // restore

    // Override Mg threshold
    params.mg_min_mg_per_l = 2.0; // more permissive
    let permissive_mg = molt_mineral_modifier(3.0, 10.0, 3.0, &params);
    assert!(
        permissive_mg > default_result,
        "Lowering mg_min_mg_per_l should increase modifier (default={default_result:.3}, permissive={permissive_mg:.3})"
    );

    // Verify molt_success_threshold is a named parameter
    assert!(
        (params.molt_success_threshold - 0.55).abs() < f64::EPSILON,
        "molt_success_threshold should be accessible as a named parameter"
    );

    // Verify per-stage molt intervals are named parameters
    assert!(params.juvenile_molt_interval_days < params.base_molt_interval_days);
    assert!(params.sub_adult_molt_interval_days < params.base_molt_interval_days);

    Ok(())
}

// ── Test 7: Molt frequency varies by stage ──────────────────────────────

#[test]
fn test_molt_frequency_varies_by_stage() -> Result<()> {
    let params = ShrimpRuntimeParams::default();

    // Verify the interval parameters exist and juveniles molt faster
    assert!(
        params.juvenile_molt_interval_days < params.sub_adult_molt_interval_days,
        "Juvenile interval ({}) should be shorter than sub-adult ({})",
        params.juvenile_molt_interval_days,
        params.sub_adult_molt_interval_days
    );
    assert!(
        params.sub_adult_molt_interval_days < params.base_molt_interval_days,
        "Sub-adult interval ({}) should be shorter than adult ({})",
        params.sub_adult_molt_interval_days,
        params.base_molt_interval_days
    );

    // Run a simulation with mixed stages and verify per-stage timers advance independently
    let mut state = molt_test_state(SimSeed(8400));
    set_minerals(&mut state, 40.0, 10.0); // good minerals
    state.animal.adult.count = 5;
    state.animal.sub_adult.count = 5;
    state.animal.juvenile.count = 5;
    state.animal.set_population_condition_index(0.8);
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.shrimp_params.failed_molt_mortality_scale = 0.0;
    // Prevent maturation so stage counts stay fixed
    state.shrimp_params.juvenile_to_subadult_days = 999.0;
    state.shrimp_params.subadult_to_adult_days = 999.0;
    // Start timers from zero (default inter_molt_timer_days=14 would trigger migration)
    state.animal.inter_molt_timer_days = 0.0;

    // Run for juvenile molt interval days + 1 at optimal temp
    let juv_interval = state.shrimp_params.juvenile_molt_interval_days as u32;
    for _ in 0..=juv_interval {
        step_daily_shrimp(&mut state);
    }

    // Juvenile timer should have reset (molted) while adult timer should still be counting
    assert!(
        state.animal.juvenile.molt_timer_days < state.animal.adult.molt_timer_days,
        "Juvenile timer ({:.1}) should be less than adult timer ({:.1}) after {} days",
        state.animal.juvenile.molt_timer_days,
        state.animal.adult.molt_timer_days,
        juv_interval + 1,
    );

    Ok(())
}

// ── Test 8: Soft vs hard water integration test ──────────────────────────

#[test]
fn test_soft_vs_hard_water_shrimp_survival() -> std::result::Result<(), SimError> {
    let run_tank = |ca: f64, mg: f64, seed: u64| -> std::result::Result<u32, SimError> {
        let mut state = molt_test_state(SimSeed(seed));
        set_minerals(&mut state, ca, mg);
        state.animal.adult.count = 20;
        state.animal.set_population_condition_index(0.8);
        state.shrimp_params.base_molt_interval_days = 14.0;
        state.shrimp_params.juvenile_molt_interval_days = 7.0;
        state.shrimp_params.sub_adult_molt_interval_days = 10.0;
        state.shrimp_params.base_spawn_rate = 0.0;

        let mut engine = Engine::from_parts(state, vec![]);

        // Run ~15 days — long enough for one molt cycle to differentiate
        for _ in 0..360 {
            engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
            engine.step_hours(1)?;
        }

        Ok(engine.full_state().animal.total_count())
    };

    // Hard water: GH ~9.3 (Ca=42, Mg=15)
    let hard_pop = run_tank(42.0, 15.0, 9000)?;
    // Soft water: GH ~1.7 (Ca=3, Mg=1) — well below recommended
    let soft_pop = run_tank(3.0, 1.0, 9001)?;

    assert!(
        hard_pop > soft_pop,
        "Hard water population ({hard_pop}) should exceed soft water ({soft_pop})"
    );

    // Hard water should maintain reasonable population
    assert!(
        hard_pop >= 10,
        "Hard water tank should maintain population, got {hard_pop}"
    );

    Ok(())
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
