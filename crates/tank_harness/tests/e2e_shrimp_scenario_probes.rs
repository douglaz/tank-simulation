//! Shrimp-focused scenario probes (tanksim-6e5.6.6).
//!
//! Five scenario-level tests that confirm shrimp life-history mechanics
//! produce qualitatively correct outcomes across success and failure modes.
//! Each probe tells a husbandry story that a player or developer can read
//! to understand what the simulation is modelling.
//!
//! 1. **Successful breeding**: mature planted tank, stable conditions,
//!    adequate minerals → population grows over 2+ reproductive cycles.
//! 2. **Thermal suppression**: temperature above 30 °C → breeding stops,
//!    egg dropping, reproductive readiness collapses.
//! 3. **Chemistry-induced stress**: low GH + high NO₂ → molt failures,
//!    increased mortality, condition decline.
//! 4. **Chloride protection**: same NO₂ but high chloride → less mortality
//!    than the low-chloride control.
//! 5. **Crash mode**: overcrowding + overfeeding + no water changes →
//!    population collapse.
//!
//! Assertions use directional comparisons and envelope ranges so the tests
//! remain resilient to reasonable parameter retuning. Exact counts are not
//! frozen — later calibration can adjust numbers without rewriting the
//! scientific story.
//!
//! Run all probes:  `cargo test --test e2e_shrimp_scenario_probes`
//! Verbose traces:  `TANK_E2E_VERBOSE=1 cargo test --test e2e_shrimp_scenario_probes -- --nocapture`
//! Summary only:    `cargo test --test e2e_shrimp_scenario_probes all_shrimp_probes_summary -- --nocapture`
//!
//! Exit codes: 0 = all pass, non-zero = envelope violation.

use tank_core::{
    systems::chemistry::{
        bicarbonate_mg_total_from_mmol_per_l, validate_source_water_carbonate_profile,
    },
    Engine, PlayerAction, ProcessParams, SimSeed, SimulationEngine, SourceWaterProfile,
    TankGeometry, TankSnapshot, TankState, WaterState,
};
use tank_harness::{Envelope, HarnessRun};

// ---------------------------------------------------------------------------
// Probe result tracking for the summary report
// ---------------------------------------------------------------------------

struct ProbeResult {
    name: &'static str,
    passed: bool,
    /// One-line summary of observed population metrics.
    observed: String,
    /// Detail message on failure (empty string if passed).
    failure_detail: String,
}

// ---------------------------------------------------------------------------
// Debug helpers — dump shrimp state on failure
// ---------------------------------------------------------------------------

/// Format per-stage shrimp state and water chemistry for diagnostic output.
///
/// Dumps counts, condition, reserve, molt status, and key chemistry
/// (GH, pH, NO₂, Cl) in a compact format for assertion messages.
fn shrimp_diag(snap: &TankSnapshot) -> String {
    format!(
        "shrimp={} (adult={}, sub_adult={}, juv={}, berried={}), \
         cond={:.3}, molt_stress={:.3}, readiness={:.3}, \
         repro_suppression={}, \
         pH={:.2}, GH={:.1}, NO2={:.4}, Cl={:.2}, DO={:.2}, temp={:.1}C",
        snap.total_shrimp_count,
        snap.adult_shrimp_count,
        snap.sub_adult_count,
        snap.juveniles_count,
        snap.berried_females_count,
        snap.shrimp_condition_index,
        snap.shrimp_molt_stress_index,
        snap.shrimp_reproductive_readiness,
        snap.repro_dominant_suppression,
        snap.ph,
        snap.gh_d,
        snap.nitrite_mg_n_per_l,
        snap.chloride_mg_per_l,
        snap.do_mg_l,
        snap.water_temp_c,
    )
}

// ---------------------------------------------------------------------------
// 1. Successful breeding
// ---------------------------------------------------------------------------

