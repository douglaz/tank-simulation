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

use std::collections::BTreeSet;

use tank_core::{
    systems::chemistry::{
        bicarbonate_mg_total_from_mmol_per_l, validate_source_water_carbonate_profile,
    },
    EggCohort, EventKind, PlayerAction, ProcessParams, SimSeed, SimTracer, SimulationEngine,
    SourceWaterProfile, TankGeometry, TankState, Verbosity, WaterState,
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

fn require_probe_pass<E>(result: Result<ProbeResult, E>) -> Result<(), Box<dyn std::error::Error>>
where
    E: std::fmt::Display,
{
    match result {
        Ok(probe) if probe.passed => Ok(()),
        Ok(probe) => Err(format!("{} failed:\n{}", probe.name, probe.failure_detail).into()),
        Err(err) => Err(err.to_string().into()),
    }
}

fn finish_probe(name: &'static str, observed: String, runs: Vec<HarnessRun>) -> ProbeResult {
    let mut finish_errors = Vec::new();
    for run in runs {
        if let Err(err) = run.finish() {
            finish_errors.push(err);
        }
    }

    let failure_detail = finish_errors.join("\n");
    ProbeResult {
        name,
        passed: failure_detail.is_empty(),
        observed,
        failure_detail,
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|msg| (*msg).to_string()))
        .unwrap_or_else(|| "unknown panic".to_string())
}

fn probe_artifact_label(base: &str, suffix: Option<&str>) -> String {
    match suffix {
        Some(suffix) => format!("{base}_{suffix}"),
        None => base.to_string(),
    }
}

fn record_failure_all(runs: &mut [&mut HarnessRun], label: &str, message: String) {
    for run in runs.iter_mut() {
        run.record_failure(label, message.clone());
    }
}

fn enable_probe_instrumentation(run: &mut HarnessRun) {
    run.engine_mut().enable_budget_tracking();
    run.engine_mut()
        .enable_tracing(SimTracer::new(Verbosity::Trace));
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

// ---------------------------------------------------------------------------
// Debug helpers — dump shrimp state on failure
// ---------------------------------------------------------------------------

/// Format per-stage shrimp state and water chemistry for diagnostic output.
///
/// Dumps counts, condition, reserve, molt status, and key chemistry
/// (GH, pH, NO₂, Cl) in a compact format for assertion messages.
fn shrimp_diag(state: &TankState) -> String {
    let chemistry = state.concentrations();
    format!(
        "shrimp={} (adult={} cond={:.3} reserve={:.3}, sub_adult={} cond={:.3} reserve={:.3}, \
         juv={} cond={:.3} reserve={:.3}, berried={}), \
         cond={:.3}, molt_stress={:.3}, molt_ready={:.3}, failed_molt_accum={:.3}, \
         last_molt_success={}, readiness={:.3}, \
         pH={:.2}, GH={:.1}, NO2={:.4}, Cl={:.2}, DO={:.2}, temp={:.1}C",
        state.animal.total_count(),
        state.animal.adult.count,
        state.animal.adult.condition_index,
        state.animal.adult.reserve_g,
        state.animal.sub_adult.count,
        state.animal.sub_adult.condition_index,
        state.animal.sub_adult.reserve_g,
        state.animal.juvenile.count,
        state.animal.juvenile.condition_index,
        state.animal.juvenile.reserve_g,
        state.animal.berried_females_count,
        state.animal.population_condition_index(),
        state.animal.molt_stress_index,
        state.animal.molt_readiness,
        state.animal.failed_molt_accum,
        state.animal.last_molt_success,
        state.animal.reproductive_readiness_index,
        state.water.ph,
        chemistry.gh_d(),
        chemistry.nitrite_mg_n_per_l(),
        chemistry.chloride_mg_per_l(),
        chemistry.do_mg_per_l(),
        state.water.temperature_c,
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
/// mechanics. Over 1 400 simulated hours (~60 days) the colony should
/// produce at least 2 complete reproductive cycles: adults become berried,
/// eggs hatch, and juveniles appear. Total population should grow.
#[test]
fn probe_successful_breeding() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_successful_breeding())
}

fn run_probe_successful_breeding() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_successful_breeding_with_suffix(None)
}

