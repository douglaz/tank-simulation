use tank_core::systems::shrimp::step_daily_shrimp;
use tank_core::{
    EggCohort, Engine, EventCause, EventKind, PlayerAction, ProcessParams, SimError, SimSeed,
    SimulationEngine, SourceWaterProfile, TankGeometry, TankState, WaterState,
};

// ── Test fixture ───────────────────────────────────────────────────────────

/// Creates a well-maintained tank at optimal conditions with shrimp ready to breed.
/// All suppression factors are at their baseline so individual factors can be
/// isolated by the caller.
fn breeding_fixture(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
        length_cm: 60.0,
        width_cm: 40.0,
        height_cm: 40.0,
        fill_height_cm: 35.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0; // optimal
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 10.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    // Keep DOC/DON low to avoid transient ammonia spike from mineralization
    state.water.dissolved_organic_carbon_mg_c_total = 5.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.8;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;
    // Clean chemistry: no ammonia, no nitrite
    state.water.ammonia_total_mg_n_total = 0.0;
    state.water.nitrite_mg_n_total = 0.0;
    state.water.nitrate_mg_n_total = 5.0;

    // Abundant food
    state.algae.set_periphyton_total(30.0);

    // Strong biofilter
    state.microbe.set_decomposer_total(0.3);
    state.microbe.ammonia_oxidizer_biomass_g = 2.0;
    state.microbe.nitrite_oxidizer_biomass_g = 1.5;
    state.microbe.comammox_biomass_g = 0.5;
    state.filter_state.biofilter_maturity_index = 1.0;

    // Good aeration
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;

    // Light for periphyton regrowth
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.7;
    state.hardware.light.photoperiod_hours = 10.0;

    // Moderate population, well-conditioned adults
    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.8;
    state.animal.adult.reserve_g = 5.0;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.8;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.periphyton_capacity_g_per_m2 = 200.0;
    // Disable mortality to isolate reproduction effects
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;

    // Fast spawning for test visibility
    state.shrimp_params.base_spawn_rate = 0.15;
    state.shrimp_params.hatch_success_base = 1.0;
    state.shrimp_params.egg_duration_days = 14;
    state
        .shrimp_params
        .apply_legacy_total_maturation_days(120.0);

    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = 7.5;
    state.stability_tracker.instability_index = 0.0;

    state
}

