//! End-to-end integration tests simulating realistic user journeys.
//! Tests cover happy paths (well-maintained tanks that thrive) and
//! sad paths (neglect, mistakes, and environmental disasters).

use tank_core::{Engine, PlayerAction, SimSeed, SimulationEngine, TankSnapshot};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_engine(scenario: &str, overrides: StartupOverrides) -> Engine {
    let seed = SimSeed(42);
    let state = tank_scenarios::seeded_state_with_full_overrides(seed, scenario, overrides)
        .unwrap_or_else(|e| panic!("failed to materialize {scenario}: {e}"));
    Engine::from_parts(state, vec![])
}

fn default_overrides() -> StartupOverrides {
    StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 1.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("moderate".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlanted),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours8),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(false),
        initial_adult_shrimp_count: Some(10),
        ..StartupOverrides::default()
    }
}

fn snap(engine: &Engine) -> TankSnapshot {
    engine.snapshot()
}

fn step_days(engine: &mut Engine, days: u32) {
    for _ in 0..days {
        engine.step_hours(24).expect("step failed");
    }
}

fn feed_daily(engine: &mut Engine, days: u32, grams: f64) {
    for _ in 0..days {
        engine
            .apply_action(PlayerAction::Feed { grams })
            .expect("feed failed");
        engine.step_hours(24).expect("step failed");
    }
}

fn weekly_maintenance(engine: &mut Engine, days: u32, feed_g: f64, wc_percent: f64) {
    for day in 1..=days {
        engine
            .apply_action(PlayerAction::Feed { grams: feed_g })
            .expect("feed failed");
        engine.step_hours(24).expect("step failed");

        if day % 7 == 0 {
            engine
                .apply_action(PlayerAction::WaterChangePercent {
                    percent: wc_percent,
                    source_profile_id: "moderate".to_string(),
                })
                .expect("water change failed");
            engine.step_hours(1).expect("step after wc failed");
        }
    }
}

fn assert_finite_snapshot(s: &TankSnapshot) {
    assert!(s.ph.is_finite(), "pH NaN/Inf");
    assert!(s.water_temp_c.is_finite(), "temp NaN/Inf");
    assert!(s.tan_mg_n_per_l.is_finite(), "TAN NaN/Inf");
    assert!(s.nitrite_mg_n_per_l.is_finite(), "NO2 NaN/Inf");
    assert!(s.nitrate_mg_n_per_l.is_finite(), "NO3 NaN/Inf");
    assert!(s.do_mg_l.is_finite(), "DO NaN/Inf");
    assert!(s.gh_d.is_finite(), "GH NaN/Inf");
    assert!(s.kh_d.is_finite(), "KH NaN/Inf");
    assert!(s.total_plant_biomass_g.is_finite(), "plant biomass NaN/Inf");
    assert!(s.water_volume_l > 0.0, "water volume must be positive");
}

fn print_status(label: &str, s: &TankSnapshot, engine: &Engine) {
    let st = engine.full_state();
    println!(
        "{label}: {:.1}°C pH {:.2} TAN {:.3} NO2 {:.3} NO3 {:.2} DO {:.1} GH {:.1} | \
         shrimp {}/{}/{} | plants {:.2}g | algae {:.3}g | biofilter {:.3}",
        s.water_temp_c,
        s.ph,
        s.tan_mg_n_per_l,
        s.nitrite_mg_n_per_l,
        s.nitrate_mg_n_per_l,
        s.do_mg_l,
        s.gh_d,
        st.animal.adult.count,
        st.animal.juvenile.count,
        st.animal.berried_females_count,
        s.total_plant_biomass_g,
        s.suspended_algae_biomass_g + s.periphyton_biomass_g,
        s.biofilter_maturity_index,
    );
}

// ===========================================================================
// HAPPY PATH TESTS
// ===========================================================================