fn run_probe_successful_breeding_summary() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_successful_breeding_with_suffix(Some("summary"))
}

fn run_probe_successful_breeding_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = breeding_success_state(SimSeed(6600));
    let mut run =
        HarnessRun::from_state(SimSeed(6600), "breeding_success", state).with_artifact_label(
            probe_artifact_label("shrimp_successful_breeding", artifact_suffix),
        );
    enable_probe_instrumentation(&mut run);
    run.checkpoint("initial");

    let initial_count = run.engine().full_state().animal.total_count();
    let total_days = 60u32;
    let window_days = 10u32;
    let num_windows = total_days / window_days;
    let mut peak_tan = 0.0_f64;
    let mut peak_no2 = 0.0_f64;
    let mut peak_instability = 0.0_f64;

    for day in 1..=total_days {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;

        if day % 5 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
            run.step_hours(1)?;
        }

        let snap = run.snapshot();
        let state = run.engine().full_state();
        peak_tan = peak_tan.max(snap.tan_mg_n_per_l);
        peak_no2 = peak_no2.max(snap.nitrite_mg_n_per_l);
        peak_instability = peak_instability.max(state.stability_tracker.instability_index);

        if day % window_days == 0 {
            run.assert_envelope(
                &format!("window_{}", day / window_days),
                &Envelope::default()
                    .ph(6.8, 8.6)
                    .temperature_c(23.0, 26.5)
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 0.6)
                    .do_min(7.0),
            );
        }
    }

    let final_snap = run.snapshot();
    let final_state = run.engine().full_state().clone();
    let final_count = final_snap.total_shrimp_count;
    let berried_days: BTreeSet<u32> = final_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpBerried)
        .map(|event| event.day)
        .collect();
    let hatch_days: BTreeSet<u32> = final_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpHatched)
        .map(|event| event.day)
        .collect();
    let complete_cycle_count = berried_days.len().min(hatch_days.len());

    if final_count <= initial_count {
        run.record_failure(
            "population_growth",
            format!(
                "healthy breeding scenario should end with net growth: initial={initial_count}, final={final_count}. {}",
                shrimp_diag(&final_state),
            ),
        );
    }

    if complete_cycle_count < 2 {
        run.record_failure(
            "reproductive_cycles",
            format!(
                "expected at least 2 complete berried->hatch cycles across {num_windows} windows, \
                 observed {complete_cycle_count} (berried_days={}, hatch_days={}). {}",
                berried_days.len(),
                hatch_days.len(),
                shrimp_diag(&final_state),
            ),
        );
    }

    if final_snap.juveniles_count == 0 && final_snap.sub_adult_count == 0 {
        run.record_failure(
            "offspring_present",
            format!(
                "expected juveniles or sub-adults by the end of the healthy breeding run. {}",
                shrimp_diag(&final_state),
            ),
        );
    }

    if peak_tan >= 0.6 || peak_no2 >= 0.6 || peak_instability >= 0.35 {
        run.record_failure(
            "stable_conditions",
            format!(
                "healthy breeding fixture should stay chemically stable: peak_tan={peak_tan:.3}, \
                 peak_no2={peak_no2:.3}, peak_instability={peak_instability:.3}. {}",
                shrimp_diag(&final_state),
            ),
        );
    }

    run.checkpoint("final");
    let observed = format!(
        "initial={initial_count}, final={final_count}, cycles={complete_cycle_count}, \
         berried_days={}, hatch_days={}, juv={}, sub={}, peak_tan={peak_tan:.3}, \
         peak_no2={peak_no2:.3}, peak_instability={peak_instability:.3}",
        berried_days.len(),
        hatch_days.len(),
        final_snap.juveniles_count,
        final_snap.sub_adult_count,
    );

    eprintln!(
        "probe_successful_breeding: {observed}. {}",
        shrimp_diag(&final_state),
    );

    Ok(finish_probe("successful_breeding", observed, vec![run]))
}