fn breeding_fixture_with_volume(seed: SimSeed, target_volume_l: f64) -> TankState {
    let mut state = breeding_fixture(seed);

    let (length_cm, width_cm, fill_height_cm) = if target_volume_l <= 20.0 {
        let side_cm = 20.0;
        let fill_height_cm = target_volume_l * 1000.0 / (side_cm * side_cm);
        (side_cm, side_cm, fill_height_cm)
    } else {
        let length_cm = 50.0;
        let width_cm = 40.0;
        let fill_height_cm = target_volume_l * 1000.0 / (length_cm * width_cm);
        (length_cm, width_cm, fill_height_cm)
    };

    state.geometry = TankGeometry {
        length_cm,
        width_cm,
        height_cm: fill_height_cm + 5.0,
        fill_height_cm,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 10.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 5.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.8;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;
    state.water.ammonia_total_mg_n_total = 0.0;
    state.water.nitrite_mg_n_total = 0.0;
    state.water.nitrate_mg_n_total = 5.0;

    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = 7.5;
    state.stability_tracker.instability_index = 0.0;
    state.hardware.heater.enabled = false;

    state
}

fn run_day(engine: &mut Engine, feed_grams: f64) -> Result<(), SimError> {
    engine.apply_action(PlayerAction::Feed { grams: feed_grams })?;
    engine.step_hours(24)?;
    Ok(())
}

#[derive(Debug, Default, Clone, Copy)]
struct DayPeaks {
    tan_mg_n_per_l: f64,
    nitrite_mg_n_per_l: f64,
    instability_index: f64,
}

fn run_day_with_hourly_peaks(engine: &mut Engine, feed_grams: f64) -> Result<DayPeaks, SimError> {
    engine.apply_action(PlayerAction::Feed { grams: feed_grams })?;
    let mut peaks = DayPeaks::default();
    for _ in 0..24 {
        engine.step_hours(1)?;
        let snapshot = engine.snapshot();
        peaks.tan_mg_n_per_l = peaks.tan_mg_n_per_l.max(snapshot.tan_mg_n_per_l);
        peaks.nitrite_mg_n_per_l = peaks.nitrite_mg_n_per_l.max(snapshot.nitrite_mg_n_per_l);
        peaks.instability_index = peaks
            .instability_index
            .max(engine.full_state().stability_tracker.instability_index);
    }
    Ok(peaks)
}

fn load_source_profile(id: &str) -> SourceWaterProfile {
    let preset = tank_data::load_source_water(id)
        .unwrap_or_else(|e| panic!("failed to load source water preset '{id}': {e}"));
    SourceWaterProfile {
        temperature_c: preset.temperature_c,
        ammonia_mg_n_per_l: preset.ammonia_mg_n_per_l,
        nitrite_mg_n_per_l: preset.nitrite_mg_n_per_l,
        nitrate_mg_n_per_l: preset.nitrate_mg_n_per_l,
        phosphate_mg_p_per_l: preset.phosphate_mg_p_per_l,
        dic_mg_c_per_l: preset.dic_mg_c_per_l,
        doc_mg_c_per_l: preset.doc_mg_c_per_l,
        don_mg_n_per_l: preset.don_mg_n_per_l,
        alkalinity_meq_per_l: preset.alkalinity_meq_per_l,
        calcium_mg_per_l: preset.calcium_mg_per_l,
        magnesium_mg_per_l: preset.magnesium_mg_per_l,
        sodium_mg_per_l: preset.sodium_mg_per_l,
        potassium_mg_per_l: preset.potassium_mg_per_l,
        bicarbonate_mg_per_l: preset.bicarbonate_mg_per_l,
        chloride_mg_per_l: preset.chloride_mg_per_l,
        sulfate_mg_per_l: preset.sulfate_mg_per_l,
    }
}

/// Run the engine for the given number of days with light feeding.
fn run_days(engine: &mut Engine, days: u32) -> Result<(), SimError> {
    for _ in 0..days {
        run_day(engine, 0.1)?;
    }
    Ok(())
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[test]
fn test_reproduction_suppressed_above_30c() -> Result<(), SimError> {
    // At 31°C, breeding rate should drop to near zero.
    // At 25°C (optimal), breeding is at baseline.
    let mut hot_state = breeding_fixture(SimSeed(8001));
    hot_state.water.temperature_c = 31.0;
    hot_state.environment.ambient_temp_c = 31.0;
    hot_state.stability_tracker.prev_temp_c = 31.0;
    // Heater off so temp stays at ambient
    hot_state.hardware.heater.enabled = false;

    let mut cool_state = breeding_fixture(SimSeed(8001));
    cool_state.water.temperature_c = 25.0;
    cool_state.environment.ambient_temp_c = 25.0;
    cool_state.stability_tracker.prev_temp_c = 25.0;
    cool_state.hardware.heater.enabled = false;

    let mut hot_engine = Engine::from_parts(hot_state, vec![]);
    let mut cool_engine = Engine::from_parts(cool_state, vec![]);

    run_days(&mut hot_engine, 45)?;
    run_days(&mut cool_engine, 45)?;

    let hot_readiness = hot_engine.full_state().animal.reproductive_readiness_index;
    let cool_readiness = cool_engine.full_state().animal.reproductive_readiness_index;

    assert!(
        hot_readiness < cool_readiness * 0.5,
        "31°C readiness ({hot_readiness:.4}) should be less than half of 25°C readiness ({cool_readiness:.4})"
    );

    // The hot tank should have significantly fewer juvenile/berried outcomes
    let hot_snap = hot_engine.snapshot();
    let cool_snap = cool_engine.snapshot();
    assert!(
        cool_snap.berried_females_count + cool_snap.juveniles_count
            > hot_snap.berried_females_count + hot_snap.juveniles_count,
        "Cool tank should have more reproductive output than hot tank. \
         Cool: {} berried + {} juv, Hot: {} berried + {} juv",
        cool_snap.berried_females_count,
        cool_snap.juveniles_count,
        hot_snap.berried_females_count,
        hot_snap.juveniles_count,
    );

    Ok(())
}

#[test]
fn test_reproduction_suppressed_by_instability() -> Result<(), SimError> {
    let stable_state = breeding_fixture_with_volume(SimSeed(8002), 10.0);
    let swing_state = breeding_fixture_with_volume(SimSeed(8002), 10.0);

    let mut stable_engine = Engine::from_parts(stable_state, vec![]);
    let mut swing_engine = Engine::from_parts(swing_state, vec![]);

    run_days(&mut stable_engine, 5)?;
    run_days(&mut swing_engine, 5)?;

    swing_engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 36.0 })?;
    run_day(&mut stable_engine, 0.1)?;
    run_day(&mut swing_engine, 0.1)?;

    let shock_state = swing_engine.full_state();
    let shock_swing = shock_state.stability_tracker.last_temp_swing_c;
    let shock_instability = shock_state.stability_tracker.instability_index;
    let shock_readiness = shock_state.animal.reproductive_readiness_index;
    let stable_shock_readiness = stable_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        shock_swing >= 3.0,
        "Expected a real >=3°C swing in 24h, got {shock_swing:.2}°C"
    );
    assert!(
        shock_instability > 0.2,
        "Instability should jump after the swing, got {shock_instability:.4}"
    );
    assert!(
        shock_readiness < stable_shock_readiness,
        "Readiness should drop on the swing day: swing={shock_readiness:.4}, stable={stable_shock_readiness:.4}"
    );

    let mut lingering_state = shock_state.clone();
    lingering_state.water.temperature_c = 24.0;
    lingering_state.environment.ambient_temp_c = 24.0;
    lingering_state.stability_tracker.prev_temp_c = 24.0;
    lingering_state.stability_tracker.prev_ph = lingering_state.water.ph;
    lingering_state.stability_tracker.prev_gh_d = lingering_state.gh_d();
    lingering_state.stability_tracker.prev_do_mg_l = lingering_state.do_mg_per_l();
    lingering_state.process_params.shrimp_condition_smoothing = 0.0;
    lingering_state.animal.adult.condition_index = 0.8;
    lingering_state.animal.molt_stress_index = 0.0;
    lingering_state.animal.adult.molt_timer_days = 0.0;
    lingering_state.animal.sub_adult.molt_timer_days = 0.0;
    lingering_state.animal.juvenile.molt_timer_days = 0.0;
    lingering_state.animal.hourly_heat_stress_accum = 0.0;
    lingering_state.animal.hourly_instability_stress_accum = 0.0;

    let mut recovered_state = lingering_state.clone();
    recovered_state.stability_tracker.instability_index = 0.0;

    step_daily_shrimp(&mut lingering_state);
    step_daily_shrimp(&mut recovered_state);

    assert!(
        lingering_state.animal.reproductive_readiness_index
            < recovered_state.animal.reproductive_readiness_index,
        "At matched temperature/condition, lingering instability alone should suppress readiness: unstable={:.4}, recovered={:.4}",
        lingering_state.animal.reproductive_readiness_index,
        recovered_state.animal.reproductive_readiness_index,
    );
    assert!(
        lingering_state.animal.adult.condition_index
            >= recovered_state.animal.adult.condition_index - 1.0e-9,
        "This regression should not rely on a worse post-shock condition state"
    );

    Ok(())
}