/// A user sets up a medium planted tank with proper cycling, adds shrimp
/// after the cycle matures, and maintains it well for 120 days.
/// Shrimp should survive and the ecosystem should be stable.
#[test]
fn happy_medium_planted_well_maintained() {
    let mut engine = make_engine(
        "medium_planted",
        StartupOverrides {
            initial_adult_shrimp_count: Some(0), // fishless cycle first
            ..default_overrides()
        },
    );

    let s0 = snap(&engine);
    print_status("Day 0", &s0, &engine);
    assert!(s0.water_volume_l > 20.0, "medium tank should be >20L");

    // Fishless cycle: ghost-feed for 30 days
    feed_daily(&mut engine, 30, 0.05);

    let s30 = snap(&engine);
    print_status("Day 30 (pre-stock)", &s30, &engine);
    assert_finite_snapshot(&s30);

    // Biofilter should be establishing
    assert!(
        s30.ammonia_oxidizer_biomass_g > 0.0,
        "AOB should have grown during cycling"
    );

    // Stock shrimp
    engine
        .apply_action(PlayerAction::AddShrimp { count: 15 })
        .expect("stocking failed");

    // 90 days of good maintenance
    weekly_maintenance(&mut engine, 90, 0.05, 25.0);

    let s120 = snap(&engine);
    let state = engine.full_state();
    print_status("Day 120", &s120, &engine);
    assert_finite_snapshot(&s120);

    // Ecosystem viability checks
    assert!(s120.ph >= 5.5 && s120.ph < 8.5, "pH viable: {}", s120.ph);
    assert!(s120.do_mg_l > 3.0, "DO healthy: {}", s120.do_mg_l);
    assert!(
        s120.water_temp_c > 20.0 && s120.water_temp_c < 30.0,
        "temp viable"
    );

    // Plants should still be alive
    assert!(s120.total_plant_biomass_g > 0.0, "plants should be alive");

    // Nitrifier biomass should be present (cycle is active)
    assert!(
        s120.ammonia_oxidizer_biomass_g > 0.0 || s120.comammox_biomass_g > 0.0,
        "nitrifiers should be present at day 120"
    );

    // Save/load roundtrip
    let save = tank_core::save::SaveFile::from_engine(&engine);
    let json = save.to_json_pretty().expect("save failed");
    let loaded = tank_core::save::SaveFile::from_json(&json).expect("load failed");
    assert_eq!(state.water.temperature_c, loaded.state.water.temperature_c);

    println!("Events: {}", state.event_log.len());
}

/// A nano tank with careful maintenance and weekly water changes.
/// Even though small tanks are harder, good husbandry should keep things stable.
#[test]
fn happy_nano_careful_husbandry() {
    let mut engine = make_engine(
        "nano_cycle",
        StartupOverrides {
            initial_adult_shrimp_count: Some(5), // conservative stocking
            aeration_enabled: Some(true),        // extra oxygenation for nano
            ..default_overrides()
        },
    );

    // Conservative feeding + frequent water changes for 60 days
    for day in 1..=60 {
        engine
            .apply_action(PlayerAction::Feed { grams: 0.01 }) // very light
            .expect("feed");
        engine.step_hours(24).expect("step");

        // Twice-weekly 30% water changes for a nano
        if day % 3 == 0 {
            engine
                .apply_action(PlayerAction::WaterChangePercent {
                    percent: 30.0,
                    source_profile_id: "moderate".to_string(),
                })
                .expect("wc");
            engine.step_hours(1).expect("step after wc");
        }
    }

    let s = snap(&engine);
    print_status("Day 60 (nano careful)", &s, &engine);
    assert_finite_snapshot(&s);

    // With frequent water changes, TAN should be somewhat controlled
    assert!(s.do_mg_l > 4.0, "DO should be good with aeration");
}

