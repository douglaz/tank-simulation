//! Tests for photosynthesis/respiration coupling to DIC, CO2, and day–night pH behavior.
//!
//! Covers the acceptance criteria from tanksim-6e5.4.4:
//! - Light cycle changes affect DIC / CO2 / pH in a believable direction
//! - Defaults are no longer effectively disabling the pathway
//! - Stoichiometric consistency between O2 and DIC fluxes
//! - Diurnal pH swing in planted tanks

use tank_core::{Engine, SimSeed, SimulationEngine, TankState};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a planted tank state suitable for DIC/pH testing.
///
/// Features:
/// - High plant biomass to amplify photosynthesis/respiration signals
/// - Zero K_LA to isolate biological DIC effects from atmospheric CO2 exchange
/// - 12h/12h photoperiod centered on noon
/// - Moderate DIC and alkalinity for realistic pH buffering
fn planted_dic_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    let volume_l = state.water_volume_l();

    // Generous plant biomass to produce clear day/night signal.
    state.plant_guilds[0].biomass_g = 15.0;
    state.plant_guilds[1].biomass_g = 10.0;
    state.algae.periphyton_biomass_g = 2.0;
    state.algae.suspended_biomass_g = 0.5;

    // Moderate DIC and alkalinity for stable pH range.
    state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    state.water.alkalinity_meq_total = 1.5 * volume_l;
    state.water.temperature_c = 25.0;

    // 12h/12h photoperiod.
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;

    // Some animal biomass for respiration.
    state.animal.adult.count = 10;

    // Zero K_LA to isolate biological DIC from atmospheric exchange.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.flow_lph = 0.0;

    // Use the stoichiometric defaults (already set by ProcessParams::default()).
    // Verify they are non-zero.
    assert!(
        state
            .process_params
            .respiration_dic_rate_mg_c_per_g_per_hour
            > 0.0,
        "default respiration DIC rate must be non-zero"
    );
    assert!(
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour
            > 0.0,
        "default photosynthesis DIC rate must be non-zero"
    );

    // Resolve carbonate state after setting DIC/alk.
    tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);
    state
}

// ---------------------------------------------------------------------------
// 1. Photosynthesis consumes DIC during lit hours
// ---------------------------------------------------------------------------