#[test]
fn test_reproduction_increases_with_condition() -> Result<(), SimError> {
    // Condition directly feeds into the reproductive readiness multiplier.
    // Freeze condition via zero smoothing and verify readiness tracks it.

    // High condition
    let mut high = breeding_fixture(SimSeed(8003));
    high.animal.adult.condition_index = 0.9;
    high.animal.reproductive_readiness_index = 0.5; // same starting readiness
    high.process_params.shrimp_condition_smoothing = 0.0; // freeze condition

    // Low condition
    let mut low = breeding_fixture(SimSeed(8003));
    low.animal.adult.condition_index = 0.2;
    low.animal.reproductive_readiness_index = 0.5; // same starting readiness
    low.process_params.shrimp_condition_smoothing = 0.0; // freeze condition

    let mut high_engine = Engine::from_parts(high, vec![]);
    let mut low_engine = Engine::from_parts(low, vec![]);

    run_days(&mut high_engine, 30)?;
    run_days(&mut low_engine, 30)?;

    let high_snap = high_engine.snapshot();
    let low_snap = low_engine.snapshot();

    // Condition should remain frozen at set values
    assert!(
        (high_snap.shrimp_condition_index - 0.9).abs() < 0.01,
        "High condition should stay frozen near 0.9, got {:.4}",
        high_snap.shrimp_condition_index,
    );
    assert!(
        (low_snap.shrimp_condition_index - 0.2).abs() < 0.01,
        "Low condition should stay frozen near 0.2, got {:.4}",
        low_snap.shrimp_condition_index,
    );

    // Higher condition should yield higher readiness
    assert!(
        high_snap.shrimp_reproductive_readiness > low_snap.shrimp_reproductive_readiness,
        "High-condition readiness ({:.4}) should exceed low-condition ({:.4})",
        high_snap.shrimp_reproductive_readiness,
        low_snap.shrimp_reproductive_readiness,
    );

    // The readiness difference should be substantial (multiplicative factor)
    assert!(
        high_snap.shrimp_reproductive_readiness > low_snap.shrimp_reproductive_readiness * 2.0,
        "High readiness ({:.4}) should be > 2x low readiness ({:.4})",
        high_snap.shrimp_reproductive_readiness,
        low_snap.shrimp_reproductive_readiness,
    );

    Ok(())
}

#[test]
fn test_density_reduces_per_capita_breeding() -> Result<(), SimError> {
    // At 2 shrimp/L → normal breeding rate.
    // At 15 shrimp/L → reduced per-capita breeding rate.
    // Assert monotonic decrease.
    let vol_l = {
        let state = breeding_fixture(SimSeed(8004));
        state.water_volume_l()
    };

    // Low density: ~2/L
    let low_count = (2.0 * vol_l).round() as u32;
    let mut low_state = breeding_fixture(SimSeed(8004));
    low_state.animal.adult.count = low_count;
    low_state.animal.adult.reserve_g = f64::from(low_count) * 0.5;

    // Medium density: ~8/L
    let med_count = (8.0 * vol_l).round() as u32;
    let mut med_state = breeding_fixture(SimSeed(8004));
    med_state.animal.adult.count = med_count;
    med_state.animal.adult.reserve_g = f64::from(med_count) * 0.5;

    // High density: ~15/L
    let high_count = (15.0 * vol_l).round() as u32;
    let mut high_state = breeding_fixture(SimSeed(8004));
    high_state.animal.adult.count = high_count;
    high_state.animal.adult.reserve_g = f64::from(high_count) * 0.5;

    let mut low_engine = Engine::from_parts(low_state, vec![]);
    let mut med_engine = Engine::from_parts(med_state, vec![]);
    let mut high_engine = Engine::from_parts(high_state, vec![]);

    run_days(&mut low_engine, 20)?;
    run_days(&mut med_engine, 20)?;
    run_days(&mut high_engine, 20)?;

    let low_readiness = low_engine.full_state().animal.reproductive_readiness_index;
    let med_readiness = med_engine.full_state().animal.reproductive_readiness_index;
    let high_readiness = high_engine.full_state().animal.reproductive_readiness_index;

    // Monotonic decrease: low ≥ medium ≥ high
    assert!(
        low_readiness >= med_readiness,
        "Low density readiness ({low_readiness:.4}) should be >= medium ({med_readiness:.4})"
    );
    assert!(
        med_readiness >= high_readiness,
        "Medium density readiness ({med_readiness:.4}) should be >= high ({high_readiness:.4})"
    );
    // High density should be meaningfully suppressed
    assert!(
        high_readiness < low_readiness * 0.8,
        "High density readiness ({high_readiness:.4}) should be < 80% of low ({low_readiness:.4})"
    );

    Ok(())
}

#[test]
fn test_chemistry_stress_suppresses_breeding() -> Result<(), SimError> {
    // High TAN (> 1 mg/L as N) suppresses breeding.
    let vol = {
        let s = breeding_fixture(SimSeed(8005));
        s.water_volume_l()
    };

    // TAN stress test
    let mut tan_state = breeding_fixture(SimSeed(8005));
    tan_state.water.ammonia_total_mg_n_total = 2.0 * vol; // 2 mg/L TAN
                                                          // Weaken nitrifiers so TAN persists
    tan_state.microbe.ammonia_oxidizer_biomass_g = 0.01;
    tan_state.microbe.comammox_biomass_g = 0.0;

    let mut tan_engine = Engine::from_parts(tan_state, vec![]);
    run_days(&mut tan_engine, 15)?;

    let tan_readiness = tan_engine.full_state().animal.reproductive_readiness_index;

    // NO2 stress test
    let mut no2_state = breeding_fixture(SimSeed(8005));
    no2_state.water.nitrite_mg_n_total = 1.5 * vol; // 1.5 mg/L NO2
                                                    // Weaken NOB so nitrite persists
    no2_state.microbe.nitrite_oxidizer_biomass_g = 0.01;

    let mut no2_engine = Engine::from_parts(no2_state, vec![]);
    run_days(&mut no2_engine, 15)?;

    let no2_readiness = no2_engine.full_state().animal.reproductive_readiness_index;

    // Clean baseline
    let mut clean_engine = Engine::from_parts(breeding_fixture(SimSeed(8005)), vec![]);
    run_days(&mut clean_engine, 15)?;

    let clean_readiness = clean_engine
        .full_state()
        .animal
        .reproductive_readiness_index;

    assert!(
        tan_readiness < clean_readiness * 0.7,
        "TAN-stressed readiness ({tan_readiness:.4}) should be < 70% of clean ({clean_readiness:.4})"
    );
    assert!(
        no2_readiness < clean_readiness * 0.7,
        "NO2-stressed readiness ({no2_readiness:.4}) should be < 70% of clean ({clean_readiness:.4})"
    );

    Ok(())
}