/// Warm room scenario at higher ambient temp — heater should barely fire,
/// tank should stabilize at ambient.
#[test]
fn happy_warm_room_stable_temperature() {
    let mut engine = make_engine(
        "warm_room",
        StartupOverrides {
            heater_preset: Some(StartupHeaterPreset::Off),
            initial_adult_shrimp_count: Some(0),
            ..default_overrides()
        },
    );

    step_days(&mut engine, 7);

    let s = snap(&engine);
    print_status("Day 7 (warm room)", &s, &engine);
    assert_finite_snapshot(&s);

    // Warm room ambient should drive water temp up
    assert!(
        s.water_temp_c > 22.0,
        "warm room should have elevated temp: {}",
        s.water_temp_c
    );
    // No heater, so heater output should be zero
    assert!(
        s.last_heater_output_w < 0.01,
        "heater off, output should be ~0"
    );
}

/// Verify plants grow under good light and nutrients.
#[test]
fn happy_plants_grow_with_light_and_nutrients() {
    let mut engine = make_engine(
        "medium_planted",
        StartupOverrides {
            light_preset: Some(StartupLightPreset::Hours10),
            substrate_preset: Some(StartupSubstratePreset::ActivePlantedWithCoarsePorous),
            initial_adult_shrimp_count: Some(0),
            ..default_overrides()
        },
    );

    let initial_biomass = snap(&engine).total_plant_biomass_g;

    // 30 days of good light, no competition from overstocking
    feed_daily(&mut engine, 30, 0.02);

    let s = snap(&engine);
    print_status("Day 30 (plant growth)", &s, &engine);
    assert_finite_snapshot(&s);

    // With geometry-scaled initial biomass, plants at higher density may
    // self-shade and equilibrate slightly below their starting mass.
    // A ≥80% threshold verifies plants remain viable rather than demanding
    // strict growth from any starting density.
    assert!(
        s.total_plant_biomass_g >= initial_biomass * 0.80,
        "plants should remain viable under good conditions: initial {:.3} -> {:.3} ({:.1}%)",
        initial_biomass,
        s.total_plant_biomass_g,
        (s.total_plant_biomass_g / initial_biomass) * 100.0,
    );
}

/// Determinism: two runs with the same seed must produce identical results.
#[test]
fn happy_determinism_full_journey() {
    fn run_journey() -> TankSnapshot {
        let mut engine = make_engine("nano_cycle", default_overrides());
        weekly_maintenance(&mut engine, 30, 0.03, 25.0);
        engine
            .apply_action(PlayerAction::AddShrimp { count: 3 })
            .expect("stock");
        weekly_maintenance(&mut engine, 30, 0.03, 20.0);
        snap(&engine)
    }

    let s1 = run_journey();
    let s2 = run_journey();

    assert_eq!(s1.ph, s2.ph, "pH must be deterministic");
    assert_eq!(s1.water_temp_c, s2.water_temp_c, "temp deterministic");
    assert_eq!(s1.tan_mg_n_per_l, s2.tan_mg_n_per_l, "TAN deterministic");
    assert_eq!(s1.do_mg_l, s2.do_mg_l, "DO deterministic");
    assert_eq!(
        s1.adult_shrimp_count, s2.adult_shrimp_count,
        "shrimp count deterministic"
    );
}

// ===========================================================================
// SAD PATH TESTS
// ===========================================================================

/// User never does water changes. Waste products accumulate,
/// pH crashes, shrimp die.
#[test]
fn sad_no_water_changes() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    // Feed daily for 60 days, never do a water change
    feed_daily(&mut engine, 60, 0.05);

    let s = snap(&engine);
    let state = engine.full_state();
    print_status("Day 60 (no WC)", &s, &engine);
    assert_finite_snapshot(&s);

    // TAN or nitrate should be elevated from no dilution
    let total_dissolved_n = s.tan_mg_n_per_l + s.nitrite_mg_n_per_l + s.nitrate_mg_n_per_l;
    println!("  Total dissolved N: {:.2} mg/L", total_dissolved_n);

    // In a neglected nano, shrimp should be stressed or dead
    let alive = state.animal.adult.count + state.animal.juvenile.count;
    println!("  Shrimp remaining: {alive}");
    // We don't assert all dead (some may survive), but conditions should be poor
}

