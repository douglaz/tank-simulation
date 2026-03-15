use tank_core::{Engine, PlayerAction, SimError, SimSeed, SimulationEngine, SourceWaterProfile, TankState};

/// Helper: build a state with a known source-water catalog containing `ro_like`.
fn state_with_ro_like(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state
        .source_water_catalog
        .insert("ro_like".to_string(), SourceWaterProfile::zero());
    state
}

#[test]
fn water_change_50_percent_ro_like_halves_dissolved_totals() -> Result<(), tank_core::SimError> {
    let state = state_with_ro_like(SimSeed(100));
    let mut engine = Engine::from_parts(state, vec![]);

    // Record pre-change totals
    let before = engine.full_state().water.clone();

    // Apply 50% water change with zero-nutrient source
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "ro_like".to_string(),
    })?;
    engine.step_hours(1)?;

    let after = &engine.full_state().water;

    // Each dissolved total should be reduced by 50% ± 0.5%
    let tolerance = 0.005; // 0.5%
    let expected_ratio = 0.5;

    let checks: Vec<(&str, f64, f64)> = vec![
        (
            "ammonia_total_mg_n_total",
            before.ammonia_total_mg_n_total,
            after.ammonia_total_mg_n_total,
        ),
        (
            "nitrite_mg_n_total",
            before.nitrite_mg_n_total,
            after.nitrite_mg_n_total,
        ),
        (
            "nitrate_mg_n_total",
            before.nitrate_mg_n_total,
            after.nitrate_mg_n_total,
        ),
        (
            "phosphate_mg_p_total",
            before.phosphate_mg_p_total,
            after.phosphate_mg_p_total,
        ),
        (
            "dissolved_inorganic_carbon_mg_c_total",
            before.dissolved_inorganic_carbon_mg_c_total,
            after.dissolved_inorganic_carbon_mg_c_total,
        ),
        (
            "calcium_mg_total",
            before.calcium_mg_total,
            after.calcium_mg_total,
        ),
        (
            "magnesium_mg_total",
            before.magnesium_mg_total,
            after.magnesium_mg_total,
        ),
        (
            "sodium_mg_total",
            before.sodium_mg_total,
            after.sodium_mg_total,
        ),
        (
            "potassium_mg_total",
            before.potassium_mg_total,
            after.potassium_mg_total,
        ),
        (
            "bicarbonate_mg_total",
            before.bicarbonate_mg_total,
            after.bicarbonate_mg_total,
        ),
        (
            "chloride_mg_total",
            before.chloride_mg_total,
            after.chloride_mg_total,
        ),
        (
            "sulfate_mg_total",
            before.sulfate_mg_total,
            after.sulfate_mg_total,
        ),
    ];

    for (name, before_val, after_val) in &checks {
        if *before_val > 0.0 {
            let actual_ratio = after_val / before_val;
            assert!(
                (actual_ratio - expected_ratio).abs() < tolerance,
                "{name}: expected ratio {expected_ratio} ± {tolerance}, got {actual_ratio} (before={before_val}, after={after_val})"
            );
        } else {
            // If before was 0.0, after should also be 0.0 (ro_like adds nothing)
            assert!(
                after_val.abs() < f64::EPSILON,
                "{name}: expected 0.0 after zero water change, got {after_val}"
            );
        }
    }

    Ok(())
}

#[test]
fn water_change_0_percent_is_noop() -> Result<(), tank_core::SimError> {
    let state = state_with_ro_like(SimSeed(200));
    let mut engine = Engine::from_parts(state, vec![]);

    let before = engine.full_state().water.clone();

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 0.0,
        source_profile_id: "ro_like".to_string(),
    })?;
    engine.step_hours(1)?;

    let after = &engine.full_state().water;

    // Chemistry should be unchanged (temperature may shift from ambient model)
    assert!(
        (after.calcium_mg_total - before.calcium_mg_total).abs() < f64::EPSILON,
        "0% water change should not change calcium"
    );
    assert!(
        (after.alkalinity_meq_total - before.alkalinity_meq_total).abs() < f64::EPSILON,
        "0% water change should not change alkalinity"
    );

    Ok(())
}

#[test]
fn water_change_100_percent_yields_source_temperature() -> Result<(), tank_core::SimError> {
    let mut state = state_with_ro_like(SimSeed(300));
    // Add a source with different temperature
    let mut warm_source = SourceWaterProfile::zero();
    warm_source.temperature_c = 30.0;
    state
        .source_water_catalog
        .insert("warm".to_string(), warm_source);

    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 100.0,
        source_profile_id: "warm".to_string(),
    })?;
    engine.step_hours(1)?;

    // After 100% water change, temperature should be very close to source temp
    // (may shift slightly from the temperature system running after the change)
    let temp = engine.full_state().water.temperature_c;
    assert!(
        (temp - 30.0).abs() < 1.0,
        "100% water change should bring temp near source: got {temp}"
    );

    Ok(())
}