#[test]
fn test_egg_dropping_from_sudden_change() -> Result<(), SimError> {
    // A berried female in an unstable environment (high instability_index from
    // a recent temp/chemistry swing) should experience egg dropping.
    // The stability tracker runs before the shrimp pipeline, so a same-day
    // temperature shock should be preserved via `last_temp_swing_c` even while
    // the smoothed instability index is still below the broader threshold.

    // Setup: simulate a tank where today's water temperature jumped 4°C.
    // The smoothed instability index only rises to 0.3 on day one, so this
    // regression proves the direct swing parameter is wired into egg dropping.
    let mut state = breeding_fixture(SimSeed(8006));
    state.animal.berried_females_count = 10;
    state.animal.adult.count = 20;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 10,
        progress_days: 10.0,
    }];
    state.water.temperature_c = 28.0;
    state.environment.ambient_temp_c = 28.0;
    state.stability_tracker.prev_temp_c = 24.0;
    state.hardware.heater.enabled = false;

    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
    engine.step_hours(24)?;

    let events = &engine.full_state().event_log;
    let drop_events: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::EggDropping)
        .collect();

    let berried_after = engine.full_state().animal.berried_females_count;
    let egg_count: u32 = engine
        .full_state()
        .animal
        .egg_cohorts
        .iter()
        .map(|c| c.count)
        .sum();
    let tracker = &engine.full_state().stability_tracker;
    let threshold = engine
        .full_state()
        .shrimp_params
        .egg_drop_instability_threshold;

    assert!(
        tracker.last_temp_swing_c >= 3.9,
        "Daily stability tracking should preserve the direct temp shock, got {:.3}°C",
        tracker.last_temp_swing_c,
    );
    assert!(
        tracker.instability_index < threshold,
        "Day-one instability should stay below the generic threshold so this test exercises the direct temp-swing path: instability {:.3}, threshold {:.3}",
        tracker.instability_index,
        threshold,
    );

    // A 4°C swing against a 2°C threshold yields the capped drop probability
    // of 0.6. With 10 berried females, getting 0 drops is (1-0.6)^10 ≈ 0.0001.
    assert!(
        !drop_events.is_empty() || berried_after < 10 || egg_count < 10,
        "Sudden instability should cause egg dropping. \
         Drop events: {}, berried: {berried_after}/10, eggs: {egg_count}/10",
        drop_events.len()
    );
    assert!(
        drop_events
            .iter()
            .any(|event| event.summary.contains("temp swing")),
        "Egg-dropping diagnostics should report the direct temp swing trigger"
    );

    // Also verify that a stable tank does NOT drop eggs.
    let mut stable_state = breeding_fixture(SimSeed(8006));
    stable_state.animal.berried_females_count = 10;
    stable_state.animal.adult.count = 20;
    stable_state.animal.egg_cohorts = vec![EggCohort {
        count: 10,
        progress_days: 10.0,
    }];
    stable_state.stability_tracker.instability_index = 0.0;

    let mut stable_engine = Engine::from_parts(stable_state, vec![]);
    stable_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
    stable_engine.step_hours(24)?;

    let stable_events = &stable_engine.full_state().event_log;
    let stable_drops = stable_events
        .iter()
        .filter(|e| e.kind == EventKind::EggDropping)
        .count();

    assert_eq!(
        stable_drops, 0,
        "Stable tank should not have egg dropping events"
    );

    Ok(())
}

#[test]
fn test_high_density_berried_events_report_density_cause() -> Result<(), SimError> {
    let mut state = breeding_fixture(SimSeed(8010));
    state.animal.adult.count = 20;
    state.animal.adult.reserve_g = 10.0;
    state.animal.adult.condition_index = 0.9;
    state.animal.reproductive_readiness_index = 1.0;
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.shrimp_params.base_spawn_rate = 0.3;
    state.shrimp_params.density_repro_threshold_per_l = 0.1;
    state.shrimp_params.density_repro_half_suppression_per_l = 0.2;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
    engine.step_hours(24)?;

    let berried_events: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpBerried)
        .collect();

    assert!(
        !berried_events.is_empty(),
        "High-density test should still produce at least one berried event"
    );
    assert!(
        berried_events
            .iter()
            .any(|event| event.cause_codes.contains(&EventCause::HighDensity)),
        "High-density berried events should surface the HighDensity cause code"
    );
    assert!(
        berried_events
            .iter()
            .any(|event| event.summary.contains("density")),
        "High-density berried events should include density diagnostics in the summary"
    );

    Ok(())
}

#[test]
fn test_density_suppression_applies_immediately_to_spawning() -> Result<(), SimError> {
    let mut low_density = breeding_fixture_with_volume(SimSeed(8017), 10.0);
    low_density.animal.adult.count = 10;
    low_density.animal.adult.reserve_g = 5.0;
    low_density.animal.adult.condition_index = 1.0;
    low_density.animal.molt_stress_index = 0.0;
    low_density.animal.reproductive_readiness_index = 1.0;
    low_density.process_params.shrimp_condition_smoothing = 0.0;
    low_density.shrimp_params.base_spawn_rate = 0.4;
    low_density.shrimp_params.density_repro_threshold_per_l = 2.0;
    low_density
        .shrimp_params
        .density_repro_half_suppression_per_l = 4.0;
    low_density.algae.set_periphyton_total(400.0);

    let mut high_density = low_density.clone();
    high_density.animal.adult.count = 150;
    high_density.animal.adult.reserve_g = 75.0;

    let low_eligible = ((0.5 * f64::from(low_density.animal.adult.count))
        - f64::from(low_density.animal.berried_females_count))
    .max(0.0);
    let high_eligible = ((0.5 * f64::from(high_density.animal.adult.count))
        - f64::from(high_density.animal.berried_females_count))
    .max(0.0);

    let mut low_engine = Engine::from_parts(low_density, vec![]);
    let mut high_engine = Engine::from_parts(high_density, vec![]);

    run_day(&mut low_engine, 0.1)?;
    run_day(&mut high_engine, 0.1)?;

    let low_spawn_per_capita =
        f64::from(low_engine.full_state().animal.berried_females_count) / low_eligible.max(1.0);
    let high_spawn_per_capita =
        f64::from(high_engine.full_state().animal.berried_females_count) / high_eligible.max(1.0);
    let high_readiness = high_engine.full_state().animal.reproductive_readiness_index;

    assert!(
        high_readiness > 0.85,
        "Crowding regression should observe immediate spawn suppression before the smoothed readiness index fully adapts, got readiness={high_readiness:.4}"
    );
    assert!(
        high_spawn_per_capita < low_spawn_per_capita * 0.7,
        "High density should cap same-day per-capita spawning. low={low_spawn_per_capita:.4}, high={high_spawn_per_capita:.4}"
    );

    Ok(())
}