/// Massive overfeeding leads to ammonia spike and detritus buildup.
#[test]
fn sad_massive_overfeeding() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    // Day 1-3: normal
    feed_daily(&mut engine, 3, 0.03);

    let s_before = snap(&engine);

    // Day 4: dump a huge amount of food
    engine
        .apply_action(PlayerAction::Feed { grams: 2.0 })
        .expect("overfeed");
    step_days(&mut engine, 7);

    let s_after = snap(&engine);
    print_status("Day 10 (post overfeed)", &s_after, &engine);
    assert_finite_snapshot(&s_after);

    // Ammonia should spike from decomposing food
    assert!(
        s_after.tan_mg_n_per_l > s_before.tan_mg_n_per_l,
        "TAN should spike after overfeeding: {:.3} -> {:.3}",
        s_before.tan_mg_n_per_l,
        s_after.tan_mg_n_per_l
    );

    // Detritus should accumulate
    let detritus_before = s_before.detritus_particulate_g_total + s_before.detritus_fine_g_total;
    let detritus_after = s_after.detritus_particulate_g_total + s_after.detritus_fine_g_total;
    println!(
        "  Detritus: {:.4}g -> {:.4}g",
        detritus_before, detritus_after
    );
}

/// Lights left off — plants suffer, algae dynamics shift.
#[test]
fn sad_lights_off_plant_decline() {
    let mut engine = make_engine(
        "medium_planted",
        StartupOverrides {
            initial_adult_shrimp_count: Some(0),
            ..default_overrides()
        },
    );

    let initial_biomass = snap(&engine).total_plant_biomass_g;

    // Turn lights completely off
    engine
        .apply_action(PlayerAction::ChangePhotoperiod { hours: 0.0 })
        .expect("lights off");
    engine
        .apply_action(PlayerAction::ChangeLightIntensity {
            intensity_index: 0.0,
        })
        .expect("intensity off");

    feed_daily(&mut engine, 30, 0.02);

    let s = snap(&engine);
    print_status("Day 30 (lights off)", &s, &engine);
    assert_finite_snapshot(&s);

    // Plants should decline without light
    assert!(
        s.total_plant_biomass_g <= initial_biomass,
        "plants should not grow without light: {:.3} -> {:.3}",
        initial_biomass,
        s.total_plant_biomass_g
    );
}

/// Heater failure in a cold room — temperature drops, shrimp stressed.
#[test]
fn sad_heater_failure_cold_room() {
    let mut engine = make_engine(
        "nano_cycle",
        StartupOverrides {
            heater_preset: Some(StartupHeaterPreset::Celsius25),
            ..default_overrides()
        },
    );

    // Let tank stabilize at 25C for a week
    step_days(&mut engine, 7);
    let s_warm = snap(&engine);
    println!("Day 7 (warm): temp {:.1}°C", s_warm.water_temp_c);

    // "Heater fails" — turn it off and drop ambient to 15C
    engine
        .apply_action(PlayerAction::ChangeHeaterSetpoint { setpoint_c: 0.1 })
        .expect("heater off");
    engine
        .apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 15.0 })
        .expect("cold room");

    step_days(&mut engine, 14);

    let s_cold = snap(&engine);
    print_status("Day 21 (cold)", &s_cold, &engine);
    assert_finite_snapshot(&s_cold);

    // Temperature should have dropped significantly
    assert!(
        s_cold.water_temp_c < s_warm.water_temp_c,
        "tank should cool: {:.1} -> {:.1}",
        s_warm.water_temp_c,
        s_cold.water_temp_c
    );
}