#[test]
fn water_change_50_percent_mixes_temperature() -> Result<(), tank_core::SimError> {
    let mut state = state_with_ro_like(SimSeed(400));
    let mut cold_source = SourceWaterProfile::zero();
    cold_source.temperature_c = 10.0;
    state
        .source_water_catalog
        .insert("cold".to_string(), cold_source);
    // Tank starts at 24°C, ambient is 24°C
    state.water.temperature_c = 24.0;

    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "cold".to_string(),
    })?;
    engine.step_hours(1)?;

    // Expected mixed temp: 24*0.5 + 10*0.5 = 17°C, plus thermal system may shift slightly
    let temp = engine.full_state().water.temperature_c;
    assert!(
        temp > 14.0 && temp < 20.0,
        "50% cold water change should mix temperature proportionally: got {temp}"
    );

    Ok(())
}

#[test]
fn unknown_source_profile_returns_error() -> Result<(), tank_core::SimError> {
    let state = TankState::new(SimSeed(500)); // empty catalog
    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "nonexistent".to_string(),
    })?;

    let result = engine.step_hours(1);
    assert!(
        matches!(result, Err(tank_core::SimError::UnknownSourceProfile { .. })),
        "Expected UnknownSourceProfile error, got: {result:?}"
    );

    Ok(())
}

#[test]
fn unknown_profile_leaves_state_unchanged() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(600));
    // Feed first to have some detritus, then queue a bad water change
    state.detritus.particulate_organics_g_total = 5.0;
    let before = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);

    // Queue both a feed and a bad water change
    engine.apply_action(PlayerAction::Feed { grams: 1.0 })?;
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "bad_id".to_string(),
    })?;

    let result = engine.step_hours(1);
    assert!(result.is_err());

    // State should be exactly unchanged: day, hour, event_log, water, rng all pristine
    let after = engine.full_state();
    assert_eq!(before.environment.day, after.environment.day);
    assert_eq!(before.environment.hour_of_day, after.environment.hour_of_day);
    assert_eq!(before.event_log.len(), after.event_log.len());
    assert!(
        (before.water.calcium_mg_total - after.water.calcium_mg_total).abs() < f64::EPSILON,
    );

    Ok(())
}

#[test]
fn invalid_resolved_profile_returns_error_and_leaves_state_unchanged() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(700));
    // Insert a profile with NaN in a chemistry field — simulating corrupted saved state
    let mut bad_profile = SourceWaterProfile::zero();
    bad_profile.calcium_mg_per_l = f64::NAN;
    state
        .source_water_catalog
        .insert("corrupted".to_string(), bad_profile);

    let before = state.clone();
    let mut engine = Engine::from_parts(state, vec![]);

    // Queue a feed and a water change referencing the invalid profile
    engine.apply_action(PlayerAction::Feed { grams: 1.0 })?;
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "corrupted".to_string(),
    })?;

    let result = engine.step_hours(1);
    assert!(
        matches!(result, Err(SimError::InvalidSourceProfile { .. })),
        "Expected InvalidSourceProfile error, got: {result:?}"
    );

    // State should be exactly unchanged
    let after = engine.full_state();
    assert_eq!(before.environment.day, after.environment.day);
    assert_eq!(before.environment.hour_of_day, after.environment.hour_of_day);
    assert_eq!(before.event_log.len(), after.event_log.len());
    assert!(
        (before.water.calcium_mg_total - after.water.calcium_mg_total).abs() < f64::EPSILON,
    );
    assert!(
        (before.water.ammonia_total_mg_n_total - after.water.ammonia_total_mg_n_total).abs()
            < f64::EPSILON,
    );
    // Detritus should not have been modified (feed not applied)
    assert!(
        (before.detritus.particulate_organics_g_total - after.detritus.particulate_organics_g_total)
            .abs()
            < f64::EPSILON,
    );

    Ok(())
}

#[test]
fn invalid_resolved_profile_negative_chemistry_returns_error() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(800));
    let mut bad_profile = SourceWaterProfile::zero();
    bad_profile.nitrate_mg_n_per_l = -5.0;
    state
        .source_water_catalog
        .insert("negative".to_string(), bad_profile);

    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 25.0,
        source_profile_id: "negative".to_string(),
    })?;

    let result = engine.step_hours(1);
    assert!(
        matches!(result, Err(SimError::InvalidSourceProfile { .. })),
        "Expected InvalidSourceProfile error for negative chemistry, got: {result:?}"
    );

    Ok(())
}