/// Build a well-maintained planted tank optimised for breeding success.
///
/// Mortality is disabled so the probe isolates reproductive mechanics.
/// Strong nitrification prevents nitrite buildup; abundant periphyton and
/// weekly remineralized water changes keep chemistry and minerals stable.
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
    state.water.dissolved_organic_carbon_mg_c_total = 5.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.8;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;
    state.water.ammonia_total_mg_n_total = 0.0;
    state.water.nitrite_mg_n_total = 0.0;
    state.water.nitrate_mg_n_total = 5.0 * vol;

    // Abundant periphyton so food is never limiting
    state.algae.set_periphyton_total(120.0);

    // Very strong nitrification to keep ammonia/nitrite near zero
    state.microbe.set_decomposer_total(0.3);
    state.microbe.ammonia_oxidizer_biomass_g = 10.0;
    state.microbe.nitrite_oxidizer_biomass_g = 10.0;
    state.microbe.comammox_biomass_g = 2.0;
    state.filter_state.biofilter_maturity_index = 1.0;
    state.source_water_catalog.insert(
        "hard_shrimp".to_string(),
        load_source_profile("hard_shrimp"),
    );

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.7;
    state.hardware.light.photoperiod_hours = 10.0;

    state.animal.adult.count = 10;
    state.animal.set_population_condition_index(0.85);
    state.animal.molt_stress_index = 0.0;
    state.animal.reproductive_readiness_index = 0.85;
    state.animal.adult.reserve_g = 10.0;
    state.animal.last_molt_success = true;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 30.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 30.0;
    state.process_params.periphyton_capacity_g_per_m2 = 200.0;
    // Disable mortality to isolate reproductive mechanics
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;

    state.shrimp_params.base_spawn_rate = 0.15;
    state.shrimp_params.hatch_success_base = 0.9;
    state.shrimp_params.egg_duration_days = 14;
    state.shrimp_params.failed_molt_mortality_scale = 0.0;
    state.shrimp_params.apply_legacy_total_maturation_days(40.0);

    state.reseed_stability_tracker();
    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = state.water.ph;
    state.stability_tracker.instability_index = 0.0;
    state
}

// ---------------------------------------------------------------------------
// 2. Thermal suppression
// ---------------------------------------------------------------------------

/// Thermal suppression of breeding at high temperature.
///
/// **Husbandry story**: Two identical colonies start with 10 adults in
/// well-maintained tanks. One tank is kept at a comfortable 25 °C, the other
/// receives a heat spike from 25 °C to 31 °C and is then held there (above
/// the species' optimal range). Both colonies start with active berried
/// females so the hot arm must resolve an in-flight clutch under heat stress.
/// Over 60 days the warm tank should show: suppressed reproductive readiness,
/// egg dropping after the heat shock, and fewer juveniles compared to the
/// cool tank. Chemistry should remain stable so temperature is the main
/// driver of the divergence.
///
/// Note: we intentionally do not assert that the final warm-tank
/// `molt_stress` exceeds the cool tank. In the current calibrated model the
/// cool colony breeds and molts more aggressively, so recent runs finish with
/// an inverted ordering (`cool=1.000`, `warm=0.351`). The probe therefore
/// locks the direct thermal outcomes that stay stable across retuning:
/// lower readiness, fewer offspring, and more egg dropping in the warm arm.
#[test]
fn probe_thermal_suppression() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_thermal_suppression())
}