#[test]
fn test_photosynthesis_consumes_dic() -> Result<(), tank_core::SimError> {
    // Start at midday (hour 12) so light is on.
    let mut state = planted_dic_state(SimSeed(10_000));
    state.environment.hour_of_day = 12;

    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let dic_after = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;
    assert!(
        dic_after < dic_before,
        "photosynthesis during lit hours should consume DIC: before={dic_before:.4}, after={dic_after:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Respiration produces DIC during dark hours
// ---------------------------------------------------------------------------

#[test]
fn test_respiration_produces_dic() -> Result<(), tank_core::SimError> {
    // Start at midnight (hour 0) so light is off.
    let mut state = planted_dic_state(SimSeed(10_100));
    state.environment.hour_of_day = 0;

    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let dic_after = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;
    assert!(
        dic_after > dic_before,
        "respiration during dark hours should produce DIC: before={dic_before:.4}, after={dic_after:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Day/night pH swing > 0.1 in planted tank over 48h
// ---------------------------------------------------------------------------

#[test]
fn test_day_night_ph_swing() -> Result<(), tank_core::SimError> {
    // Start at hour 0 (midnight), run for 48 hours with 12h/12h cycle.
    let mut state = planted_dic_state(SimSeed(10_200));
    state.environment.hour_of_day = 0;
    state.environment.day = 1;

    let mut engine = Engine::from_parts(state, vec![]);

    // Track pH at end of each light and dark period over 2 full cycles.
    let mut ph_end_of_light = Vec::new();
    let mut ph_end_of_dark = Vec::new();

    // Collect pH at every hour for the full 48h period.
    let mut ph_trace: Vec<(u8, f64)> = Vec::new();
    for _hour in 1..=48 {
        engine.step_hours(1)?;
        let s = engine.full_state();
        ph_trace.push((s.environment.hour_of_day, s.water.ph));
    }

    // Extract end-of-light (hour 18) and end-of-dark (hour 6) from day 2
    // (indices 24-47) to allow transient settling on day 1.
    for &(h, ph) in &ph_trace[24..] {
        if h == 18 {
            ph_end_of_light.push(ph);
        }
        if h == 6 {
            ph_end_of_dark.push(ph);
        }
    }

    assert!(
        !ph_end_of_light.is_empty(),
        "should capture at least one end-of-light pH"
    );
    assert!(
        !ph_end_of_dark.is_empty(),
        "should capture at least one end-of-dark pH"
    );

    // pH should be higher at end-of-light than end-of-dark.
    for (light_ph, dark_ph) in ph_end_of_light.iter().zip(ph_end_of_dark.iter()) {
        assert!(
            light_ph > dark_ph,
            "pH at end-of-light ({light_ph:.3}) should exceed pH at end-of-dark ({dark_ph:.3})"
        );
    }

    // Swing magnitude should be at least 0.1 pH units.
    let max_light_ph = ph_end_of_light.iter().cloned().reduce(f64::max).unwrap();
    let min_dark_ph = ph_end_of_dark.iter().cloned().reduce(f64::min).unwrap();
    let swing = max_light_ph - min_dark_ph;
    assert!(
        swing >= 0.1,
        "day-night pH swing ({swing:.3}) should be >= 0.1 units \
         (light_max={max_light_ph:.3}, dark_min={min_dark_ph:.3})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Heavy plant load → larger pH swing
// ---------------------------------------------------------------------------

#[test]
fn test_heavy_plant_load_larger_ph_swing() -> Result<(), tank_core::SimError> {
    let make_engine = |plant_scale: f64, seed: SimSeed| -> Engine {
        let mut state = planted_dic_state(seed);
        state.environment.hour_of_day = 0;
        state.plant_guilds[0].biomass_g = 15.0 * plant_scale;
        state.plant_guilds[1].biomass_g = 10.0 * plant_scale;
        let volume_l = state.water_volume_l();
        tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);
        Engine::from_parts(state, vec![])
    };

    let mut engine_1x = make_engine(1.0, SimSeed(10_300));
    let mut engine_2x = make_engine(2.0, SimSeed(10_300));

    // Run 24 hours and collect pH range.
    let ph_range = |engine: &mut Engine| -> Result<f64, tank_core::SimError> {
        let mut min_ph = f64::MAX;
        let mut max_ph = f64::MIN;
        for _ in 0..24 {
            engine.step_hours(1)?;
            let ph = engine.full_state().water.ph;
            min_ph = min_ph.min(ph);
            max_ph = max_ph.max(ph);
        }
        Ok(max_ph - min_ph)
    };

    let swing_1x = ph_range(&mut engine_1x)?;
    let swing_2x = ph_range(&mut engine_2x)?;

    assert!(
        swing_2x > swing_1x,
        "2× plant biomass should produce larger pH swing: 1×={swing_1x:.4}, 2×={swing_2x:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. No plants → minimal pH swing (only microbial respiration)
// ---------------------------------------------------------------------------

#[test]
fn test_no_plants_minimal_ph_swing() -> Result<(), tank_core::SimError> {
    let mut state = planted_dic_state(SimSeed(10_400));
    state.environment.hour_of_day = 0;
    // Remove all photosynthetic biomass.
    for guild in &mut state.plant_guilds {
        guild.biomass_g = 0.0;
    }
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    let volume_l = state.water_volume_l();
    tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);

    let mut engine = Engine::from_parts(state, vec![]);

    let mut min_ph = f64::MAX;
    let mut max_ph = f64::MIN;
    for _ in 0..48 {
        engine.step_hours(1)?;
        let ph = engine.full_state().water.ph;
        min_ph = min_ph.min(ph);
        max_ph = max_ph.max(ph);
    }

    // Compare with a planted tank.
    let mut planted_state = planted_dic_state(SimSeed(10_400));
    planted_state.environment.hour_of_day = 0;
    let volume_l = planted_state.water_volume_l();
    tank_core::systems::chemistry::resolve_carbonate_state(&mut planted_state.water, volume_l);
    let mut planted_engine = Engine::from_parts(planted_state, vec![]);

    let mut planted_min_ph = f64::MAX;
    let mut planted_max_ph = f64::MIN;
    for _ in 0..48 {
        planted_engine.step_hours(1)?;
        let ph = planted_engine.full_state().water.ph;
        planted_min_ph = planted_min_ph.min(ph);
        planted_max_ph = planted_max_ph.max(ph);
    }

    let no_plant_swing = max_ph - min_ph;
    let planted_swing = planted_max_ph - planted_min_ph;

    assert!(
        no_plant_swing < planted_swing,
        "no-plant pH swing ({no_plant_swing:.4}) should be less than planted ({planted_swing:.4})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Photosynthesis O2/DIC stoichiometry (within 5%)
// ---------------------------------------------------------------------------

#[test]
fn test_photosynthesis_o2_dic_stoichiometry() -> Result<(), tank_core::SimError> {
    // Isolate photosynthesis: zero respiration rates, run 1 lit hour.
    let mut state = planted_dic_state(SimSeed(10_500));
    state.environment.hour_of_day = 12; // midday, light on

    // Zero respiration so only photosynthesis drives DIC and O2.
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    // Zero nitrifiers to prevent O2 consumption from nitrification.
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;

    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;
    let do_before = state.water.dissolved_oxygen_mg_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let dic_after = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;
    let do_after = engine.full_state().water.dissolved_oxygen_mg_total;

    let dic_consumed = dic_before - dic_after;
    let o2_produced = do_after - do_before;

    assert!(
        dic_consumed > 0.0,
        "photosynthesis should consume DIC: delta={dic_consumed:.6}"
    );
    assert!(
        o2_produced > 0.0,
        "photosynthesis should produce O2: delta={o2_produced:.6}"
    );

    // Stoichiometric ratio: 12g C per 32g O2 = 0.375
    let expected_ratio = 12.0 / 32.0;
    let actual_ratio = dic_consumed / o2_produced;
    let relative_error = ((actual_ratio - expected_ratio) / expected_ratio).abs();

    assert!(
        relative_error < 0.05,
        "photosynthesis DIC:O2 ratio ({actual_ratio:.6}) should be within 5% of \
         stoichiometric ({expected_ratio:.6}), relative error = {relative_error:.6}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Respiration O2/DIC stoichiometry (within 5%)
// ---------------------------------------------------------------------------

#[test]
fn test_respiration_o2_dic_stoichiometry() -> Result<(), tank_core::SimError> {
    // Isolate respiration: zero photosynthesis rates, run 1 dark hour.
    let mut state = planted_dic_state(SimSeed(10_600));
    state.environment.hour_of_day = 0; // midnight, light off

    // Zero photosynthesis so only respiration drives DIC and O2.
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    // Zero nitrifiers to prevent O2 consumption from nitrification.
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;

    // Ensure plenty of O2 so respiration is not O2-limited.
    let volume_l = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;

    let dic_before = state.water.dissolved_inorganic_carbon_mg_c_total;
    let do_before = state.water.dissolved_oxygen_mg_total;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let dic_after = engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;
    let do_after = engine.full_state().water.dissolved_oxygen_mg_total;

    let dic_produced = dic_after - dic_before;
    let o2_consumed = do_before - do_after;

    assert!(
        dic_produced > 0.0,
        "respiration should produce DIC: delta={dic_produced:.6}"
    );
    assert!(
        o2_consumed > 0.0,
        "respiration should consume O2: delta={o2_consumed:.6}"
    );

    // Stoichiometric ratio: 12g C per 32g O2 = 0.375
    let expected_ratio = 12.0 / 32.0;
    let actual_ratio = dic_produced / o2_consumed;
    let relative_error = ((actual_ratio - expected_ratio) / expected_ratio).abs();

    assert!(
        relative_error < 0.05,
        "respiration DIC:O2 ratio ({actual_ratio:.6}) should be within 5% of \
         stoichiometric ({expected_ratio:.6}), relative error = {relative_error:.6}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Integration: medium_planted scenario, 168h (1 week), diurnal pH pattern
// ---------------------------------------------------------------------------

#[test]
fn test_planted_tank_day_night_cycle() -> Result<(), tank_core::SimError> {
    let state = tank_scenarios::seeded_state(SimSeed(10_700), "medium_planted")
        .expect("medium_planted scenario should materialize");

    let mut engine = Engine::from_parts(state, vec![]);

    let mut ph_trajectory: Vec<f64> = Vec::new();
    let mut dic_trajectory: Vec<f64> = Vec::new();

    // Run for 168 hours (1 week).
    for _ in 0..168 {
        engine.step_hours(1)?;
        let s = engine.full_state();
        ph_trajectory.push(s.water.ph);
        dic_trajectory.push(s.water.dissolved_inorganic_carbon_mg_c_total);
    }

    // pH must stay within storage bounds at all times.
    for (i, &ph) in ph_trajectory.iter().enumerate() {
        assert!(
            (5.5..=8.5).contains(&ph),
            "pH out of bounds at hour {}: {ph:.3}",
            i + 1
        );
    }

    // DIC should not be constant — the photosynthesis/respiration pathway
    // and atmospheric exchange should exercise it.
    let dic_min = dic_trajectory.iter().cloned().reduce(f64::min).unwrap();
    let dic_max = dic_trajectory.iter().cloned().reduce(f64::max).unwrap();
    assert!(
        dic_max - dic_min > 0.1,
        "DIC should vary over 168h: min={dic_min:.2}, max={dic_max:.2}"
    );

    // DIC should remain positive and finite.
    for (i, &dic) in dic_trajectory.iter().enumerate() {
        assert!(
            dic >= 0.0 && dic.is_finite(),
            "DIC non-finite at hour {}: {dic}",
            i + 1
        );
    }

    // Verify hourly photosynthesis/respiration is actually coupled:
    // during consecutive lit hours, pH should generally increase (or at least
    // not monotonically decrease). Check the FIRST lit period of day 2
    // (hours 7-16 of day 2, which is index 31-40 in the trajectory).
    if ph_trajectory.len() >= 41 {
        // Hourly photosynthesis removes DIC → pH rises during lit hours.
        // This may be partially offset by atmospheric exchange but should
        // still show a net upward trend.
        let lit_start_ph = ph_trajectory[31]; // hour 7 of day 2
        let lit_end_ph = ph_trajectory[40]; // hour 16 of day 2
        assert!(
            lit_end_ph >= lit_start_ph - 0.05,
            "pH should not significantly decline during lit hours: \
             start={lit_start_ph:.3}, end={lit_end_ph:.3}"
        );
    }

    // Plants should still be alive and healthy.
    let final_state = engine.full_state();
    let total_plant_biomass: f64 = final_state.plant_guilds.iter().map(|p| p.biomass_g).sum();
    assert!(
        total_plant_biomass > 0.5,
        "plants should survive a 1-week simulation, got {total_plant_biomass:.2}g"
    );

    // At least one plant guild should have reasonable health.
    let max_health = final_state
        .plant_guilds
        .iter()
        .map(|p| p.health_index)
        .reduce(f64::max)
        .unwrap_or(0.0);
    assert!(
        max_health > 0.3,
        "at least one plant guild should maintain health > 0.3, got {max_health:.2}"
    );

    Ok(())
}