#[test]
fn test_multiple_stressors_compound() -> Result<(), SimError> {
    // Two moderate stressors produce greater suppression than either alone.

    // Stressor A only: moderately elevated temperature (28°C)
    let mut temp_state = breeding_fixture(SimSeed(8007));
    temp_state.water.temperature_c = 28.0;
    temp_state.environment.ambient_temp_c = 28.0;
    temp_state.stability_tracker.prev_temp_c = 28.0;
    temp_state.hardware.heater.enabled = false;

    // Stressor B only: moderate instability
    let mut instab_state = breeding_fixture(SimSeed(8007));
    instab_state.stability_tracker.instability_index = 0.4;

    // Both stressors combined
    let mut both_state = breeding_fixture(SimSeed(8007));
    both_state.water.temperature_c = 28.0;
    both_state.environment.ambient_temp_c = 28.0;
    both_state.stability_tracker.prev_temp_c = 28.0;
    both_state.hardware.heater.enabled = false;
    both_state.stability_tracker.instability_index = 0.4;

    let mut temp_engine = Engine::from_parts(temp_state, vec![]);
    let mut instab_engine = Engine::from_parts(instab_state, vec![]);
    let mut both_engine = Engine::from_parts(both_state, vec![]);

    run_days(&mut temp_engine, 20)?;
    run_days(&mut instab_engine, 20)?;
    run_days(&mut both_engine, 20)?;

    let temp_readiness = temp_engine.full_state().animal.reproductive_readiness_index;
    let instab_readiness = instab_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let both_readiness = both_engine.full_state().animal.reproductive_readiness_index;

    // Combined should be worse than either alone
    assert!(
        both_readiness < temp_readiness,
        "Combined stressors ({both_readiness:.4}) should produce lower readiness \
         than temperature alone ({temp_readiness:.4})"
    );
    assert!(
        both_readiness < instab_readiness,
        "Combined stressors ({both_readiness:.4}) should produce lower readiness \
         than instability alone ({instab_readiness:.4})"
    );

    Ok(())
}