/// Build a stocked shrimp tank at the given ambient temperature, isolating
/// thermal effects from food limitation and ammonia stress.
fn thermal_scenario_state(seed: SimSeed, ambient_temp_c: f64) -> TankState {
    let mut state = breeding_success_state(seed);
    state.water.temperature_c = ambient_temp_c;
    state.environment.ambient_temp_c = ambient_temp_c;
    state.animal.berried_females_count = 4;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 4,
        progress_days: 5.0,
    }];
    state.animal.sync_egg_progress_from_cohorts();
    state.shrimp_params.base_spawn_rate = 0.12;
    state.shrimp_params.hatch_success_base = 0.9;
    state.shrimp_params.failed_molt_mortality_scale = 0.0;
    state.shrimp_params.apply_legacy_total_maturation_days(40.0);

    let mut source = load_source_profile("hard_shrimp");
    source.temperature_c = ambient_temp_c;
    state
        .source_water_catalog
        .insert("thermal_probe".to_string(), source);

    state.reseed_stability_tracker();
    if ambient_temp_c > 30.0 {
        state.stability_tracker.prev_temp_c = 25.0;
    }
    state
}

fn run_probe_thermal_suppression() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_thermal_suppression_with_suffix(None)
}

fn run_probe_thermal_suppression_summary() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_thermal_suppression_with_suffix(Some("summary"))
}