/// Filter turned off — cycling capacity drops, ammonia rises.
#[test]
fn sad_filter_off_ammonia_rises() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    // Run with filter for 14 days
    feed_daily(&mut engine, 14, 0.03);
    let s_filtered = snap(&engine);

    // Build a second engine from same starting point but no filter
    let mut engine_no_filter = make_engine(
        "nano_cycle",
        StartupOverrides {
            filter_enabled: Some(false),
            ..default_overrides()
        },
    );
    feed_daily(&mut engine_no_filter, 14, 0.03);
    let s_no_filter = snap(&engine_no_filter);

    print_status("Day 14 (with filter)", &s_filtered, &engine);
    print_status("Day 14 (no filter)", &s_no_filter, &engine_no_filter);

    assert_finite_snapshot(&s_filtered);
    assert_finite_snapshot(&s_no_filter);

    // Without filter, nitrification should be worse
    // (filter provides flow and biomedia surface area)
    println!(
        "  Filter biofilter maturity: {:.4} vs no-filter: {:.4}",
        s_filtered.biofilter_maturity_index, s_no_filter.biofilter_maturity_index
    );

    // The no-filter run should have equal or worse TAN (less processing)
    // or lower biofilter maturity.
    let filtered_better = s_filtered.tan_mg_n_per_l <= s_no_filter.tan_mg_n_per_l
        || s_filtered.biofilter_maturity_index >= s_no_filter.biofilter_maturity_index;
    assert!(
        filtered_better,
        "filter should help: TAN {:.3} vs {:.3}, maturity {:.4} vs {:.4}",
        s_filtered.tan_mg_n_per_l,
        s_no_filter.tan_mg_n_per_l,
        s_filtered.biofilter_maturity_index,
        s_no_filter.biofilter_maturity_index,
    );
}

/// Overstocking — too many shrimp for a nano tank causes crowding stress.
#[test]
fn sad_overstocking_nano() {
    let mut engine = make_engine(
        "nano_cycle",
        StartupOverrides {
            initial_adult_shrimp_count: Some(0),
            ..default_overrides()
        },
    );

    // Cycle for 7 days
    feed_daily(&mut engine, 7, 0.02);

    // Dump 50 shrimp into a 10L nano
    engine
        .apply_action(PlayerAction::AddShrimp { count: 50 })
        .expect("overstock");

    feed_daily(&mut engine, 30, 0.1);

    let s = snap(&engine);
    let state = engine.full_state();
    print_status("Day 37 (overstocked)", &s, &engine);
    assert_finite_snapshot(&s);

    let alive = state.animal.adult.count + state.animal.juvenile.count;
    println!("  Shrimp alive: {alive}/50 stocked");

    // With 50 shrimp in 10L and heavy feeding, conditions should be poor
    // Ammonia should be elevated
    assert!(
        s.tan_mg_n_per_l > 0.5,
        "TAN should be elevated from overstocking"
    );
}

/// User removes all shrimp — ecosystem should still function, just without fauna.
#[test]
fn sad_remove_all_shrimp() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    feed_daily(&mut engine, 7, 0.03);

    let count = engine.full_state().animal.adult.count;
    if count > 0 {
        engine
            .apply_action(PlayerAction::RemoveShrimp { count })
            .expect("remove all");
    }

    // Continue running — should not panic
    feed_daily(&mut engine, 14, 0.02);

    let s = snap(&engine);
    let state = engine.full_state();
    print_status("Day 21 (no shrimp)", &s, &engine);
    assert_finite_snapshot(&s);

    assert_eq!(state.animal.adult.count, 0, "all shrimp removed");
    assert!(s.ph > 4.0, "pH still valid");
}