#[test]
fn test_all_reproduction_factors_are_named_parameters() -> Result<(), SimError> {
    // Temperature curve knobs should change readiness under the same warm state.
    let mut temp_default = breeding_fixture(SimSeed(8008));
    temp_default.water.temperature_c = 29.0;
    temp_default.environment.ambient_temp_c = 29.0;
    temp_default.stability_tracker.prev_temp_c = 29.0;
    temp_default.hardware.heater.enabled = false;
    temp_default.process_params.shrimp_condition_smoothing = 0.0;

    let mut temp_relaxed = temp_default.clone();
    temp_relaxed.shrimp_params.high_temp_repro_penalty_start_c = 31.0;
    temp_relaxed.shrimp_params.high_temp_repro_penalty_full_c = 35.0;

    let mut temp_default_engine = Engine::from_parts(temp_default, vec![]);
    let mut temp_relaxed_engine = Engine::from_parts(temp_relaxed, vec![]);
    run_days(&mut temp_default_engine, 12)?;
    run_days(&mut temp_relaxed_engine, 12)?;
    let temp_default_readiness = temp_default_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let temp_relaxed_readiness = temp_relaxed_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        temp_relaxed_readiness > temp_default_readiness,
        "Temperature penalty parameters should change reproduction readiness: relaxed={temp_relaxed_readiness:.4}, default={temp_default_readiness:.4}"
    );

    // Density threshold knobs should change readiness under the same crowding.
    let mut density_strict = breeding_fixture_with_volume(SimSeed(8009), 10.0);
    density_strict.animal.adult.count = 120;
    density_strict.animal.adult.reserve_g = 60.0;
    density_strict.animal.adult.condition_index = 1.0;
    density_strict.animal.reproductive_readiness_index = 0.8;
    density_strict.animal.molt_stress_index = 0.0;
    density_strict.process_params.shrimp_condition_smoothing = 0.0;
    density_strict.shrimp_params.density_repro_threshold_per_l = 4.0;
    density_strict
        .shrimp_params
        .density_repro_half_suppression_per_l = 6.0;
    density_strict.algae.set_periphyton_total(400.0);

    let mut density_relaxed = density_strict.clone();
    density_relaxed.shrimp_params.density_repro_threshold_per_l = 20.0;
    density_relaxed
        .shrimp_params
        .density_repro_half_suppression_per_l = 30.0;

    let mut density_strict_engine = Engine::from_parts(density_strict, vec![]);
    let mut density_relaxed_engine = Engine::from_parts(density_relaxed, vec![]);
    run_days(&mut density_strict_engine, 10)?;
    run_days(&mut density_relaxed_engine, 10)?;
    let density_strict_readiness = density_strict_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let density_relaxed_readiness = density_relaxed_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        density_relaxed_readiness > density_strict_readiness,
        "Density thresholds should change readiness under the same crowding: relaxed={density_relaxed_readiness:.4}, strict={density_strict_readiness:.4}"
    );

    // TAN and NO2 thresholds should change the same chemistry state.
    let mut tan_strict = breeding_fixture(SimSeed(8014));
    tan_strict.water.ammonia_total_mg_n_total = 1.2 * tan_strict.water_volume_l();
    tan_strict.process_params.shrimp_condition_smoothing = 0.0;
    tan_strict.microbe.ammonia_oxidizer_biomass_g = 0.0;
    tan_strict.microbe.nitrite_oxidizer_biomass_g = 0.0;
    tan_strict.microbe.comammox_biomass_g = 0.0;
    tan_strict.filter_state.biofilter_maturity_index = 0.0;
    tan_strict.shrimp_params.tan_repro_threshold_mg_n_per_l = 0.5;
    let mut tan_relaxed = tan_strict.clone();
    tan_relaxed.shrimp_params.tan_repro_threshold_mg_n_per_l = 2.0;

    let mut tan_strict_engine = Engine::from_parts(tan_strict, vec![]);
    let mut tan_relaxed_engine = Engine::from_parts(tan_relaxed, vec![]);
    run_days(&mut tan_strict_engine, 5)?;
    run_days(&mut tan_relaxed_engine, 5)?;
    let tan_strict_readiness = tan_strict_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let tan_relaxed_readiness = tan_relaxed_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        tan_relaxed_readiness > tan_strict_readiness,
        "TAN threshold should change readiness: relaxed={tan_relaxed_readiness:.4}, strict={tan_strict_readiness:.4}"
    );

    let mut no2_strict = breeding_fixture(SimSeed(8015));
    no2_strict.water.nitrite_mg_n_total = 0.8 * no2_strict.water_volume_l();
    no2_strict.process_params.shrimp_condition_smoothing = 0.0;
    no2_strict.microbe.ammonia_oxidizer_biomass_g = 0.0;
    no2_strict.microbe.nitrite_oxidizer_biomass_g = 0.0;
    no2_strict.microbe.comammox_biomass_g = 0.0;
    no2_strict.filter_state.biofilter_maturity_index = 0.0;
    no2_strict.shrimp_params.no2_repro_threshold_mg_n_per_l = 0.2;
    let mut no2_relaxed = no2_strict.clone();
    no2_relaxed.shrimp_params.no2_repro_threshold_mg_n_per_l = 1.0;

    let mut no2_strict_engine = Engine::from_parts(no2_strict, vec![]);
    let mut no2_relaxed_engine = Engine::from_parts(no2_relaxed, vec![]);
    run_days(&mut no2_strict_engine, 5)?;
    run_days(&mut no2_relaxed_engine, 5)?;
    let no2_strict_readiness = no2_strict_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let no2_relaxed_readiness = no2_relaxed_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        no2_relaxed_readiness > no2_strict_readiness,
        "NO2 threshold should change readiness: relaxed={no2_relaxed_readiness:.4}, strict={no2_strict_readiness:.4}"
    );

    // GH/mineral thresholds should change the same low-GH state.
    let mut mineral_strict = breeding_fixture(SimSeed(8017));
    mineral_strict.process_params.shrimp_condition_smoothing = 0.0;
    mineral_strict.animal.adult.condition_index = 1.0;
    mineral_strict.animal.reproductive_readiness_index = 0.5;
    mineral_strict.animal.molt_stress_index = 0.0;
    mineral_strict.water.calcium_mg_total = 10.0 * mineral_strict.water_volume_l();
    mineral_strict.water.magnesium_mg_total = 2.0 * mineral_strict.water_volume_l();
    mineral_strict.shrimp_params.gh_min_d = 5.0;
    mineral_strict.shrimp_params.gh_max_d = 10.0;
    let mut mineral_relaxed = mineral_strict.clone();
    mineral_relaxed.shrimp_params.gh_min_d = 1.0;
    mineral_relaxed.shrimp_params.gh_max_d = 12.0;

    let mut mineral_strict_engine = Engine::from_parts(mineral_strict, vec![]);
    let mut mineral_relaxed_engine = Engine::from_parts(mineral_relaxed, vec![]);
    run_days(&mut mineral_strict_engine, 8)?;
    run_days(&mut mineral_relaxed_engine, 8)?;
    let mineral_strict_readiness = mineral_strict_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    let mineral_relaxed_readiness = mineral_relaxed_engine
        .full_state()
        .animal
        .reproductive_readiness_index;
    assert!(
        mineral_relaxed_readiness > mineral_strict_readiness,
        "GH/mineral thresholds should change readiness: relaxed={mineral_relaxed_readiness:.4}, strict={mineral_strict_readiness:.4}"
    );

    // Instability tuning should change both the smoothed instability response
    // and the egg-drop outcome for the same real thermal shock.
    let mut swing_sensitive = breeding_fixture_with_volume(SimSeed(8016), 10.0);
    swing_sensitive.animal.adult.count = 80;
    swing_sensitive.animal.berried_females_count = 40;
    swing_sensitive.animal.egg_cohorts = vec![EggCohort {
        count: 40,
        progress_days: 10.0,
    }];
    swing_sensitive.shrimp_params.instability_temp_swing_c = 2.0;
    swing_sensitive.shrimp_params.egg_drop_temp_swing_c = 2.0;
    swing_sensitive.shrimp_params.instability_ph_swing = 100.0;
    swing_sensitive.shrimp_params.instability_gh_swing_d = 100.0;
    swing_sensitive.shrimp_params.instability_do_swing_mg_l = 100.0;

    let mut swing_tolerant = swing_sensitive.clone();
    swing_tolerant.shrimp_params.instability_temp_swing_c = 6.0;
    swing_tolerant.shrimp_params.egg_drop_temp_swing_c = 6.0;

    let mut swing_sensitive_engine = Engine::from_parts(swing_sensitive, vec![]);
    let mut swing_tolerant_engine = Engine::from_parts(swing_tolerant, vec![]);
    swing_sensitive_engine
        .apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 28.0 })?;
    swing_tolerant_engine
        .apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 28.0 })?;
    run_day(&mut swing_sensitive_engine, 0.1)?;
    run_day(&mut swing_tolerant_engine, 0.1)?;

    let sensitive_state = swing_sensitive_engine.full_state();
    let tolerant_state = swing_tolerant_engine.full_state();
    assert!(
        sensitive_state.stability_tracker.last_temp_swing_c >= 3.0
            && tolerant_state.stability_tracker.last_temp_swing_c >= 3.0,
        "Expected both scenarios to see the same >=3°C swing, got sensitive={:.2}°C tolerant={:.2}°C",
        sensitive_state.stability_tracker.last_temp_swing_c,
        tolerant_state.stability_tracker.last_temp_swing_c,
    );
    assert!(
        sensitive_state.stability_tracker.instability_index
            > tolerant_state.stability_tracker.instability_index,
        "Instability swing tuning should change the tracker response: sensitive={:.4}, tolerant={:.4}",
        sensitive_state.stability_tracker.instability_index,
        tolerant_state.stability_tracker.instability_index,
    );
    assert!(
        sensitive_state.animal.berried_females_count < tolerant_state.animal.berried_females_count,
        "Egg-drop swing tuning should change clutch loss: sensitive={} tolerant={}",
        sensitive_state.animal.berried_females_count,
        tolerant_state.animal.berried_females_count,
    );

    Ok(())
}