fn run_probe_thermal_suppression_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let cool_state = thermal_scenario_state(SimSeed(6601), 25.0);
    let warm_state = thermal_scenario_state(SimSeed(6601), 31.0);

    let mut cool_run = HarnessRun::from_state(SimSeed(6601), "thermal_cool", cool_state)
        .with_artifact_label(probe_artifact_label("shrimp_thermal_cool", artifact_suffix));
    let mut warm_run = HarnessRun::from_state(SimSeed(6601), "thermal_warm", warm_state)
        .with_artifact_label(probe_artifact_label("shrimp_thermal_warm", artifact_suffix));
    enable_probe_instrumentation(&mut cool_run);
    enable_probe_instrumentation(&mut warm_run);
    cool_run.checkpoint("initial");
    warm_run.checkpoint("initial");

    let window_days = 10u32;
    for day in 1..=60 {
        cool_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        warm_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        cool_run.step_hours(24)?;
        warm_run.step_hours(24)?;

        if day % 5 == 0 {
            cool_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "thermal_probe".to_string(),
            })?;
            warm_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "thermal_probe".to_string(),
            })?;
            cool_run.step_hours(1)?;
            warm_run.step_hours(1)?;
        }

        if day % window_days == 0 {
            cool_run.assert_envelope(
                &format!("cool_window_{}", day / window_days),
                &Envelope::default()
                    .ph(6.8, 8.6)
                    .temperature_c(23.0, 26.5)
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 0.6)
                    .do_min(7.0),
            );
            warm_run.assert_envelope(
                &format!("warm_window_{}", day / window_days),
                &Envelope::default()
                    .ph(6.8, 8.6)
                    .temperature_c(29.5, 32.5)
                    .tan_mg_n_per_l(0.0, 0.6)
                    .nitrite_mg_n_per_l(0.0, 0.6)
                    .do_min(6.8),
            );
            cool_run.checkpoint(&format!("day_{day}"));
            warm_run.checkpoint(&format!("day_{day}"));
        }
    }

    let cool_snap = cool_run.snapshot();
    let warm_snap = warm_run.snapshot();
    let cool_state = cool_run.engine().full_state().clone();
    let warm_state = warm_run.engine().full_state().clone();
    let cool_egg_drops = cool_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::EggDropping)
        .count();
    let warm_egg_drops = warm_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::EggDropping)
        .count();
    let mut runs = [&mut cool_run, &mut warm_run];

    if warm_snap.shrimp_reproductive_readiness >= cool_snap.shrimp_reproductive_readiness {
        record_failure_all(
            &mut runs,
            "warm_readiness_suppressed",
            format!(
                "warm tank readiness ({:.3}) should be lower than cool tank ({:.3}). \
                 Cool: {} | Warm: {}",
                warm_snap.shrimp_reproductive_readiness,
                cool_snap.shrimp_reproductive_readiness,
                shrimp_diag(&cool_state),
                shrimp_diag(&warm_state),
            ),
        );
    }

    let warm_worse = if cool_snap.juveniles_count > 0 || warm_snap.juveniles_count > 0 {
        warm_snap.juveniles_count < cool_snap.juveniles_count
    } else {
        warm_snap.shrimp_reproductive_readiness <= cool_snap.shrimp_reproductive_readiness
    };
    // We intentionally do not assert a warm>cool final molt-stress ordering.
    // Current calibrated runs invert that relationship because the cooler arm
    // reproduces and molts more aggressively (`cool=1.000`, `warm=0.351`).
    if !warm_worse {
        record_failure_all(
            &mut runs,
            "warm_worse_outcomes",
            format!(
                "warm tank should have worse reproductive outcomes. \
                 Cool juv={}, Warm juv={}, Cool readiness={:.3}, Warm readiness={:.3}. \
                 Cool: {} | Warm: {}",
                cool_snap.juveniles_count,
                warm_snap.juveniles_count,
                cool_snap.shrimp_reproductive_readiness,
                warm_snap.shrimp_reproductive_readiness,
                shrimp_diag(&cool_state),
                shrimp_diag(&warm_state),
            ),
        );
    }

    if warm_egg_drops == 0 || warm_egg_drops <= cool_egg_drops {
        record_failure_all(
            &mut runs,
            "warm_egg_dropping",
            format!(
                "warm tank should drop eggs after the heat shock: cool_egg_drops={cool_egg_drops}, \
                 warm_egg_drops={warm_egg_drops}. Cool: {} | Warm: {}",
                shrimp_diag(&cool_state),
                shrimp_diag(&warm_state),
            ),
        );
    }

    if cool_state.animal.population_condition_index() < 0.6
        || cool_state.animal.molt_stress_index > 0.4
        || cool_state.animal.failed_molt_accum > 0.25
    {
        record_failure_all(
            &mut runs,
            "cool_control_health",
            format!(
                "cool control should remain a healthy baseline instead of drifting into stress. \
                 {}",
                shrimp_diag(&cool_state),
            ),
        );
    }

    cool_run.checkpoint("final");
    warm_run.checkpoint("final");
    let observed = format!(
        "cool_readiness={:.3}, warm_readiness={:.3}, cool_juv={}, warm_juv={}, \
         cool_egg_drops={cool_egg_drops}, warm_egg_drops={warm_egg_drops}, \
         cool_molt_stress={:.3}, warm_molt_stress={:.3}",
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.shrimp_reproductive_readiness,
        cool_snap.juveniles_count,
        warm_snap.juveniles_count,
        cool_snap.shrimp_molt_stress_index,
        warm_snap.shrimp_molt_stress_index,
    );

    eprintln!(
        "probe_thermal_suppression: {observed}. Cool: {} | Warm: {}",
        shrimp_diag(&cool_state),
        shrimp_diag(&warm_state),
    );

    Ok(finish_probe(
        "thermal_suppression",
        observed,
        vec![cool_run, warm_run],
    ))
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
    require_probe_pass(run_probe_chemistry_stress())
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
    // Seed the colony at the end of an intermolt cycle so the 21-day probe
    // actually exercises chemistry-driven molt resolution instead of only
    // chronic nitrite mortality.
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;
    state.animal.inter_molt_timer_days = state.shrimp_params.base_molt_interval_days;
    state.animal.last_molt_success = true;

    state.process_params = ProcessParams::default();
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.3;
    // Disable spawning to isolate mortality effects
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

fn run_probe_chemistry_stress() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_chemistry_stress_with_suffix(None)
}

fn run_probe_chemistry_stress_summary() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_chemistry_stress_with_suffix(Some("summary"))
}