/// Successful breeding in a mature planted tank.
///
/// **Husbandry story**: A well-cycled 200 L planted tank stocked with 10
/// adult neocaridina shrimp receives regular feeding. Mineral levels
/// (Ca 40 mg/L, Mg 10 mg/L, GH ~8) support successful molts. Temperature
/// is a stable 25 °C with strong nitrification and abundant periphyton.
/// Mortality is isolated out so the test focuses purely on reproductive
/// mechanics. Over 1 200 simulated hours (~50 days) the colony should
/// produce at least 2 complete reproductive cycles: adults become berried,
/// eggs hatch, and juveniles appear. Total population should grow.
#[test]
fn probe_successful_breeding() -> Result<(), Box<dyn std::error::Error>> {
    let state = breeding_success_state(SimSeed(6600));

    let mut run = HarnessRun::from_state(SimSeed(6600), "breeding_success", state)
        .with_artifact_label("shrimp_successful_breeding");
    run.enable_instrumentation();

    let initial_snap = run.snapshot();
    let initial_count = initial_snap.total_shrimp_count;

    // Track berried-female appearances across distinct time windows to detect
    // at least 2 reproductive cycles.
    let mut berried_windows: Vec<bool> = Vec::new();
    let window_hours = 200u32; // ~8-day windows across the run
    let total_hours: u32 = 1200;
    let num_windows = total_hours / window_hours;

    for window_idx in 0..num_windows {
        let mut saw_berried_in_window = false;
        for _ in 0..(window_hours / 24) {
            run.apply_action(PlayerAction::Feed { grams: 0.15 })?;
            run.step_hours(24)?;

            let snap = run.snapshot();
            if snap.berried_females_count > 0 {
                saw_berried_in_window = true;
            }
        }
        // Advance remaining hours in the window beyond full days
        let remaining = window_hours % 24;
        if remaining > 0 {
            run.step_hours(remaining)?;
        }
        berried_windows.push(saw_berried_in_window);

        // Periodic envelope: chemistry should stay safe throughout
        run.assert_envelope(
            &format!("window_{}", window_idx + 1),
            &Envelope::default()
                .ph(5.5, 8.5)
                .temperature_c(22.0, 28.0)
                .do_min(2.0),
        );
    }

    let final_snap = run.snapshot();
    let final_count = final_snap.total_shrimp_count;

    // At least 2 windows should have observed berried females (= 2 reproductive cycles)
    let berried_cycle_count = berried_windows.iter().filter(|&&b| b).count();

    run.assert_snapshot("population_grew", |snap| {
        if snap.total_shrimp_count <= initial_count {
            Err(format!(
                "population should grow: initial={initial_count}, final={}. {}",
                snap.total_shrimp_count,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    run.assert_snapshot("reproductive_cycles", |snap| {
        if berried_cycle_count < 2 {
            Err(format!(
                "expected at least 2 reproductive cycles (berried-female windows), \
                 observed {berried_cycle_count} out of {num_windows} windows. {}",
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    // Juveniles should be present by the end (evidence of successful hatching)
    run.assert_snapshot("juveniles_present", |snap| {
        if snap.juveniles_count == 0 && snap.sub_adult_count == 0 {
            Err(format!(
                "expected juveniles or sub-adults from hatching, got neither. {}",
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    eprintln!(
        "probe_successful_breeding: initial={initial_count}, final={final_count}, \
         berried_windows={berried_cycle_count}/{num_windows}. {}",
        shrimp_diag(&final_snap),
    );

    run.finish().map_err(|e| e.into())
}

fn run_probe_successful_breeding() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = breeding_success_state(SimSeed(6600));
    let mut engine = Engine::from_parts(state, vec![]);

    let initial_count = engine.snapshot().total_shrimp_count;
    let mut berried_windows: Vec<bool> = Vec::new();
    let window_hours = 200u32;
    let total_hours = 1200u32;
    let num_windows = total_hours / window_hours;

    for _ in 0..num_windows {
        let mut saw_berried = false;
        for _ in 0..(window_hours / 24) {
            engine.apply_action(PlayerAction::Feed { grams: 0.15 })?;
            engine.step_hours(24)?;
            if engine.snapshot().berried_females_count > 0 {
                saw_berried = true;
            }
        }
        let remaining = window_hours % 24;
        if remaining > 0 {
            engine.step_hours(remaining)?;
        }
        berried_windows.push(saw_berried);
    }

    let snap = engine.snapshot();
    let berried_cycle_count = berried_windows.iter().filter(|&&b| b).count();
    let grew = snap.total_shrimp_count > initial_count;
    let enough_cycles = berried_cycle_count >= 2;
    let has_offspring = snap.juveniles_count > 0 || snap.sub_adult_count > 0;
    let passed = grew && enough_cycles && has_offspring;

    let observed = format!(
        "initial={initial_count}, final={}, cycles={berried_cycle_count}/{num_windows}, juv={}, sub={}",
        snap.total_shrimp_count, snap.juveniles_count, snap.sub_adult_count,
    );
    let failure_detail = if passed {
        String::new()
    } else {
        let mut detail = Vec::new();
        if !grew {
            detail.push(format!(
                "population did not grow (initial={initial_count}, final={})",
                snap.total_shrimp_count
            ));
        }
        if !enough_cycles {
            detail.push(format!(
                "only {berried_cycle_count} reproductive cycles (need >= 2)"
            ));
        }
        if !has_offspring {
            detail.push("no juveniles or sub-adults present".to_string());
        }
        format!("{} | {}", detail.join("; "), shrimp_diag(&snap))
    };

    Ok(ProbeResult {
        name: "successful_breeding",
        passed,
        observed,
        failure_detail,
    })
}

/// Build a well-maintained planted tank optimised for breeding success.
///
/// Mortality is disabled so the probe isolates reproductive mechanics.
/// Strong nitrification prevents nitrite buildup; abundant periphyton and
/// good minerals support condition and molting.
fn breeding_success_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
        length_cm: 80.0,
        width_cm: 50.0,
        height_cm: 55.0,
        fill_height_cm: 50.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 12.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;

    // Abundant periphyton so food is never limiting
    state.algae.set_periphyton_total(30.0);

    // Very strong nitrification to keep ammonia/nitrite near zero
    state.microbe.set_decomposer_total(0.3);
    state.microbe.ammonia_oxidizer_biomass_g = 2.0;
    state.microbe.nitrite_oxidizer_biomass_g = 1.5;
    state.microbe.comammox_biomass_g = 0.5;
    state.filter_state.biofilter_maturity_index = 1.0;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.7;
    state.hardware.light.photoperiod_hours = 10.0;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.8;
    state.animal.adult.reserve_g = 5.0;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.periphyton_capacity_g_per_m2 = 200.0;
    // Disable mortality to isolate reproductive mechanics
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;

    state.shrimp_params.base_spawn_rate = 0.08;
    state.shrimp_params.hatch_success_base = 1.0;
    state.shrimp_params.egg_duration_days = 14;
    state
        .shrimp_params
        .apply_legacy_total_maturation_days(120.0);

    state.reseed_stability_tracker();
    state
}

// ---------------------------------------------------------------------------
// 2. Thermal suppression
// ---------------------------------------------------------------------------

/// Thermal suppression of breeding at high temperature.
///
/// **Husbandry story**: Two identical colonies start with 10 adults in
/// well-maintained tanks. One tank is kept at a comfortable 25 °C, the other
/// is pushed to 31 °C (above the species' optimal range). Over 60 days the
/// warm tank should show: suppressed reproductive readiness, fewer or no
/// berried females, and fewer juveniles compared to the cool tank. Heat
/// stress accumulates hourly and feeds into molt stress, which in turn
/// suppresses spawning.
#[test]
fn probe_thermal_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let cool_state = thermal_scenario_state(SimSeed(6601), 25.0);
    let warm_state = thermal_scenario_state(SimSeed(6601), 31.0);

    let mut cool_run = HarnessRun::from_state(SimSeed(6601), "thermal_cool", cool_state)
        .with_artifact_label("shrimp_thermal_cool");
    let mut warm_run = HarnessRun::from_state(SimSeed(6601), "thermal_warm", warm_state)
        .with_artifact_label("shrimp_thermal_warm");
    cool_run.enable_instrumentation();
    warm_run.enable_instrumentation();

    // Run both for 60 days with identical care
    for _ in 0..60 {
        cool_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        warm_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        cool_run.step_hours(24)?;
        warm_run.step_hours(24)?;
    }

    let cool_snap = cool_run.snapshot();
    let warm_snap = warm_run.snapshot();

    // Warm tank should have lower reproductive readiness
    cool_run.assert_snapshot("warm_readiness_suppressed", |_| {
        if warm_snap.shrimp_reproductive_readiness >= cool_snap.shrimp_reproductive_readiness {
            Err(format!(
                "warm tank readiness ({:.3}) should be lower than cool tank ({:.3}). \
                 Cool: {} | Warm: {}",
                warm_snap.shrimp_reproductive_readiness,
                cool_snap.shrimp_reproductive_readiness,
                shrimp_diag(&cool_snap),
                shrimp_diag(&warm_snap),
            ))
        } else {
            Ok(())
        }
    });

    // Warm tank should have worse reproductive outcomes (fewer juveniles or
    // lower readiness when both have zero juveniles)
    cool_run.assert_snapshot("warm_worse_outcomes", |_| {
        let warm_worse = if cool_snap.juveniles_count > 0 || warm_snap.juveniles_count > 0 {
            warm_snap.juveniles_count < cool_snap.juveniles_count
        } else {
            warm_snap.shrimp_reproductive_readiness <= cool_snap.shrimp_reproductive_readiness
        };
        if !warm_worse {
            Err(format!(
                "warm tank should have worse reproductive outcomes. \
                 Cool juv={}, Warm juv={}, Cool readiness={:.3}, Warm readiness={:.3}",
                cool_snap.juveniles_count,
                warm_snap.juveniles_count,
                cool_snap.shrimp_reproductive_readiness,
                warm_snap.shrimp_reproductive_readiness,
            ))
        } else {
            Ok(())
        }
    });

    eprintln!(
        "probe_thermal_suppression: Cool: {} | Warm: {}",
        shrimp_diag(&cool_snap),
        shrimp_diag(&warm_snap),
    );

    let cool_result = cool_run.finish();
    let warm_result = warm_run.finish();
    cool_result.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    warm_result.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    Ok(())
}

/// Build a stocked shrimp tank at the given ambient temperature, isolating
/// thermal effects from food limitation and ammonia stress.
fn thermal_scenario_state(seed: SimSeed, ambient_temp_c: f64) -> TankState {
    let geometry = TankGeometry {
        length_cm: 80.0,
        width_cm: 50.0,
        height_cm: 55.0,
        fill_height_cm: 50.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = ambient_temp_c;
    state.environment.ambient_temp_c = ambient_temp_c;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 12.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;

    // Abundant periphyton so food is never limiting
    state.algae.set_periphyton_total(30.0);

    // Very strong nitrification
    state.microbe.set_decomposer_total(0.3);
    state.microbe.ammonia_oxidizer_biomass_g = 2.0;
    state.microbe.nitrite_oxidizer_biomass_g = 1.5;
    state.microbe.comammox_biomass_g = 0.5;
    state.filter_state.biofilter_maturity_index = 1.0;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.7;
    state.hardware.light.photoperiod_hours = 10.0;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.8;
    state.animal.adult.reserve_g = 5.0;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.periphyton_capacity_g_per_m2 = 200.0;
    // Disable mortality to isolate reproductive effects
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;

    state.shrimp_params.base_spawn_rate = 0.08;
    state.shrimp_params.hatch_success_base = 1.0;
    state.shrimp_params.egg_duration_days = 14;
    state
        .shrimp_params
        .apply_legacy_total_maturation_days(120.0);

    state.reseed_stability_tracker();
    state
}

fn run_probe_thermal_suppression() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let cool_state = thermal_scenario_state(SimSeed(6601), 25.0);
    let warm_state = thermal_scenario_state(SimSeed(6601), 31.0);

    let mut cool_engine = Engine::from_parts(cool_state, vec![]);
    let mut warm_engine = Engine::from_parts(warm_state, vec![]);

    for _ in 0..60 {
        cool_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        warm_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        cool_engine.step_hours(24)?;
        warm_engine.step_hours(24)?;
    }

    let cool_snap = cool_engine.snapshot();
    let warm_snap = warm_engine.snapshot();

    let readiness_suppressed =
        warm_snap.shrimp_reproductive_readiness < cool_snap.shrimp_reproductive_readiness;
    let worse_outcomes = if cool_snap.juveniles_count > 0 || warm_snap.juveniles_count > 0 {
        warm_snap.juveniles_count < cool_snap.juveniles_count
    } else {
        warm_snap.shrimp_reproductive_readiness <= cool_snap.shrimp_reproductive_readiness
    };

    let passed = readiness_suppressed && worse_outcomes;
    let observed = format!(
        "cool_readiness={:.3}, warm_readiness={:.3}, cool_juv={}, warm_juv={}",
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.shrimp_reproductive_readiness,
        cool_snap.juveniles_count,
        warm_snap.juveniles_count,
    );
    let failure_detail = if passed {
        String::new()
    } else {
        let mut issues = Vec::new();
        if !readiness_suppressed {
            issues.push("readiness not suppressed at 31C");
        }
        if !worse_outcomes {
            issues.push("warm tank did not have worse reproductive outcomes");
        }
        format!(
            "{} | Cool: {} | Warm: {}",
            issues.join("; "),
            shrimp_diag(&cool_snap),
            shrimp_diag(&warm_snap)
        )
    };

    Ok(ProbeResult {
        name: "thermal_suppression",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// 3. Chemistry-induced stress (low GH + high NO₂)
// ---------------------------------------------------------------------------

/// Chemistry-induced molt failures and mortality.
///
/// **Husbandry story**: A tank has dangerously low general hardness (GH < 3,
/// calcium 5 mg/L, magnesium 2 mg/L) and elevated nitrite from an incomplete
/// nitrogen cycle (4 mg N/L). With inadequate minerals the shrimp cannot
/// build proper exoskeletons, leading to molt failures. Elevated nitrite
/// compounds the stress. Over 21 days the colony should show: rising molt
/// stress, declining condition, and significant mortality. This scenario
/// models the common beginner mistake of keeping shrimp in soft water
/// without remineralisation.
#[test]
fn probe_chemistry_stress() -> Result<(), Box<dyn std::error::Error>> {
    let state = chemistry_stress_state(SimSeed(6602));

    let mut run = HarnessRun::from_state(SimSeed(6602), "chemistry_stress", state)
        .with_artifact_label("shrimp_chemistry_stress");
    run.enable_instrumentation();

    let initial_snap = run.snapshot();
    let initial_count = initial_snap.total_shrimp_count;
    let initial_condition = initial_snap.shrimp_condition_index;

    // Run 21 days with light feeding, no water changes (simulating neglect)
    for _ in 0..21 {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;
    }

    let final_snap = run.snapshot();

    // Population should have declined from nitrite + mineral stress
    run.assert_snapshot("mortality_occurred", |snap| {
        if snap.total_shrimp_count >= initial_count {
            Err(format!(
                "low GH + high NO₂ should cause mortality: initial={initial_count}, final={}. {}",
                snap.total_shrimp_count,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    // Molt stress should be elevated
    run.assert_snapshot("elevated_molt_stress", |snap| {
        if snap.shrimp_molt_stress_index <= 0.2 {
            Err(format!(
                "molt stress should be elevated (> 0.2) under low-mineral conditions, \
                 got {:.3}. {}",
                snap.shrimp_molt_stress_index,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    // Condition should have declined
    run.assert_snapshot("condition_declined", |snap| {
        if snap.shrimp_condition_index >= initial_condition {
            Err(format!(
                "condition should decline under stress: initial={initial_condition:.3}, \
                 final={:.3}. {}",
                snap.shrimp_condition_index,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    eprintln!(
        "probe_chemistry_stress: initial={initial_count}, final={}, \
         condition {initial_condition:.3} -> {:.3}, molt_stress={:.3}. {}",
        final_snap.total_shrimp_count,
        final_snap.shrimp_condition_index,
        final_snap.shrimp_molt_stress_index,
        shrimp_diag(&final_snap),
    );

    run.finish().map_err(|e| e.into())
}

/// Build a tank with dangerously low minerals and elevated nitrite.
fn chemistry_stress_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
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
    // Dangerously low minerals (GH < 3)
    state.water.calcium_mg_total = 5.0 * vol;
    state.water.magnesium_mg_total = 2.0 * vol;
    state.water.alkalinity_meq_total = 3.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 7.0 * vol;
    state.water.bicarbonate_mg_total = 100.0 * vol;
    // High nitrite from incomplete cycle
    state.water.nitrite_mg_n_total = 4.0 * vol;
    // No chloride protection
    state.water.chloride_mg_total = 0.0;

    state.algae.set_periphyton_total(3.0);

    // Weak nitrification so nitrite stays elevated
    state.microbe.set_decomposer_total(0.1);
    state.microbe.ammonia_oxidizer_biomass_g = 0.5;
    state.microbe.nitrite_oxidizer_biomass_g = 0.05;
    state.microbe.comammox_biomass_g = 0.05;
    state.filter_state.biofilter_maturity_index = 0.3;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.5;
    state.hardware.light.photoperiod_hours = 8.0;

    // 20 adults at decent starting condition
    state.animal.adult.count = 20;
    state.animal.set_population_condition_index(0.7);
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.3;

    state.process_params = ProcessParams::default();
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.3;
    // Disable spawning to isolate mortality effects
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

fn run_probe_chemistry_stress() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = chemistry_stress_state(SimSeed(6602));
    let mut engine = Engine::from_parts(state, vec![]);
    let initial_snap = engine.snapshot();
    let initial_count = initial_snap.total_shrimp_count;
    let initial_condition = initial_snap.shrimp_condition_index;

    for _ in 0..21 {
        engine.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        engine.step_hours(24)?;
    }

    let snap = engine.snapshot();
    let mortality = snap.total_shrimp_count < initial_count;
    let elevated_stress = snap.shrimp_molt_stress_index > 0.2;
    let condition_declined = snap.shrimp_condition_index < initial_condition;
    let passed = mortality && elevated_stress && condition_declined;

    let observed = format!(
        "count {initial_count}->{}, cond {initial_condition:.3}->{:.3}, molt_stress={:.3}",
        snap.total_shrimp_count, snap.shrimp_condition_index, snap.shrimp_molt_stress_index,
    );
    let failure_detail = if passed {
        String::new()
    } else {
        let mut issues = Vec::new();
        if !mortality {
            issues.push(format!(
                "no mortality (count stayed at {})",
                snap.total_shrimp_count
            ));
        }
        if !elevated_stress {
            issues.push(format!(
                "molt stress not elevated ({:.3})",
                snap.shrimp_molt_stress_index
            ));
        }
        if !condition_declined {
            issues.push(format!(
                "condition did not decline ({:.3} >= {initial_condition:.3})",
                snap.shrimp_condition_index
            ));
        }
        format!("{} | {}", issues.join("; "), shrimp_diag(&snap))
    };

    Ok(ProbeResult {
        name: "chemistry_stress",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// 4. Chloride protection
// ---------------------------------------------------------------------------

/// Chloride protection against nitrite hazard.
///
/// **Husbandry story**: Two tanks experience the same nitrite crisis
/// (4 mg N/L from an incomplete cycle). Both run 3 days unprotected. Then
/// the keeper performs a 50 % water change on both tanks, but only one uses
/// salt-enriched replacement water (NaCl raising chloride to ~200 mg/L).
/// The control gets a matched water change without extra salt. Chloride
/// competes with nitrite at the gill uptake sites, reducing effective
/// toxicity. After 3 more days the salted tank should show lower mortality
/// than the unsalted control.
#[test]
fn probe_chloride_protection() -> Result<(), Box<dyn std::error::Error>> {
    let base_state = chloride_test_base_state(SimSeed(6603));
    let initial_count = base_state.animal.total_count();

    // Phase 1: both tanks run 3 days unprotected
    let mut crash_engine = Engine::from_parts(base_state, vec![]);
    crash_engine.step_hours(3 * 24)?;
    let pre_treatment = crash_engine.full_state().clone();
    let count_before_treatment = pre_treatment.animal.total_count();
    let phase1_deaths = initial_count - count_before_treatment;

    // Build source-water profiles: one with salt, one matched control
    let vol = pre_treatment.water_volume_l().max(f64::EPSILON);
    let baseline_sodium = pre_treatment.water.sodium_mg_total / vol;
    let baseline_chloride = pre_treatment.concentrations().chloride_mg_per_l();

    let salt_profile = chloride_source_profile(&pre_treatment, baseline_sodium + 130.0, 200.0);
    let control_profile =
        chloride_source_profile(&pre_treatment, baseline_sodium, baseline_chloride);

    // Treated branch: 50% water change with salt-enriched water
    let mut treated_state = pre_treatment.clone();
    treated_state
        .source_water_catalog
        .insert("salt_treatment".to_string(), salt_profile);
    let mut treated_engine = Engine::from_parts(treated_state, vec![]);

    // Control branch: 50% water change with matched (no extra salt) water
    let mut control_state = pre_treatment;
    control_state
        .source_water_catalog
        .insert("matched_control".to_string(), control_profile);
    let mut control_engine = Engine::from_parts(control_state, vec![]);

    // Apply water changes
    treated_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    control_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "matched_control".to_string(),
    })?;
    treated_engine.step_hours(1)?;
    control_engine.step_hours(1)?;

    // Phase 2: run both for 3 more days
    treated_engine.step_hours(71)?;
    control_engine.step_hours(71)?;

    let treated_snap = treated_engine.snapshot();
    let control_snap = control_engine.snapshot();
    let treated_deaths =
        count_before_treatment.saturating_sub(treated_engine.full_state().animal.total_count());
    let control_deaths =
        count_before_treatment.saturating_sub(control_engine.full_state().animal.total_count());

    // Use a HarnessRun for structured failure recording
    let mut run =
        HarnessRun::from_state(SimSeed(6603), "chloride_probe", TankState::new(SimSeed(0)))
            .with_artifact_label("shrimp_chloride_protection");

    if treated_deaths >= control_deaths {
        run.record_failure(
            "chloride_less_mortality",
            format!(
                "salt-treated tank should have fewer post-treatment deaths: \
                 treated={treated_deaths}, control={control_deaths}. \
                 Treated: {} | Control: {}",
                shrimp_diag(&treated_snap),
                shrimp_diag(&control_snap),
            ),
        );
    }

    if treated_snap.effective_nitrite_hazard_mg_per_l
        >= control_snap.effective_nitrite_hazard_mg_per_l
    {
        run.record_failure(
            "chloride_reduces_hazard",
            format!(
                "chloride should reduce effective nitrite hazard: \
                 treated={:.4}, control={:.4}",
                treated_snap.effective_nitrite_hazard_mg_per_l,
                control_snap.effective_nitrite_hazard_mg_per_l,
            ),
        );
    }

    eprintln!(
        "probe_chloride_protection: phase1_deaths={phase1_deaths}, \
         post_treatment: treated_deaths={treated_deaths}, control_deaths={control_deaths}. \
         Treated: {} | Control: {}",
        shrimp_diag(&treated_snap),
        shrimp_diag(&control_snap),
    );

    run.finish().map_err(|e| e.into())
}

/// Build a carbonate-consistent source-water profile that matches the
/// current tank chemistry except for sodium and chloride.
fn chloride_source_profile(
    state: &TankState,
    sodium_mg_per_l: f64,
    chloride_mg_per_l: f64,
) -> SourceWaterProfile {
    let vol = state.water_volume_l().max(f64::EPSILON);
    let (dic, alk, bicarb) = valid_source_profile_carbonate(state);
    SourceWaterProfile {
        temperature_c: state.water.temperature_c,
        ammonia_mg_n_per_l: state.tan_mg_n_per_l(),
        nitrite_mg_n_per_l: state.nitrite_mg_n_per_l(),
        nitrate_mg_n_per_l: state.nitrate_mg_n_per_l(),
        phosphate_mg_p_per_l: state.phosphate_mg_p_per_l(),
        dic_mg_c_per_l: dic,
        doc_mg_c_per_l: state.doc_mg_c_per_l(),
        don_mg_n_per_l: state.don_mg_n_per_l(),
        alkalinity_meq_per_l: alk,
        calcium_mg_per_l: state.calcium_mg_per_l(),
        magnesium_mg_per_l: state.magnesium_mg_per_l(),
        sodium_mg_per_l,
        potassium_mg_per_l: state.water.potassium_mg_total / vol,
        bicarbonate_mg_per_l: bicarb,
        chloride_mg_per_l,
        sulfate_mg_per_l: state.water.sulfate_mg_total / vol,
    }
}

/// Resolve a valid DIC/alkalinity/bicarbonate triple for source-water
/// construction, falling back to defaults if the current state is out of
/// the solver's valid range.
fn valid_source_profile_carbonate(state: &TankState) -> (f64, f64, f64) {
    let dic = state.dic_mg_c_per_l();
    let alk = state.alkalinity_meq_per_l();
    if let Ok(eq) = validate_source_water_carbonate_profile(dic, alk, state.water.temperature_c) {
        return (
            dic,
            alk,
            bicarbonate_mg_total_from_mmol_per_l(eq.hco3_mmol_per_l, 1.0),
        );
    }
    let fallback = WaterState::default_for_volume_l(1.0);
    let eq = validate_source_water_carbonate_profile(
        fallback.dissolved_inorganic_carbon_mg_c_total,
        fallback.alkalinity_meq_total,
        state.water.temperature_c,
    )
    .expect("default source-water carbonate profile should validate");
    (
        fallback.dissolved_inorganic_carbon_mg_c_total,
        fallback.alkalinity_meq_total,
        bicarbonate_mg_total_from_mmol_per_l(eq.hco3_mmol_per_l, 1.0),
    )
}

/// Build a nitrite-crash state for chloride protection testing.
fn chloride_test_base_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
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
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.set_periphyton_total(5.0);

    // Cycling crash: AOB active but NOB insufficient → nitrite accumulates
    state.microbe.set_decomposer_total(0.1);
    state.microbe.ammonia_oxidizer_biomass_g = 1.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.05;
    state.microbe.comammox_biomass_g = 0.05;
    state.filter_state.biofilter_maturity_index = 0.4;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    // High nitrite, no chloride initially
    state.water.nitrite_mg_n_total = 4.0 * vol;
    state.water.chloride_mg_total = 0.0;

    state.animal.adult.count = 20;
    state.animal.set_population_condition_index(0.7);
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.3;

    state.process_params = ProcessParams::default();
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.3;
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

fn run_probe_chloride_protection() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let base_state = chloride_test_base_state(SimSeed(6603));
    let initial_count = base_state.animal.total_count();

    let mut crash_engine = Engine::from_parts(base_state, vec![]);
    crash_engine.step_hours(3 * 24)?;
    let pre_treatment = crash_engine.full_state().clone();
    let count_before_treatment = pre_treatment.animal.total_count();

    let vol = pre_treatment.water_volume_l().max(f64::EPSILON);
    let baseline_sodium = pre_treatment.water.sodium_mg_total / vol;
    let baseline_chloride = pre_treatment.concentrations().chloride_mg_per_l();

    let salt_profile = chloride_source_profile(&pre_treatment, baseline_sodium + 130.0, 200.0);
    let control_profile =
        chloride_source_profile(&pre_treatment, baseline_sodium, baseline_chloride);

    let mut treated_state = pre_treatment.clone();
    treated_state
        .source_water_catalog
        .insert("salt_treatment".to_string(), salt_profile);
    let mut treated_engine = Engine::from_parts(treated_state, vec![]);

    let mut control_state = pre_treatment;
    control_state
        .source_water_catalog
        .insert("matched_control".to_string(), control_profile);
    let mut control_engine = Engine::from_parts(control_state, vec![]);

    treated_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    control_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "matched_control".to_string(),
    })?;
    treated_engine.step_hours(1)?;
    control_engine.step_hours(1)?;

    treated_engine.step_hours(71)?;
    control_engine.step_hours(71)?;

    let treated_snap = treated_engine.snapshot();
    let control_snap = control_engine.snapshot();
    let treated_deaths =
        count_before_treatment.saturating_sub(treated_engine.full_state().animal.total_count());
    let control_deaths =
        count_before_treatment.saturating_sub(control_engine.full_state().animal.total_count());

    let less_mortality = treated_deaths < control_deaths;
    let lower_hazard = treated_snap.effective_nitrite_hazard_mg_per_l
        < control_snap.effective_nitrite_hazard_mg_per_l;
    let passed = less_mortality && lower_hazard;

    let observed = format!(
        "phase1_deaths={}, post: treated_deaths={treated_deaths}, control_deaths={control_deaths}, \
         treated_hazard={:.4}, control_hazard={:.4}",
        initial_count - count_before_treatment,
        treated_snap.effective_nitrite_hazard_mg_per_l,
        control_snap.effective_nitrite_hazard_mg_per_l,
    );
    let failure_detail = if passed {
        String::new()
    } else {
        let mut issues = Vec::new();
        if !less_mortality {
            issues.push(format!(
                "treated deaths ({treated_deaths}) not less than control ({control_deaths})"
            ));
        }
        if !lower_hazard {
            issues.push(format!(
                "treated hazard ({:.4}) not lower than control ({:.4})",
                treated_snap.effective_nitrite_hazard_mg_per_l,
                control_snap.effective_nitrite_hazard_mg_per_l,
            ));
        }
        format!(
            "{} | Treated: {} | Control: {}",
            issues.join("; "),
            shrimp_diag(&treated_snap),
            shrimp_diag(&control_snap),
        )
    };

    Ok(ProbeResult {
        name: "chloride_protection",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// 5. Crash mode
// ---------------------------------------------------------------------------

/// Population crash from overcrowding, overfeeding, and no water changes.
///
/// **Husbandry story**: A small nano tank (15 L effective) is massively
/// overstocked with 40 shrimp (~2.7 per litre, well above the recommended
/// 0.2/L). The keeper overfeeds (1 g/day in a 15 L tank) and performs zero
/// water changes. Uneaten food decomposes, ammonia and nitrite spike,
/// dissolved oxygen drops as decomposition accelerates, and the biofilter
/// cannot keep up. Within 30 days the colony should crash: the population
/// drops by at least 50 % from the combined effects of ammonia toxicity,
/// nitrite poisoning, low DO, and deteriorating conditions.
#[test]
fn probe_crash_mode() -> Result<(), Box<dyn std::error::Error>> {
    let state = crash_mode_state(SimSeed(6604));

    let mut run = HarnessRun::from_state(SimSeed(6604), "crash_mode", state)
        .with_artifact_label("shrimp_crash_mode");
    run.enable_instrumentation();

    let initial_snap = run.snapshot();
    let initial_count = initial_snap.total_shrimp_count;

    // Overfeed daily for 30 days, no water changes, no maintenance
    for _ in 0..30 {
        run.apply_action(PlayerAction::Feed { grams: 1.0 })?;
        run.step_hours(24)?;
    }

    let final_snap = run.snapshot();
    let final_count = final_snap.total_shrimp_count;
    let mortality_fraction = 1.0 - (final_count as f64 / initial_count.max(1) as f64);

    // At least 50% mortality expected
    run.assert_snapshot("population_crashed", |snap| {
        let frac = 1.0 - (snap.total_shrimp_count as f64 / initial_count.max(1) as f64);
        if frac < 0.50 {
            Err(format!(
                "crash mode should produce >= 50% mortality, got {:.1}% \
                 (initial={initial_count}, final={}). {}",
                frac * 100.0,
                snap.total_shrimp_count,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    // Water quality should be terrible
    run.assert_snapshot("water_quality_degraded", |snap| {
        // At least one of: high ammonia, high nitrite, or low DO
        let bad_ammonia = snap.tan_mg_n_per_l > 1.0;
        let bad_nitrite = snap.nitrite_mg_n_per_l > 1.0;
        let low_do = snap.do_mg_l < 4.0;
        if !(bad_ammonia || bad_nitrite || low_do) {
            Err(format!(
                "crash mode should degrade water quality (TAN > 1, NO2 > 1, or DO < 4). \
                 TAN={:.4}, NO2={:.4}, DO={:.2}. {}",
                snap.tan_mg_n_per_l,
                snap.nitrite_mg_n_per_l,
                snap.do_mg_l,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    // Condition should be poor
    run.assert_snapshot("poor_condition", |snap| {
        if snap.shrimp_condition_index > 0.6 {
            Err(format!(
                "condition should be poor (< 0.6) in crash mode, got {:.3}. {}",
                snap.shrimp_condition_index,
                shrimp_diag(snap),
            ))
        } else {
            Ok(())
        }
    });

    eprintln!(
        "probe_crash_mode: initial={initial_count}, final={final_count}, \
         mortality={:.1}%. {}",
        mortality_fraction * 100.0,
        shrimp_diag(&final_snap),
    );

    run.finish().map_err(|e| e.into())
}

/// Build an overcrowded, neglected nano tank destined for a crash.
fn crash_mode_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
        length_cm: 30.0,
        width_cm: 20.0,
        height_cm: 30.0,
        fill_height_cm: 25.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 30.0 * vol;
    state.water.magnesium_mg_total = 8.0 * vol;
    state.water.alkalinity_meq_total = 4.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 10.0 * vol;
    state.water.dissolved_oxygen_mg_total = 7.0 * vol;
    state.water.bicarbonate_mg_total = 200.0 * vol;

    state.algae.set_periphyton_total(2.0);

    // Partially cycled — can handle some load but will be overwhelmed
    state.microbe.set_decomposer_total(0.15);
    state.microbe.ammonia_oxidizer_biomass_g = 0.3;
    state.microbe.nitrite_oxidizer_biomass_g = 0.2;
    state.microbe.comammox_biomass_g = 0.1;
    state.filter_state.biofilter_maturity_index = 0.5;

    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.4;
    state.hardware.light.photoperiod_hours = 8.0;

    // Massive overcrowding: 40 in ~15 L = ~2.7/L
    state.animal.adult.count = 40;
    state.animal.set_population_condition_index(0.6);
    state.animal.molt_stress_index = 0.2;
    state.animal.reproductive_readiness_index = 0.1;

    state.process_params = ProcessParams::default();
    // Disable spawning — we want to see mortality, not breeding
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

fn run_probe_crash_mode() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = crash_mode_state(SimSeed(6604));
    let mut engine = Engine::from_parts(state, vec![]);
    let initial_count = engine.full_state().animal.total_count();

    for _ in 0..30 {
        engine.apply_action(PlayerAction::Feed { grams: 1.0 })?;
        engine.step_hours(24)?;
    }

    let snap = engine.snapshot();
    let mortality_frac = 1.0 - (snap.total_shrimp_count as f64 / initial_count.max(1) as f64);
    let crashed = mortality_frac >= 0.50;
    let bad_water =
        snap.tan_mg_n_per_l > 1.0 || snap.nitrite_mg_n_per_l > 1.0 || snap.do_mg_l < 4.0;
    let poor_condition = snap.shrimp_condition_index <= 0.6;
    let passed = crashed && bad_water && poor_condition;

    let observed = format!(
        "count {initial_count}->{}, mortality={:.1}%, TAN={:.3}, NO2={:.3}, DO={:.2}, cond={:.3}",
        snap.total_shrimp_count,
        mortality_frac * 100.0,
        snap.tan_mg_n_per_l,
        snap.nitrite_mg_n_per_l,
        snap.do_mg_l,
        snap.shrimp_condition_index,
    );
    let failure_detail = if passed {
        String::new()
    } else {
        let mut issues = Vec::new();
        if !crashed {
            issues.push(format!(
                "mortality only {:.1}% (need >= 50%)",
                mortality_frac * 100.0
            ));
        }
        if !bad_water {
            issues.push(format!(
                "water quality not degraded (TAN={:.3}, NO2={:.3}, DO={:.2})",
                snap.tan_mg_n_per_l, snap.nitrite_mg_n_per_l, snap.do_mg_l,
            ));
        }
        if !poor_condition {
            issues.push(format!(
                "condition not poor ({:.3})",
                snap.shrimp_condition_index
            ));
        }
        format!("{} | {}", issues.join("; "), shrimp_diag(&snap))
    };

    Ok(ProbeResult {
        name: "crash_mode",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// Summary report
// ---------------------------------------------------------------------------

/// Runs all five shrimp scenario probes and produces a structured summary.
///
/// Use `--nocapture` to see the report on stdout. Exits non-zero if any
/// probe violates its envelope.
#[test]
fn all_shrimp_probes_summary() -> Result<(), Box<dyn std::error::Error>> {
    let results = [
        run_probe_successful_breeding()?,
        run_probe_thermal_suppression()?,
        run_probe_chemistry_stress()?,
        run_probe_chloride_protection()?,
        run_probe_crash_mode()?,
    ];

    eprintln!();
    eprintln!("=== Shrimp Scenario Probe Summary Report ===");
    eprintln!();
    let mut all_passed = true;
    for (i, r) in results.iter().enumerate() {
        let status = if r.passed { "PASS" } else { "FAIL" };
        all_passed = all_passed && r.passed;
        eprintln!("  Probe {}: {} [{}]", i + 1, r.name, status);
        eprintln!("    {}", r.observed);
        if !r.passed {
            eprintln!("    >> {}", r.failure_detail);
        }
    }
    let pass_count = results.iter().filter(|r| r.passed).count();
    let fail_count = results.iter().filter(|r| !r.passed).count();
    eprintln!();
    eprintln!("  {pass_count} passed, {fail_count} failed");
    eprintln!();
    eprintln!("=== End Shrimp Probe Report ===");
    eprintln!();

    if !all_passed {
        let mut msg = format!("{fail_count} shrimp probe(s) failed:\n");
        for r in results.iter().filter(|r| !r.passed) {
            msg.push_str(&format!("  {}: {}\n", r.name, r.failure_detail));
        }
        panic!("{msg}");
    }

    Ok(())
}