// ── Integration tests ──────────────────────────────────────────────────────

/// Well-maintained tank breeds steadily over 2000 hours with at least 3
/// reproductive cycles observed.
#[test]
fn test_mature_stable_tank_breeds_well() -> Result<(), SimError> {
    let mut state = breeding_fixture(SimSeed(9001));
    // Zero mortality to isolate reproduction testing
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    // Good initial population
    state.animal.adult.count = 20;
    state.animal.adult.reserve_g = 10.0;
    state.animal.adult.condition_index = 0.85;
    state.animal.reproductive_readiness_index = 0.85;
    // Fast reproduction timing
    state.shrimp_params.base_spawn_rate = 0.15;
    state.shrimp_params.egg_duration_days = 14;
    state.shrimp_params.hatch_success_base = 0.9;
    state.shrimp_params.apply_legacy_total_maturation_days(40.0);
    state.animal.molt_stress_index = 0.0;
    state.animal.last_molt_success = true;
    // Very strong nitrifiers to handle nitrogen load from feeding + shrimp metabolism
    state.microbe.ammonia_oxidizer_biomass_g = 10.0;
    state.microbe.nitrite_oxidizer_biomass_g = 10.0;
    state.microbe.comammox_biomass_g = 2.0;
    // Higher nitrification capacity
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 30.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 30.0;
    // Abundant periphyton for natural grazing
    state.algae.set_periphyton_total(120.0);
    state.source_water_catalog.insert(
        "hard_shrimp".to_string(),
        load_source_profile("hard_shrimp"),
    );
    let initial_total = state.animal.total_count();

    let mut engine = Engine::from_parts(state, vec![]);
    let mut max_tan = 0.0_f64;
    let mut max_no2 = 0.0_f64;
    let mut max_instability = 0.0_f64;

    // Run for 2000 simulated hours (~83 days) with very light feeding
    // (shrimp primarily graze periphyton; weekly water changes keep the
    // "mature, stable tank" chemistry and minerals in range)
    let total_hours = 2000u32;
    let days = total_hours / 24;
    for day in 0..days {
        let peaks = run_day_with_hourly_peaks(&mut engine, 0.05)?;
        if (day + 1) % 5 == 0 {
            engine.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
        }
        max_tan = max_tan.max(peaks.tan_mg_n_per_l);
        max_no2 = max_no2.max(peaks.nitrite_mg_n_per_l);
        max_instability = max_instability.max(peaks.instability_index);
    }
    let remaining = total_hours % 24;
    if remaining > 0 {
        engine.step_hours(remaining)?;
    }

    let events = &engine.full_state().event_log;
    let berried_events = events
        .iter()
        .filter(|e| e.kind == EventKind::ShrimpBerried)
        .count();

    // At least 3 reproductive cycles observed (ShrimpBerried events)
    assert!(
        berried_events >= 3,
        "Stable tank should show at least 3 reproductive cycles in 2000 hours. Got {berried_events}"
    );

    let snap = engine.snapshot();
    // Population should grow while chemistry stays controlled.
    assert!(
        snap.total_shrimp_count > initial_total,
        "Stable tank should grow beyond the starting population. Initial: {initial_total}, final: {}",
        snap.total_shrimp_count,
    );
    assert!(
        max_tan < 0.6 && max_no2 < 0.6,
        "Stable tank should keep chemistry in range. peak TAN={max_tan:.3}, peak NO2={max_no2:.3}"
    );
    assert!(
        max_instability < 0.35,
        "Stable tank should avoid chronic instability. peak instability={max_instability:.3}"
    );

    Ok(())
}