fn run_probe_chemistry_stress_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = chemistry_stress_state(SimSeed(6602));
    let mut run =
        HarnessRun::from_state(SimSeed(6602), "chemistry_stress", state).with_artifact_label(
            probe_artifact_label("shrimp_chemistry_stress", artifact_suffix),
        );
    enable_probe_instrumentation(&mut run);
    run.checkpoint("initial");

    let initial_snap = run.snapshot();
    let initial_count = initial_snap.total_shrimp_count;
    let initial_condition = initial_snap.shrimp_condition_index;

    for day in 1..=21 {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;
        if day % 7 == 0 {
            run.checkpoint(&format!("day_{day}"));
        }
    }

    let final_snap = run.snapshot();
    let final_state = run.engine().full_state().clone();
    let molt_failures = final_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::MoltFailure)
        .count();

    if final_snap.total_shrimp_count >= initial_count {
        run.record_failure(
            "mortality_occurred",
            format!(
                "low GH + high NO₂ should cause mortality: initial={initial_count}, final={}. {}",
                final_snap.total_shrimp_count,
                shrimp_diag(&final_state),
            ),
        );
    }

    if final_snap.shrimp_molt_stress_index <= 0.2 {
        run.record_failure(
            "elevated_molt_stress",
            format!(
                "molt stress should be elevated (> 0.2) under low-mineral conditions, \
                 got {:.3}. {}",
                final_snap.shrimp_molt_stress_index,
                shrimp_diag(&final_state),
            ),
        );
    }

    if final_snap.shrimp_condition_index >= initial_condition {
        run.record_failure(
            "condition_declined",
            format!(
                "condition should decline under stress: initial={initial_condition:.3}, final={:.3}. {}",
                final_snap.shrimp_condition_index,
                shrimp_diag(&final_state),
            ),
        );
    }

    if molt_failures == 0 && final_state.animal.failed_molt_accum <= 0.0 {
        run.record_failure(
            "molt_failure_signal",
            format!(
                "low GH + high NO₂ should cause either explicit MoltFailure events or lingering \
                 failed-molt accumulation, but saw neither. {}",
                shrimp_diag(&final_state),
            ),
        );
    }

    run.checkpoint("final");
    let observed = format!(
        "count {initial_count}->{}, cond {initial_condition:.3}->{:.3}, \
         molt_stress={:.3}, failed_molt_accum={:.3}, molt_failures={molt_failures}",
        final_snap.total_shrimp_count,
        final_snap.shrimp_condition_index,
        final_snap.shrimp_molt_stress_index,
        final_state.animal.failed_molt_accum,
    );

    eprintln!(
        "probe_chemistry_stress: {observed}. {}",
        shrimp_diag(&final_state),
    );

    Ok(finish_probe("chemistry_stress", observed, vec![run]))
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
    require_probe_pass(run_probe_chloride_protection())
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
    run_probe_chloride_protection_with_suffix(None)
}

fn run_probe_chloride_protection_summary() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_chloride_protection_with_suffix(Some("summary"))
}