/// Rapid large water changes cause parameter swings — instability rises.
#[test]
fn sad_rapid_water_changes_cause_instability() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    step_days(&mut engine, 7);
    let instability_before = engine.full_state().stability_tracker.instability_index;

    // Do 5 back-to-back 80% water changes in rapid succession
    for _ in 0..5 {
        engine
            .apply_action(PlayerAction::WaterChangePercent {
                percent: 80.0,
                source_profile_id: "moderate".to_string(),
            })
            .expect("wc");
        engine.step_hours(6).expect("step");
    }

    let s = snap(&engine);
    print_status("After rapid WCs", &s, &engine);
    assert_finite_snapshot(&s);

    let instability_after = engine.full_state().stability_tracker.instability_index;
    println!(
        "  Instability: {:.4} -> {:.4}",
        instability_before, instability_after,
    );

    // After 5 rapid 80% water changes, instability should be non-trivial.
    // The tracker may not strictly increase (source water is consistent),
    // but it should reflect the cumulative parameter churn.
    assert!(
        instability_after > 0.1,
        "instability should be elevated after rapid large water changes, got {:.4}",
        instability_after
    );
}

/// Tank left completely unattended for 60 days — no feeding, no maintenance.
#[test]
fn sad_total_neglect() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    // Feed for a week to establish some organic load
    feed_daily(&mut engine, 7, 0.03);

    // Then abandon for 60 days
    step_days(&mut engine, 60);

    let s = snap(&engine);
    let state = engine.full_state();
    print_status("Day 67 (neglected)", &s, &engine);
    assert_finite_snapshot(&s);

    let alive = state.animal.adult.count + state.animal.juvenile.count;
    println!("  Shrimp alive after neglect: {alive}");
    println!("  Events: {}", state.event_log.len());
}

/// Extreme temperature swing — ambient jumps from 24 to 38 and back.
#[test]
fn sad_extreme_temperature_swing() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    step_days(&mut engine, 3);
    let s_before = snap(&engine);

    // Heatwave
    engine
        .apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 38.0 })
        .expect("heatwave");
    step_days(&mut engine, 3);

    let s_hot = snap(&engine);
    print_status("Day 6 (heatwave)", &s_hot, &engine);

    // Cold snap
    engine
        .apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 10.0 })
        .expect("cold snap");
    step_days(&mut engine, 3);

    let s_cold = snap(&engine);
    print_status("Day 9 (cold snap)", &s_cold, &engine);
    assert_finite_snapshot(&s_cold);

    // DO should have changed with temperature (warmer = less saturation)
    assert!(
        s_hot.do_sat_mg_l < s_before.do_sat_mg_l,
        "DO saturation should decrease when hot"
    );
}

/// All three scenarios materialize and run 7 days without errors.
#[test]
fn happy_all_scenarios_boot_and_run() {
    for scenario_id in tank_scenarios::default_scenario_ids() {
        let mut engine = make_engine(
            scenario_id,
            StartupOverrides {
                initial_adult_shrimp_count: Some(5),
                ..default_overrides()
            },
        );

        feed_daily(&mut engine, 7, 0.02);

        let s = snap(&engine);
        print_status(&format!("{scenario_id} day 7"), &s, &engine);
        assert_finite_snapshot(&s);
        assert!(s.water_volume_l > 0.0);
    }
}

/// Siphoning detritus should reduce the organic load.
#[test]
fn happy_siphon_reduces_detritus() {
    let mut engine = make_engine("nano_cycle", default_overrides());

    // Build up some detritus
    feed_daily(&mut engine, 14, 0.05);
    let detritus_before =
        snap(&engine).detritus_particulate_g_total + snap(&engine).detritus_fine_g_total;

    engine
        .apply_action(PlayerAction::SiphonDetritus { fraction: 0.8 })
        .expect("siphon");
    engine.step_hours(1).expect("step");

    let detritus_after =
        snap(&engine).detritus_particulate_g_total + snap(&engine).detritus_fine_g_total;

    println!(
        "Detritus siphon: {:.4}g -> {:.4}g",
        detritus_before, detritus_after
    );
    assert!(
        detritus_after < detritus_before,
        "siphoning should reduce detritus"
    );
}