/// Neglected tank: same initial conditions but no water changes, overfeeding.
/// Chemistry degrades, breeding stalls, population plateaus or declines.
#[test]
fn test_neglected_tank_breeding_stalls() -> Result<(), SimError> {
    let mut baseline_state = breeding_fixture(SimSeed(9002));
    baseline_state.process_params.shrimp_base_mortality_per_day = 0.002;
    baseline_state.process_params.shrimp_stress_mortality_scale = 0.15;
    baseline_state.animal.adult.count = 15;
    baseline_state.animal.adult.reserve_g = 8.0;
    baseline_state.animal.adult.condition_index = 0.8;
    baseline_state.shrimp_params.base_spawn_rate = 0.12;
    baseline_state.shrimp_params.egg_duration_days = 21;
    baseline_state.shrimp_params.hatch_success_base = 0.7;
    baseline_state
        .shrimp_params
        .apply_legacy_total_maturation_days(60.0);
    baseline_state.microbe.ammonia_oxidizer_biomass_g = 8.0;
    baseline_state.microbe.nitrite_oxidizer_biomass_g = 8.0;
    baseline_state.microbe.comammox_biomass_g = 1.5;
    baseline_state.process_params.aob_vmax_mg_n_per_g_per_hour = 25.0;
    baseline_state.process_params.nob_vmax_mg_n_per_g_per_hour = 25.0;
    baseline_state.algae.set_periphyton_total(120.0);
    let initial_total = baseline_state.animal.total_count();

    let neglected_state = baseline_state.clone();
    let mut stable_state = baseline_state;
    stable_state.source_water_catalog.insert(
        "hard_shrimp".to_string(),
        load_source_profile("hard_shrimp"),
    );

    let mut neglected_engine = Engine::from_parts(neglected_state, vec![]);
    let mut stable_engine = Engine::from_parts(stable_state, vec![]);

    let total_hours = 2000u32;
    let days = total_hours / 24;
    let mut neglected_peak_tan = 0.0_f64;
    let mut neglected_peak_no2 = 0.0_f64;
    let mut neglected_peak_instability = 0.0_f64;
    let mut neglected_peak_population = initial_total;

    for day in 0..days {
        run_day(&mut neglected_engine, 0.75)?;
        let neglected_snapshot = neglected_engine.snapshot();
        neglected_peak_tan = neglected_peak_tan.max(neglected_snapshot.tan_mg_n_per_l);
        neglected_peak_no2 = neglected_peak_no2.max(neglected_snapshot.nitrite_mg_n_per_l);
        neglected_peak_instability = neglected_peak_instability.max(
            neglected_engine
                .full_state()
                .stability_tracker
                .instability_index,
        );
        neglected_peak_population =
            neglected_peak_population.max(neglected_snapshot.total_shrimp_count);

        run_day(&mut stable_engine, 0.05)?;
        if (day + 1) % 5 == 0 {
            stable_engine.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
        }
    }
    let remaining = total_hours % 24;
    if remaining > 0 {
        neglected_engine.step_hours(remaining)?;
        stable_engine.step_hours(remaining)?;
    }

    let neglected_snap = neglected_engine.snapshot();
    let stable_snap = stable_engine.snapshot();
    let neglected_berried_events = neglected_engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpBerried)
        .count();
    let stable_berried_events = stable_engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpBerried)
        .count();

    assert!(
        (neglected_peak_tan > 0.5 || neglected_peak_no2 > 0.2) && neglected_peak_instability > 0.15,
        "Neglected tank should show chemistry and stability degradation. \
         peak TAN={neglected_peak_tan:.3}, peak NO2={neglected_peak_no2:.3}, \
         peak instability={neglected_peak_instability:.3}"
    );
    assert!(
        neglected_snap.total_shrimp_count <= initial_total + 5
            || neglected_snap.total_shrimp_count + 3 <= neglected_peak_population,
        "Neglected tank should stall or reverse population growth. \
         initial={initial_total}, peak={}, final={}",
        neglected_peak_population,
        neglected_snap.total_shrimp_count,
    );
    assert!(
        neglected_snap.total_shrimp_count + 5 < stable_snap.total_shrimp_count
            && neglected_berried_events < stable_berried_events,
        "Neglected tank should underperform the maintained control. \
         neglected total={} stable total={}, neglected berried={} stable berried={}",
        neglected_snap.total_shrimp_count,
        stable_snap.total_shrimp_count,
        neglected_berried_events,
        stable_berried_events,
    );
    assert!(
        neglected_snap.repro_dominant_suppression != "none",
        "Neglected tank should report a dominant suppression factor"
    );

    Ok(())
}

/// Verify snapshot reports the dominant suppression factor.
#[test]
fn test_snapshot_reports_dominant_suppression() -> Result<(), SimError> {
    // Create a tank where temperature is the dominant suppressor
    let mut state = breeding_fixture(SimSeed(8010));
    state.water.temperature_c = 32.0;
    state.environment.ambient_temp_c = 32.0;
    state.stability_tracker.prev_temp_c = 32.0;
    state.hardware.heater.enabled = false;

    let mut engine = Engine::from_parts(state, vec![]);
    run_days(&mut engine, 5)?;

    let snap = engine.snapshot();
    assert_eq!(
        snap.repro_dominant_suppression, "temperature",
        "At 32°C, dominant suppression should be temperature, got: {}",
        snap.repro_dominant_suppression
    );

    Ok(())
}

#[test]
fn test_snapshot_uses_runtime_suppression_labels() {
    let mut molt_state = breeding_fixture(SimSeed(8011));
    molt_state.animal.adult.condition_index = 1.0;
    molt_state.animal.molt_stress_index = 0.9;
    let molt_snap = Engine::from_parts(molt_state, vec![]).snapshot();
    assert_eq!(
        molt_snap.repro_dominant_suppression, "molt_stress",
        "snapshot should surface the runtime molt-stress suppression factor"
    );

    let mut tan_state = breeding_fixture(SimSeed(8012));
    tan_state.animal.adult.condition_index = 1.0;
    tan_state.animal.molt_stress_index = 0.0;
    tan_state.water.ammonia_total_mg_n_total = 2.0 * tan_state.water_volume_l();
    let tan_snap = Engine::from_parts(tan_state, vec![]).snapshot();
    assert_eq!(
        tan_snap.repro_dominant_suppression, "tan",
        "snapshot should distinguish TAN suppression from other chemistry factors"
    );

    let mut nitrite_state = breeding_fixture(SimSeed(8013));
    nitrite_state.animal.adult.condition_index = 1.0;
    nitrite_state.animal.molt_stress_index = 0.0;
    nitrite_state.water.nitrite_mg_n_total = 1.0 * nitrite_state.water_volume_l();
    let nitrite_snap = Engine::from_parts(nitrite_state, vec![]).snapshot();
    assert_eq!(
        nitrite_snap.repro_dominant_suppression, "nitrite",
        "snapshot should distinguish nitrite suppression from TAN"
    );

    let mut mineral_state = breeding_fixture(SimSeed(8017));
    mineral_state.animal.adult.condition_index = 1.0;
    mineral_state.animal.molt_stress_index = 0.0;
    mineral_state.water.calcium_mg_total = 10.0 * mineral_state.water_volume_l();
    mineral_state.water.magnesium_mg_total = 2.0 * mineral_state.water_volume_l();
    let mineral_snap = Engine::from_parts(mineral_state, vec![]).snapshot();
    assert_eq!(
        mineral_snap.repro_dominant_suppression, "minerals",
        "snapshot should surface GH/mineral suppression when low GH is the dominant limiter"
    );
}