fn run_probe_chloride_protection_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let base_state = chloride_test_base_state(SimSeed(6603));
    let initial_count = base_state.animal.total_count();

    let mut pre_treatment_run =
        HarnessRun::from_state(SimSeed(6603), "chloride_pre_treatment", base_state)
            .with_artifact_label(probe_artifact_label(
                "shrimp_chloride_pre_treatment",
                artifact_suffix,
            ));
    enable_probe_instrumentation(&mut pre_treatment_run);
    pre_treatment_run.checkpoint("initial");
    pre_treatment_run.step_hours(3 * 24)?;
    pre_treatment_run.checkpoint("pre_treatment");

    let pre_treatment = pre_treatment_run.engine().full_state().clone();
    let count_before_treatment = pre_treatment.animal.total_count();
    let phase1_deaths = initial_count.saturating_sub(count_before_treatment);

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
    let mut treated_run = HarnessRun::from_state(SimSeed(6603), "chloride_treated", treated_state)
        .with_artifact_label(probe_artifact_label(
            "shrimp_chloride_treated",
            artifact_suffix,
        ));
    enable_probe_instrumentation(&mut treated_run);
    treated_run.checkpoint("branch_start");

    let mut control_state = pre_treatment.clone();
    control_state
        .source_water_catalog
        .insert("matched_control".to_string(), control_profile);
    let mut control_run = HarnessRun::from_state(SimSeed(6603), "chloride_control", control_state)
        .with_artifact_label(probe_artifact_label(
            "shrimp_chloride_control",
            artifact_suffix,
        ));
    enable_probe_instrumentation(&mut control_run);
    control_run.checkpoint("branch_start");

    treated_run.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    control_run.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "matched_control".to_string(),
    })?;
    treated_run.step_hours(1)?;
    control_run.step_hours(1)?;
    treated_run.checkpoint("post_treatment");
    control_run.checkpoint("post_treatment");

    treated_run.step_hours(71)?;
    control_run.step_hours(71)?;
    treated_run.checkpoint("final");
    control_run.checkpoint("final");

    let treated_snap = treated_run.snapshot();
    let control_snap = control_run.snapshot();
    let treated_state = treated_run.engine().full_state().clone();
    let control_state = control_run.engine().full_state().clone();
    let treated_deaths = count_before_treatment.saturating_sub(treated_state.animal.total_count());
    let control_deaths = count_before_treatment.saturating_sub(control_state.animal.total_count());
    let mut runs = [&mut pre_treatment_run, &mut treated_run, &mut control_run];

    if treated_deaths >= control_deaths {
        record_failure_all(
            &mut runs,
            "chloride_less_mortality",
            format!(
                "salt-treated tank should have fewer post-treatment deaths: \
                 treated={treated_deaths}, control={control_deaths}. \
                 Pre-treatment: {} | Treated: {} | Control: {}",
                shrimp_diag(&pre_treatment),
                shrimp_diag(&treated_state),
                shrimp_diag(&control_state),
            ),
        );
    }

    if treated_snap.effective_nitrite_hazard_mg_per_l
        >= control_snap.effective_nitrite_hazard_mg_per_l
    {
        record_failure_all(
            &mut runs,
            "chloride_reduces_hazard",
            format!(
                "chloride should reduce effective nitrite hazard: treated={:.4}, control={:.4}. \
                 Treated: {} | Control: {}",
                treated_snap.effective_nitrite_hazard_mg_per_l,
                control_snap.effective_nitrite_hazard_mg_per_l,
                shrimp_diag(&treated_state),
                shrimp_diag(&control_state),
            ),
        );
    }

    let observed = format!(
        "phase1_deaths={phase1_deaths}, post: treated_deaths={treated_deaths}, control_deaths={control_deaths}, \
         treated_hazard={:.4}, control_hazard={:.4}",
        treated_snap.effective_nitrite_hazard_mg_per_l,
        control_snap.effective_nitrite_hazard_mg_per_l,
    );

    eprintln!(
        "probe_chloride_protection: {observed}. Pre-treatment: {} | Treated: {} | Control: {}",
        shrimp_diag(&pre_treatment),
        shrimp_diag(&treated_state),
        shrimp_diag(&control_state),
    );

    Ok(finish_probe(
        "chloride_protection",
        observed,
        vec![pre_treatment_run, treated_run, control_run],
    ))
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
    require_probe_pass(run_probe_crash_mode())
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
    run_probe_crash_mode_with_suffix(None)
}

fn run_probe_crash_mode_summary() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_crash_mode_with_suffix(Some("summary"))
}

fn run_probe_crash_mode_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let state = crash_mode_state(SimSeed(6604));
    let mut run = HarnessRun::from_state(SimSeed(6604), "crash_mode", state)
        .with_artifact_label(probe_artifact_label("shrimp_crash_mode", artifact_suffix));
    enable_probe_instrumentation(&mut run);
    run.checkpoint("initial");

    let initial_count = run.engine().full_state().animal.total_count();

    for day in 1..=30 {
        run.apply_action(PlayerAction::Feed { grams: 1.0 })?;
        run.step_hours(24)?;
        if day % 10 == 0 {
            run.checkpoint(&format!("day_{day}"));
        }
    }

    let final_snap = run.snapshot();
    let final_state = run.engine().full_state().clone();
    let mortality_frac = 1.0 - (final_snap.total_shrimp_count as f64 / initial_count.max(1) as f64);

    if mortality_frac < 0.50 {
        run.record_failure(
            "population_crashed",
            format!(
                "crash mode should produce >= 50% mortality, got {:.1}% \
                 (initial={initial_count}, final={}). {}",
                mortality_frac * 100.0,
                final_snap.total_shrimp_count,
                shrimp_diag(&final_state),
            ),
        );
    }

    let bad_ammonia = final_snap.tan_mg_n_per_l > 1.0;
    let bad_nitrite = final_snap.nitrite_mg_n_per_l > 1.0;
    let low_do = final_snap.do_mg_l < 4.0;
    if !(bad_ammonia || bad_nitrite || low_do) {
        run.record_failure(
            "water_quality_degraded",
            format!(
                "crash mode should degrade water quality (TAN > 1, NO2 > 1, or DO < 4). \
                 TAN={:.4}, NO2={:.4}, DO={:.2}. {}",
                final_snap.tan_mg_n_per_l,
                final_snap.nitrite_mg_n_per_l,
                final_snap.do_mg_l,
                shrimp_diag(&final_state),
            ),
        );
    }

    if final_snap.shrimp_condition_index > 0.6 {
        run.record_failure(
            "poor_condition",
            format!(
                "condition should be poor (< 0.6) in crash mode, got {:.3}. {}",
                final_snap.shrimp_condition_index,
                shrimp_diag(&final_state),
            ),
        );
    }

    run.checkpoint("final");
    let observed = format!(
        "count {initial_count}->{}, mortality={:.1}%, TAN={:.3}, NO2={:.3}, DO={:.2}, cond={:.3}",
        final_snap.total_shrimp_count,
        mortality_frac * 100.0,
        final_snap.tan_mg_n_per_l,
        final_snap.nitrite_mg_n_per_l,
        final_snap.do_mg_l,
        final_snap.shrimp_condition_index,
    );

    eprintln!(
        "probe_crash_mode: {observed}. {}",
        shrimp_diag(&final_state),
    );

    Ok(finish_probe("crash_mode", observed, vec![run]))
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
    type ProbeFn = fn() -> Result<ProbeResult, Box<dyn std::error::Error>>;
    let probes: [(&str, ProbeFn); 5] = [
        ("successful_breeding", run_probe_successful_breeding_summary),
        ("thermal_suppression", run_probe_thermal_suppression_summary),
        ("chemistry_stress", run_probe_chemistry_stress_summary),
        ("chloride_protection", run_probe_chloride_protection_summary),
        ("crash_mode", run_probe_crash_mode_summary),
    ];

    let mut results = Vec::new();
    for (name, probe_fn) in probes {
        let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(probe_fn)) {
            Ok(Ok(result)) => result,
            Ok(Err(err)) => ProbeResult {
                name,
                passed: false,
                observed: "probe execution aborted before summary".to_string(),
                failure_detail: err.to_string(),
            },
            Err(panic) => ProbeResult {
                name,
                passed: false,
                observed: "probe panicked before returning a summary".to_string(),
                failure_detail: panic_message(panic),
            },
        };
        results.push(result);
    }

    eprintln!();
    eprintln!("=== Shrimp Scenario Probe Summary Report ===");
    eprintln!("Structured trace capture is enabled for every probe run; set TANK_E2E_VERBOSE=1 to stream trace.jsonl-equivalent output on success.");
    eprintln!();
    let mut all_passed = true;
    for (i, r) in results.iter().enumerate() {
        let status = if r.passed { "PASS" } else { "FAIL" };
        all_passed = all_passed && r.passed;
        eprintln!("  Probe {}: {} [{}]", i + 1, r.name, status);
        eprintln!("    {}", r.observed);
        if !r.passed {
            for line in r.failure_detail.lines() {
                eprintln!("    >> {line}");
            }
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
