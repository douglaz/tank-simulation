//! Literature-backed validation suite (tanksim-6e5.7.3).
//!
//! Eight scenario probes verifying qualitative outcomes grounded in aquarium
//! science literature or established chemistry/biology principles.  Each probe
//! tells a causal story and checks directional plausibility, not exact numbers.
//!
//! See `docs/validation_scenarios.md` for full rationale, literature citations,
//! confidence ratings, and envelope bounds.
//!
//! Run all probes:   `cargo test --test e2e_validation_suite`
//! Verbose traces:   `TANK_E2E_VERBOSE=1 cargo test --test e2e_validation_suite -- --nocapture`
//! Summary only:     `cargo test --test e2e_validation_suite validation_suite_summary -- --nocapture`
//!
//! Exit codes: 0 = all pass, non-zero = envelope violation.

use std::error::Error;

use tank_core::{
    systems::chemistry::resolve_carbonate_state, systems::light::is_light_on, Engine, EventKind,
    JsonLinesSink, PlantGuild, PlayerAction, ProcessParams, SimSeed, SimTracer, SimulationEngine,
    SourceWaterProfile, TankGeometry, TankSnapshot, TankState, TraceSink, Verbosity, WaterState,
};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

use crate::calibration::{
    classify_field_f64, CalibrationReport, CalibrationRun, CheckStatus, CheckpointRow, FieldCheck,
    ProvenanceStatus, ScenarioArtifact, ScenarioRow, ValidationConfidence,
};
use crate::{Envelope, HarnessRun};

/// Static metadata for a shipped validation scenario.
#[derive(Debug, Clone, Copy)]
pub struct ValidationScenarioDefinition {
    pub id: &'static str,
    pub title: &'static str,
    pub domain: &'static str,
    pub confidence: ValidationConfidence,
    pub provenance_status: ProvenanceStatus,
    pub seed: SimSeed,
}

const VALIDATION_SCENARIOS: [ValidationScenarioDefinition; 8] = [
    ValidationScenarioDefinition {
        id: "vs01_cycling_timeline",
        title: "Fishless cycling timeline",
        domain: "Chemistry / Microbiology",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7301),
    },
    ValidationScenarioDefinition {
        id: "vs02_aeration_effects",
        title: "Aeration effects (DO + pH)",
        domain: "Physics / Chemistry",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7302),
    },
    ValidationScenarioDefinition {
        id: "vs03_day_night_ph_swing",
        title: "Day/night pH swing",
        domain: "Chemistry / Plant biology",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7303),
    },
    ValidationScenarioDefinition {
        id: "vs04_source_water_differentiation",
        title: "Source-water differentiation",
        domain: "Chemistry",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7304),
    },
    ValidationScenarioDefinition {
        id: "vs05_shrimp_breeding_thermal_window",
        title: "Shrimp breeding thermal window",
        domain: "Animal behavior",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7305),
    },
    ValidationScenarioDefinition {
        id: "vs06_algae_plant_competition",
        title: "Algae-plant competition",
        domain: "Ecology",
        confidence: ValidationConfidence::Medium,
        provenance_status: ProvenanceStatus::StillHeuristic,
        seed: SimSeed(7306),
    },
    ValidationScenarioDefinition {
        id: "vs07_nitrate_removal_denitrification",
        title: "Nitrate removal (denitrification)",
        domain: "Microbiology / Chemistry",
        confidence: ValidationConfidence::Medium,
        provenance_status: ProvenanceStatus::StillHeuristic,
        seed: SimSeed(7307),
    },
    ValidationScenarioDefinition {
        id: "vs08_stocking_density_crash",
        title: "Stocking density crash",
        domain: "Toxicology / Husbandry",
        confidence: ValidationConfidence::High,
        provenance_status: ProvenanceStatus::ValidatedDirectionally,
        seed: SimSeed(7308),
    },
];

pub fn validation_scenarios() -> &'static [ValidationScenarioDefinition] {
    &VALIDATION_SCENARIOS
}

fn scenario_definition(id: &str) -> &'static ValidationScenarioDefinition {
    validation_scenarios()
        .iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("unknown validation scenario id '{id}'"))
}

// ---------------------------------------------------------------------------
// Probe result tracking
// ---------------------------------------------------------------------------

struct ProbeResult {
    name: &'static str,
    passed: bool,
    observed: String,
    failure_detail: String,
}

fn is_verbose() -> bool {
    std::env::var("TANK_E2E_VERBOSE").is_ok_and(|v| v == "1")
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

fn dump_trace_artifact(label: &str, engine: &Engine) -> Option<std::path::PathBuf> {
    let tracer = engine.tracer()?;
    let dir = std::env::temp_dir()
        .join("tank_harness")
        .join("validation_suite");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{label}.jsonl"));
    let mut buf = Vec::new();
    {
        let mut sink = JsonLinesSink::new(&mut buf);
        for tick in tracer.ticks() {
            let _ = sink.emit_tick(tick);
        }
    }
    let _ = std::fs::write(&path, &buf);
    Some(path)
}

fn enable_instrumentation(run: &mut HarnessRun) {
    run.engine_mut().enable_budget_tracking();
    let verbosity = if is_verbose() {
        Verbosity::Trace
    } else {
        Verbosity::Detail
    };
    run.engine_mut().enable_tracing(SimTracer::new(verbosity));
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

fn require_probe_pass<E>(result: Result<ProbeResult, E>) -> Result<(), Box<dyn Error>>
where
    E: std::fmt::Display,
{
    match result {
        Ok(probe) if probe.passed => Ok(()),
        Ok(probe) => Err(format!("{} failed:\n{}", probe.name, probe.failure_detail).into()),
        Err(err) => Err(err.to_string().into()),
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|msg| (*msg).to_string()))
        .unwrap_or_else(|| "unknown panic".to_string())
}

fn annotate_row(
    definition: &ValidationScenarioDefinition,
    mut row: ScenarioRow,
    observed_summary: impl Into<String>,
) -> ScenarioRow {
    row.scenario_id = definition.id.to_string();
    row.scenario_name = definition.title.to_string();
    row.domain = definition.domain.to_string();
    row.confidence = definition.confidence;
    row.provenance_status = definition.provenance_status;
    row.observed_summary = observed_summary.into();
    if row.artifacts.is_empty() {
        if let Some(path) = row.artifact_path.clone() {
            row.artifacts.push(ScenarioArtifact {
                label: "scenario".to_string(),
                path,
            });
        }
    } else if row.artifact_path.is_none() {
        row.artifact_path = row.artifacts.first().map(|artifact| artifact.path.clone());
    }
    row
}

fn fail_field(field: impl Into<String>, detail: impl Into<String>) -> FieldCheck {
    FieldCheck {
        field: field.into(),
        observed: None,
        envelope_min: None,
        envelope_max: None,
        status: CheckStatus::Fail,
        detail: Some(detail.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_custom_row(
    definition: &ValidationScenarioDefinition,
    parameter_variant: &str,
    label: &str,
    day: u32,
    hour: u8,
    fields: Vec<FieldCheck>,
    observed_summary: String,
    mut runs: Vec<(&str, HarnessRun)>,
) -> ScenarioRow {
    let mut checkpoint = CheckpointRow::from_fields(label, day, hour, fields, None);
    let failing_details: Vec<String> = checkpoint
        .fields
        .iter()
        .filter(|field| field.status == CheckStatus::Fail)
        .filter_map(|field| field.detail.clone())
        .collect();

    let mut artifacts = Vec::new();
    let persist_artifacts = checkpoint.status != CheckStatus::Pass;
    for (artifact_label, run) in &mut runs {
        if !failing_details.is_empty() {
            for detail in &failing_details {
                run.record_failure(label, detail.clone());
            }
        }
        if persist_artifacts {
            let path = run.persist_artifacts();
            artifacts.push(ScenarioArtifact {
                label: (*artifact_label).to_string(),
                path: path.display().to_string(),
            });
        }
    }

    checkpoint.artifact_path = artifacts.first().map(|artifact| artifact.path.clone());

    for (_, run) in runs {
        let _ = run.finish();
    }

    ScenarioRow::from_checkpoints(
        definition.id,
        definition.title,
        definition.seed.0,
        parameter_variant,
        definition.domain,
        definition.confidence,
        definition.provenance_status,
        vec![checkpoint],
        observed_summary,
        artifacts,
    )
}

// ---------------------------------------------------------------------------
// VS-01: Fishless cycling timeline
// ---------------------------------------------------------------------------

/// Fishless cycling in a well-buffered tank completes in 4-8 weeks.
///
/// **Literature**: Hovanec & DeLong 1996; Timmons & Ebeling 2010.
///
/// A medium planted tank (hard shrimp water, KH ~8) receives daily ammonia
/// dosing for 8 weeks. The classic N succession should produce: TAN peak
/// (weeks 1-3), nitrite peak (weeks 2-5), clearing of both by weeks 6-8,
/// and steady NO₃⁻ accumulation.
pub fn vs01_cycling_timeline() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs01_cycling_timeline())
}

fn run_vs01_cycling_timeline() -> Result<ProbeResult, Box<dyn Error>> {
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlantedWithCoarsePorous),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    let mut run = HarnessRun::with_overrides(SimSeed(7301), "medium_planted", overrides)?
        .with_artifact_label("vs01_cycling_timeline");
    enable_instrumentation(&mut run);

    // 8 weeks of fishless cycling with daily feed as ammonia source.
    for day in 1..=56 {
        run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        run.step_hours(24)?;

        // Weekly water changes to replenish alkalinity (hard shrimp water).
        if day % 7 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
            run.step_hours(1)?;

            let week = day / 7;
            run.checkpoint(&format!("week_{week}"));

            match week {
                2 => {
                    // In the model, nitrification begins quickly in a well-
                    // buffered tank with active substrate. TAN may already be
                    // low if the biofilter seeds fast; NO₃ can already be
                    // accumulating from the substrate nutrient charge.
                    // Biofilter maturity index tracks a slow ramp (60-day
                    // denitrifier-like scale) so it stays low even when
                    // nitrification is active.
                    run.assert_envelope(
                        "week_2",
                        &Envelope::default()
                            .ph(6.5, 8.5)
                            .tan_mg_n_per_l(0.0, 15.0)
                            .nitrite_mg_n_per_l(0.0, 5.0)
                            .nitrate_mg_n_per_l(0.0, 15.0)
                            .do_min(6.0),
                    );
                }
                4 => {
                    // TAN declining, NO₃ building steadily.
                    run.assert_envelope(
                        "week_4",
                        &Envelope::default()
                            .ph(6.5, 8.5)
                            .tan_mg_n_per_l(0.0, 10.0)
                            .nitrite_mg_n_per_l(0.0, 10.0)
                            .nitrate_mg_n_per_l(0.5, 25.0)
                            .do_min(5.5),
                    );
                }
                6 => {
                    // Both TAN and NO₂ should be clearing.
                    run.assert_envelope(
                        "week_6",
                        &Envelope::default()
                            .ph(6.5, 8.5)
                            .tan_mg_n_per_l(0.0, 3.0)
                            .nitrite_mg_n_per_l(0.0, 3.0)
                            .nitrate_mg_n_per_l(2.0, 40.0)
                            .do_min(5.5),
                    );
                }
                8 => {
                    // Cycle complete: TAN and NO₂ near zero. The model's
                    // biofilter maturity index tracks a slow ramp, so we
                    // check nitrification output (low TAN, high NO₃) rather
                    // than the maturity index itself.
                    run.assert_envelope(
                        "week_8",
                        &Envelope::default()
                            .ph(6.5, 8.5)
                            .tan_mg_n_per_l(0.0, 2.0)
                            .nitrite_mg_n_per_l(0.0, 2.0)
                            .nitrate_mg_n_per_l(5.0, 60.0)
                            .do_min(5.5),
                    );
                }
                _ => {}
            }
        }
    }

    let snap = run.snapshot();
    let observed = format!(
        "week8: TAN={:.3} NO2={:.3} NO3={:.3} pH={:.3} maturity={:.3}",
        snap.tan_mg_n_per_l,
        snap.nitrite_mg_n_per_l,
        snap.nitrate_mg_n_per_l,
        snap.ph,
        snap.biofilter_maturity_index,
    );
    eprintln!("vs01_cycling_timeline: {observed}");

    Ok(finish_probe("VS-01 Cycling timeline", observed, vec![run]))
}

// ---------------------------------------------------------------------------
// VS-02: Aeration effects on DO and pH
// ---------------------------------------------------------------------------

/// Aerated tank shows higher DO and higher pH than unaerated.
///
/// **Literature**: Colt 2006; Boyd & Tucker 1998.
///
/// Two identical tanks with moderate bioload. One has aeration, one does not.
/// After 24 hours the aerated tank should have higher DO (O₂ transfer) and
/// higher pH (CO₂ stripping).
pub fn vs02_aeration_effects() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs02_aeration_effects())
}

fn run_vs02_aeration_effects() -> Result<ProbeResult, tank_core::SimError> {
    let make_state = |aerated: bool| -> TankState {
        let mut state = TankState::new(SimSeed(7302));
        let volume_l = state.water_volume_l();

        // Start with depressed DO (3 mg/L) and elevated DIC (40 mg C/L) to
        // create a large gap between equilibrium and current state. This lets
        // aeration drive measurable O₂ dissolution and CO₂ stripping.
        state.water.temperature_c = 25.0;
        state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
        state.water.alkalinity_meq_total = 2.0 * volume_l;
        state.water.dissolved_oxygen_mg_total = 3.0 * volume_l;
        state.water.ammonia_total_mg_n_total = 4.0 * volume_l;

        // Active biology creates ongoing oxygen demand.
        state.microbe.ammonia_oxidizer_biomass_g = 1.0;
        state.microbe.nitrite_oxidizer_biomass_g = 0.5;
        state.microbe.set_decomposer_total(0.5);
        state.filter_state.biofilter_maturity_index = 0.6;

        // Suppress surface reaeration on both arms so only the aeration
        // hardware drives the difference. Without this, passive surface
        // exchange already recovers most of the DO deficit.
        state.process_params.reaeration_kla_base = 0.01;

        // Set aeration.
        state.hardware.aeration.enabled = aerated;
        state.hardware.aeration.intensity = if aerated { 1.0 } else { 0.0 };

        resolve_carbonate_state(&mut state.water, volume_l);
        state
    };

    let mut aerated_engine = Engine::from_parts(make_state(true), vec![]);
    let mut passive_engine = Engine::from_parts(make_state(false), vec![]);

    if is_verbose() {
        aerated_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
        passive_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    aerated_engine.step_hours(24)?;
    passive_engine.step_hours(24)?;

    let aerated_snap = aerated_engine.snapshot();
    let passive_snap = passive_engine.snapshot();
    let do_gap = aerated_snap.do_mg_l - passive_snap.do_mg_l;
    let ph_gap = aerated_snap.ph - passive_snap.ph;

    let observed = format!(
        "aerated: DO={:.2} pH={:.3} | passive: DO={:.2} pH={:.3} | DO_gap={:.2} pH_gap={:.3}",
        aerated_snap.do_mg_l,
        aerated_snap.ph,
        passive_snap.do_mg_l,
        passive_snap.ph,
        do_gap,
        ph_gap,
    );

    let mut failures = Vec::new();
    // Core directional claim: aeration raises both DO and pH.
    if aerated_snap.do_mg_l <= passive_snap.do_mg_l {
        failures.push(format!(
            "aerated DO {:.2} not higher than passive DO {:.2}",
            aerated_snap.do_mg_l, passive_snap.do_mg_l,
        ));
    }
    if aerated_snap.ph <= passive_snap.ph {
        failures.push(format!(
            "aerated pH {:.3} not higher than passive pH {:.3}",
            aerated_snap.ph, passive_snap.ph,
        ));
    }

    if !failures.is_empty() {
        dump_trace_artifact("vs02_aerated", &aerated_engine);
        dump_trace_artifact("vs02_passive", &passive_engine);
    }

    eprintln!("vs02_aeration_effects: {observed}");

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");
    Ok(ProbeResult {
        name: "VS-02 Aeration effects",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// VS-03: Day/night pH swing
// ---------------------------------------------------------------------------

/// Planted tank shows diurnal pH swing of 0.2-1.0 units.
///
/// **Literature**: Brewer & Goldman 1976; Wetzel 2001.
///
/// Photosynthesis during lit hours draws down CO₂ (raises pH); respiration
/// at night releases CO₂ (lowers pH). The swing should remain detectable
/// across consecutive day/night cycles after day 1 settling.
pub fn vs03_day_night_ph_swing() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs03_day_night_ph_swing())
}

fn run_vs03_day_night_ph_swing() -> Result<ProbeResult, tank_core::SimError> {
    let mut state = TankState::new(SimSeed(7303));
    let volume_l = state.water_volume_l();

    // Heavily planted, moderate-buffer tank to expose a literature-scale
    // diurnal signal without pushing the chemistry into unrealistic extremes.
    state.plant_guilds[0].biomass_g = 18.0;
    state.plant_guilds[1].biomass_g = 12.0;
    state.algae.set_periphyton_total(2.0);
    state.algae.suspended_biomass_g = 0.5;

    // Moderate buffering keeps the chemistry realistic while the heavier
    // biological fluxes create the target planted-tank headroom.
    state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    state.water.alkalinity_meq_total = 1.5 * volume_l;
    state.water.temperature_c = 25.0;

    // 12h/12h photoperiod.
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;

    // Some animal biomass for nighttime respiration.
    state.animal.adult.count = 12;

    // This scenario targets a high-productivity planted tank rather than the
    // simulator's default moderate-growth baseline, so scale the explicit
    // DIC/O2 chemistry rates accordingly while preserving stoichiometric pairs.
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.08;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.03;
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.4;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.15;

    // Zero K_LA to isolate biological DIC effects.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.flow_lph = 0.0;

    resolve_carbonate_state(&mut state.water, volume_l);

    state.environment.hour_of_day = 0;
    state.environment.day = 1;

    let photoperiod = state.hardware.light.photoperiod_hours;
    // Find the light/dark transition hours.
    let mut end_of_dark = None;
    let mut end_of_light = None;
    for hour in 0..24u8 {
        let now = is_light_on(hour, photoperiod);
        let next = is_light_on((hour + 1) % 24, photoperiod);
        if !now && next {
            end_of_dark = Some((hour + 1) % 24);
        }
        if now && !next {
            end_of_light = Some((hour + 1) % 24);
        }
    }
    let end_of_dark = end_of_dark.expect("photoperiod should define end-of-dark");
    let end_of_light = end_of_light.expect("photoperiod should define end-of-light");

    let mut engine = Engine::from_parts(state, vec![]);
    if is_verbose() {
        engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    let mut ph_end_of_light = Vec::new();
    let mut ph_end_of_dark = Vec::new();

    // Run 72h so the post-settling window covers at least two full cycles.
    for _ in 0..72 {
        engine.step_hours(1)?;
        let s = engine.full_state();
        if s.environment.day >= 2 {
            let hour = s.environment.hour_of_day;
            if hour == end_of_light {
                ph_end_of_light.push(engine.snapshot().ph);
            }
            if hour == end_of_dark {
                ph_end_of_dark.push(engine.snapshot().ph);
            }
        }
    }

    let max_light = ph_end_of_light
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let min_dark = ph_end_of_dark.iter().cloned().fold(f64::INFINITY, f64::min);
    let swing = max_light - min_dark;

    let observed = format!(
        "light_max_pH={:.3} dark_min_pH={:.3} swing={:.3} (samples: light={}, dark={})",
        max_light,
        min_dark,
        swing,
        ph_end_of_light.len(),
        ph_end_of_dark.len(),
    );

    let mut failures = Vec::new();
    if ph_end_of_light.len() < 2 || ph_end_of_dark.len() < 2 {
        failures.push("insufficient light/dark transition samples".to_string());
    } else {
        for (i, (lph, dph)) in ph_end_of_light
            .iter()
            .zip(ph_end_of_dark.iter())
            .enumerate()
        {
            if lph <= dph {
                failures.push(format!(
                    "cycle {}: light pH {lph:.3} not > dark pH {dph:.3}",
                    i + 1
                ));
            }
        }
        if swing < 0.2 {
            failures.push(format!("swing {swing:.3} < 0.2 minimum"));
        }
        if swing > 1.0 {
            failures.push(format!("swing {swing:.3} > 1.0 maximum"));
        }
    }

    if !failures.is_empty() {
        dump_trace_artifact("vs03_day_night", &engine);
    }

    eprintln!("vs03_day_night_ph_swing: {observed}");

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");
    Ok(ProbeResult {
        name: "VS-03 Day/night pH swing",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// VS-04: Source-water pH differentiation
// ---------------------------------------------------------------------------

/// RO water vs hard tap gives pH difference > 1.0 unit.
///
/// **Literature**: Basic carbonate chemistry (Stumm & Morgan 1996).
///
/// Two biology-free equilibrium tanks: hard shrimp water (KH ~8) vs RO-like
/// water (KH ~0.1). The pH gap should be >= 1.0 unit.
pub fn vs04_source_water_differentiation() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs04_source_water_differentiation())
}

fn equilibrium_tank(profile: &SourceWaterProfile, volume_l: f64, seed: SimSeed) -> TankState {
    let water = WaterState::from_source_profile_for_volume_l(profile, volume_l);
    let mut state = TankState::new(seed);
    state.water = water;

    for sw_id in tank_data::source_water_ids() {
        state
            .source_water_catalog
            .insert(sw_id.to_string(), load_source_profile(sw_id));
    }

    // Suppress all biology.
    state.plant_guilds.clear();
    state.algae.set_periphyton_total(0.0);
    state.algae.suspended_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microbe.set_decomposer_total(0.0);
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;

    // Suppress gas exchange.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;

    state
}

fn run_vs04_source_water_differentiation() -> Result<ProbeResult, tank_core::SimError> {
    let hard = load_source_profile("hard_shrimp");
    let ro = load_source_profile("ro_like");
    let volume_l = 30.0;

    let hard_state = equilibrium_tank(&hard, volume_l, SimSeed(7304));
    let ro_state = equilibrium_tank(&ro, volume_l, SimSeed(7304));

    let mut hard_engine = Engine::from_parts(hard_state, vec![]);
    let mut ro_engine = Engine::from_parts(ro_state, vec![]);

    if is_verbose() {
        hard_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
        ro_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    hard_engine.step_hours(100)?;
    ro_engine.step_hours(100)?;

    let hard_snap = hard_engine.snapshot();
    let ro_snap = ro_engine.snapshot();
    let ph_gap = hard_snap.ph - ro_snap.ph;

    let observed = format!(
        "hard_shrimp: pH={:.3} | ro_like: pH={:.3} | gap={:.3}",
        hard_snap.ph, ro_snap.ph, ph_gap,
    );

    let mut failures = Vec::new();
    if hard_snap.ph < 7.0 || hard_snap.ph > 8.5 {
        failures.push(format!(
            "hard_shrimp pH {:.3} outside [7.0, 8.5]",
            hard_snap.ph
        ));
    }
    if ro_snap.ph < 5.5 || ro_snap.ph > 7.0 {
        failures.push(format!("ro_like pH {:.3} outside [5.5, 7.0]", ro_snap.ph));
    }
    if ph_gap < 1.0 {
        failures.push(format!("pH gap {ph_gap:.3} < 1.0 minimum"));
    }

    if !failures.is_empty() {
        dump_trace_artifact("vs04_hard_shrimp", &hard_engine);
        dump_trace_artifact("vs04_ro_like", &ro_engine);
    }

    eprintln!("vs04_source_water_differentiation: {observed}");

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");
    Ok(ProbeResult {
        name: "VS-04 Source-water differentiation",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// VS-05: Neocaridina breeding thermal window
// ---------------------------------------------------------------------------

/// Neocaridina breed at 25°C, suppress above 31°C.
///
/// **Literature**: Tropea et al. 2015; Pantaleão et al. 2015.
///
/// Cool arm (25°C): population grows, ≥2 complete repro cycles, juveniles present.
/// Warm arm (31°C): reproductive readiness depressed, fewer juveniles than cool.
pub fn vs05_shrimp_breeding_thermal_window() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs05_shrimp_breeding())
}

fn breeding_state(seed: SimSeed, temp_c: f64) -> TankState {
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
    state.water.temperature_c = temp_c;
    state.environment.ambient_temp_c = temp_c;

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

    state.algae.set_periphyton_total(120.0);
    state.microbe.set_decomposer_total(0.3);
    state.microbe.ammonia_oxidizer_biomass_g = 10.0;
    state.microbe.nitrite_oxidizer_biomass_g = 10.0;
    state.microbe.comammox_biomass_g = 2.0;
    state.filter_state.biofilter_maturity_index = 1.0;

    let mut source = load_source_profile("hard_shrimp");
    source.temperature_c = temp_c;
    state
        .source_water_catalog
        .insert("breed_source".to_string(), source);

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
    // Disable mortality to isolate reproductive mechanics.
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

fn run_vs05_shrimp_breeding() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let cool_state = breeding_state(SimSeed(7305), 25.0);
    let warm_state = breeding_state(SimSeed(7305), 31.0);

    let mut cool_run = HarnessRun::from_state(SimSeed(7305), "vs05_cool", cool_state)
        .with_artifact_label("vs05_cool");
    let mut warm_run = HarnessRun::from_state(SimSeed(7305), "vs05_warm", warm_state)
        .with_artifact_label("vs05_warm");
    enable_instrumentation(&mut cool_run);
    enable_instrumentation(&mut warm_run);

    for day in 1..=60 {
        cool_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        warm_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        cool_run.step_hours(24)?;
        warm_run.step_hours(24)?;

        if day % 5 == 0 {
            cool_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "breed_source".to_string(),
            })?;
            warm_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "breed_source".to_string(),
            })?;
            cool_run.step_hours(1)?;
            warm_run.step_hours(1)?;
        }
    }

    let cool_snap = cool_run.snapshot();
    let warm_snap = warm_run.snapshot();
    let cool_state = cool_run.engine().full_state().clone();
    let _warm_state = warm_run.engine().full_state();

    let cool_cycles: usize = cool_state
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::ShrimpBerried)
        .count()
        .min(
            cool_state
                .event_log
                .iter()
                .filter(|e| e.kind == EventKind::ShrimpHatched)
                .count(),
        );

    // Cool arm: must grow and have ≥2 complete cycles.
    if cool_snap.total_shrimp_count <= 10 {
        cool_run.record_failure(
            "cool_population_growth",
            format!(
                "cool arm should grow: final={}",
                cool_snap.total_shrimp_count
            ),
        );
    }
    if cool_cycles < 2 {
        cool_run.record_failure(
            "cool_repro_cycles",
            format!("cool arm should have ≥2 complete cycles, got {cool_cycles}"),
        );
    }
    if cool_snap.juveniles_count == 0 && cool_snap.sub_adult_count == 0 {
        cool_run.record_failure(
            "cool_offspring",
            "cool arm should have juveniles or sub-adults by day 60".to_string(),
        );
    }

    // Warm arm: fewer juveniles than cool.
    let cool_offspring = cool_snap.juveniles_count + cool_snap.sub_adult_count;
    let warm_offspring = warm_snap.juveniles_count + warm_snap.sub_adult_count;
    if warm_offspring >= cool_offspring && cool_offspring > 0 {
        warm_run.record_failure(
            "warm_fewer_offspring",
            format!(
                "warm arm should have fewer offspring: cool={cool_offspring}, warm={warm_offspring}"
            ),
        );
    }

    let observed = format!(
        "cool: pop={} cycles={cool_cycles} juv={} sub={} readiness={:.3} | \
         warm: pop={} juv={} sub={} readiness={:.3}",
        cool_snap.total_shrimp_count,
        cool_snap.juveniles_count,
        cool_snap.sub_adult_count,
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.total_shrimp_count,
        warm_snap.juveniles_count,
        warm_snap.sub_adult_count,
        warm_snap.shrimp_reproductive_readiness,
    );
    eprintln!("vs05_shrimp_breeding: {observed}");

    Ok(finish_probe(
        "VS-05 Shrimp breeding thermal window",
        observed,
        vec![cool_run, warm_run],
    ))
}

// ---------------------------------------------------------------------------
// VS-06: Algae-plant competition
// ---------------------------------------------------------------------------

/// High light + low nutrients favors algae over slow-growing plants.
///
/// **Literature**: Tilman 1982 (R* theory); Stevenson 1996.
///
/// A tank with moderate initial plant biomass, high light, very low dissolved
/// N and P, no fertilization, no shrimp. Algae nuisance should rise while
/// plant biomass declines.
pub fn vs06_algae_plant_competition() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs06_algae_plant_competition())
}

fn run_vs06_algae_plant_competition() -> Result<ProbeResult, tank_core::SimError> {
    let mut state = TankState::new(SimSeed(7306));
    let volume_l = state.water_volume_l();

    // Moderate initial plant biomass (slow-growing rosette guild only).
    // Rosettes have lower max growth rate (0.06/day vs 0.08/day) and higher
    // light requirements, making them more vulnerable to algae competition.
    state.plant_guilds[0].biomass_g = 0.0; // fast stem removed
    state.plant_guilds[1].biomass_g = 8.0; // rosette only

    // Low but non-zero dissolved N and P — enough for algae to grow but
    // limiting for the larger plant guild. Tilman's R* theory predicts the
    // organism with the lower half-saturation constant wins at low resources.
    state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
    state.water.nitrate_mg_n_total = 2.0 * volume_l;
    state.water.phosphate_mg_p_total = 0.1 * volume_l;

    // High light, long photoperiod — favors algae which have higher light
    // utilization per unit biomass.
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 1.0;
    state.hardware.light.photoperiod_hours = 14.0;

    // Larger algae seed for realistic colonization. Periphyton especially
    // competes directly with plants for light and nutrients on surfaces.
    state.algae.suspended_biomass_g = 0.5;
    state.algae.set_periphyton_total(2.0);

    // No animals, no microfauna grazing to suppress algae.
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let initial_snap = TankSnapshot::from_state(&state);
    let initial_plant = initial_snap.total_plant_biomass_g;
    let initial_health = initial_snap.root_feeding_rosette_health_index;

    let mut engine = Engine::from_parts(state, vec![]);
    if is_verbose() {
        engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    // Run 90 days with small daily feed to provide a trickle of nutrients
    // that favors fast-uptake algae over slower rosettes.
    for _ in 0..90 {
        engine.apply_action(PlayerAction::Feed { grams: 0.02 })?;
        engine.step_hours(24)?;
    }

    let final_snap = engine.snapshot();
    let final_plant = final_snap.total_plant_biomass_g;
    let final_algae_total = final_snap.suspended_algae_biomass_g + final_snap.periphyton_biomass_g;
    let initial_algae_total =
        initial_snap.suspended_algae_biomass_g + initial_snap.periphyton_biomass_g;
    let final_health = final_snap.root_feeding_rosette_health_index;

    let observed = format!(
        "plants: {initial_plant:.2}g->{final_plant:.2}g health: {initial_health:.3}->{final_health:.3} | \
         algae_biomass: {initial_algae_total:.3}g->{final_algae_total:.3}g nuisance: {:.3}->{:.3}",
        initial_snap.algae_nuisance_index, final_snap.algae_nuisance_index,
    );

    let mut failures = Vec::new();

    // Core directional claim: under high-light, low-nutrient conditions
    // with a trickle feed, algae should gain biomass relative to plants.
    // We check total algae biomass (not nuisance index, which may have
    // complex normalization) and plant health decline.

    // Algae total biomass should increase (or at least not collapse).
    if final_algae_total < initial_algae_total * 0.5 {
        failures.push(format!(
            "algae biomass should not collapse under high light: \
             initial={initial_algae_total:.3}g, final={final_algae_total:.3}g"
        ));
    }

    // Plant biomass should decline or stagnate under nutrient limitation
    // and algae competition for light.
    if final_plant > initial_plant * 1.2 {
        failures.push(format!(
            "plants should not grow significantly: \
             initial={initial_plant:.2}g, final={final_plant:.2}g"
        ));
    }

    // Plant health should decline.
    if final_health >= initial_health && initial_health > 0.0 {
        failures.push(format!(
            "plant health should decline under nutrient limitation: \
             initial={initial_health:.3}, final={final_health:.3}"
        ));
    }

    if !failures.is_empty() {
        dump_trace_artifact("vs06_algae_competition", &engine);
    }

    eprintln!("vs06_algae_plant_competition: {observed}");

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");
    Ok(ProbeResult {
        name: "VS-06 Algae-plant competition",
        passed,
        observed,
        failure_detail,
    })
}

// ---------------------------------------------------------------------------
// VS-07: Nitrate removal via substrate denitrification
// ---------------------------------------------------------------------------

/// Planted tank with mature substrate shows lower steady-state NO₃ than
/// bare-bottom via denitrification.
///
/// **Literature**: Seitzinger 1988; Vymazal 2007.
///
/// Two planted tanks: one with a deep active substrate bed (denitrification
/// pathway), one bare-bottom (no substrate, no denitrification). Both share
/// the same fast-stem biomass, feeding, and low-nitrate buffered water-change
/// source. After a long-horizon run the planted-substrate tank should finish
/// with lower NO₃ and measurable cumulative N₂ export.
pub fn vs07_nitrate_removal_denitrification() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs07_nitrate_removal())
}

fn run_vs07_nitrate_removal() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    const VS07_WATER_CHANGE_SOURCE: &str = "vs07_buffered_low_nitrate";

    let mut buffered_source = load_source_profile("hard_shrimp");
    buffered_source.nitrate_mg_n_per_l = 0.0;

    let normalize_fast_stems = |state: &mut TankState, biomass_g: f64| {
        for plant in &mut state.plant_guilds {
            match plant.guild {
                PlantGuild::FastStem => {
                    plant.biomass_g = biomass_g;
                    plant.health_index = 0.95;
                }
                _ => {
                    plant.biomass_g = 0.0;
                }
            }
        }
    };

    let prepare_vs07_biology = |state: &mut TankState| {
        normalize_fast_stems(state, 18.0);
        state.microbe.decomposer_biomass_g = 8.0;
        state.detritus.fine_detritus_g_total = 2.0;
        state.microfauna.population_index = 0.8;
        state.algae.set_periphyton_total(4.0);
        state.algae.suspended_biomass_g = 0.4;
    };

    let planted_overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlanted),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    let mut planted_state = tank_scenarios::seeded_state_with_full_overrides(
        SimSeed(7307),
        "medium_planted",
        planted_overrides,
    )?;

    planted_state.source_water_catalog.insert(
        VS07_WATER_CHANGE_SOURCE.to_string(),
        buffered_source.clone(),
    );
    prepare_vs07_biology(&mut planted_state);

    // Mature denitrifiers plus a deep, low-porosity bed create the suboxic
    // volume needed to keep the planted arm below the bare control.
    planted_state.microbe.denitrifier_activity_index = 1.0;
    planted_state
        .process_params
        .denitrification_vmax_mg_n_per_l_per_hour = 0.2;
    planted_state
        .process_params
        .denitrification_pore_water_mixing_factor = 1.0;
    for layer in &mut planted_state.substrate_layers {
        layer.depth_cm = 10.0;
        layer.porosity = 0.32;
        layer.nutrient_store_mg_n_total = 0.0;
        layer.nutrient_store_mg_p_total = 0.0;
    }

    // Start both arms from the same nitrate and DOC inventory so the only
    // enduring difference is the denitrifying substrate pathway.
    let vol = planted_state.water_volume_l();
    planted_state.water.dissolved_organic_carbon_mg_c_total = 12.0 * vol;
    planted_state.water.nitrate_mg_n_total = 12.0 * vol;
    planted_state.refresh_habitat_registry();

    let mut planted_run = HarnessRun::from_state(SimSeed(7307), "vs07_planted", planted_state)
        .with_artifact_label("vs07_planted");
    enable_instrumentation(&mut planted_run);

    // Bare-bottom arm: no substrate, so no suboxic zone, no denitrification.
    let bare_overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::InertSand),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    // Reuse the same planted template so both arms inherit matching geometry,
    // hardware, and fast-stem planting; the control becomes "bare-bottom"
    // when we clear substrate below, leaving denitrification as the key delta.
    let mut bare_state = tank_scenarios::seeded_state_with_full_overrides(
        SimSeed(7307),
        "medium_planted",
        bare_overrides,
    )?;
    bare_state
        .source_water_catalog
        .insert(VS07_WATER_CHANGE_SOURCE.to_string(), buffered_source);
    prepare_vs07_biology(&mut bare_state);

    // Remove substrate entirely to eliminate any denitrification pathway.
    bare_state.substrate_layers.clear();
    bare_state
        .process_params
        .denitrification_vmax_mg_n_per_l_per_hour = 0.2;
    bare_state
        .process_params
        .denitrification_pore_water_mixing_factor = 1.0;

    // Match initial nitrate and DOC with planted arm.
    let bare_vol = bare_state.water_volume_l();
    bare_state.water.nitrate_mg_n_total = 12.0 * bare_vol;
    bare_state.water.dissolved_organic_carbon_mg_c_total = 12.0 * bare_vol;
    bare_state.refresh_habitat_registry();

    let mut bare_run = HarnessRun::from_state(SimSeed(7307), "vs07_bare", bare_state)
        .with_artifact_label("vs07_bare");
    enable_instrumentation(&mut bare_run);

    // 120 days of identical feeding with low-nitrate buffered water changes.
    // The longer horizon gives the deep substrate enough time to establish a
    // meaningful steady-state nitrate gap relative to the bare control.
    for day in 1..=120 {
        planted_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        bare_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        planted_run.step_hours(24)?;
        bare_run.step_hours(24)?;

        if day % 7 == 0 {
            planted_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: VS07_WATER_CHANGE_SOURCE.to_string(),
            })?;
            bare_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: VS07_WATER_CHANGE_SOURCE.to_string(),
            })?;
            planted_run.step_hours(1)?;
            bare_run.step_hours(1)?;
        }
    }

    let planted_snap = planted_run.snapshot();
    let bare_snap = bare_run.snapshot();
    // Extract values before mutable borrows for record_failure.
    let planted_n2 = planted_run.engine().full_state().cumulative_n2_export_mg_n;
    let bare_n2 = bare_run.engine().full_state().cumulative_n2_export_mg_n;
    let planted_denitrifier_act = planted_run
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;
    let bare_denitrifier_act = bare_run
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;

    let observed = format!(
        "planted: NO3={:.2} N2_export={:.2}mg_N denitrifier_act={:.3} | \
         bare: NO3={:.2} N2_export={:.2}mg_N denitrifier_act={:.3}",
        planted_snap.nitrate_mg_n_per_l,
        planted_n2,
        planted_denitrifier_act,
        bare_snap.nitrate_mg_n_per_l,
        bare_n2,
        bare_denitrifier_act,
    );

    if planted_snap.nitrate_mg_n_per_l >= bare_snap.nitrate_mg_n_per_l {
        planted_run.record_failure(
            "nitrate_gap",
            format!(
                "planted final NO3 should be lower than bare-bottom: planted={:.3} mg/L bare={:.3} mg/L",
                planted_snap.nitrate_mg_n_per_l, bare_snap.nitrate_mg_n_per_l,
            ),
        );
    }

    if planted_n2 <= 1.0 {
        planted_run.record_failure(
            "n2_export",
            format!(
                "planted N2 export should be > 1.0 mg N (ecologically meaningful), got {planted_n2:.4}"
            ),
        );
    }

    if bare_n2 > 0.1 {
        bare_run.record_failure(
            "bare_no_denitrification",
            format!("bare-bottom should have negligible N2 export, got {bare_n2:.4}"),
        );
    }

    if planted_denitrifier_act <= 0.1 {
        planted_run.record_failure(
            "denitrifier_active",
            format!(
                "planted denitrifier activity should be > 0.1, got {planted_denitrifier_act:.3}",
            ),
        );
    }

    eprintln!("vs07_nitrate_removal: {observed}");

    Ok(finish_probe(
        "VS-07 Nitrate removal (denitrification)",
        observed,
        vec![planted_run, bare_run],
    ))
}

// ---------------------------------------------------------------------------
// VS-08: Stocking density crash
// ---------------------------------------------------------------------------

/// Overcrowding + overfeeding leads to water quality crash within weeks.
///
/// **Literature**: Timmons & Ebeling 2010; Colt & Armstrong 1981.
///
/// A 30L tank with 20 shrimp (heavily overcrowded), 0.5 g/day feeding,
/// no water changes. TAN and NO₂ spike; shrimp die from toxicity.
pub fn vs08_stocking_density_crash() -> Result<(), Box<dyn Error>> {
    require_probe_pass(run_vs08_stocking_density_crash())
}

fn run_vs08_stocking_density_crash() -> Result<ProbeResult, Box<dyn std::error::Error>> {
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
    let mut state = TankState::new(SimSeed(7308));
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

    // Partially cycled — can handle some load but will be overwhelmed.
    state.microbe.set_decomposer_total(0.15);
    state.microbe.ammonia_oxidizer_biomass_g = 0.3;
    state.microbe.nitrite_oxidizer_biomass_g = 0.2;
    state.microbe.comammox_biomass_g = 0.1;
    state.filter_state.biofilter_maturity_index = 0.5;

    state.hardware.aeration.enabled = false;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.4;
    state.hardware.light.photoperiod_hours = 8.0;

    // Overcrowded: 20 in ~15 L.
    state.animal.adult.count = 20;
    state.animal.set_population_condition_index(0.6);
    state.animal.molt_stress_index = 0.2;
    state.animal.reproductive_readiness_index = 0.1;

    state.process_params = ProcessParams::default();
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();

    let mut run = HarnessRun::from_state(SimSeed(7308), "vs08_crash", state)
        .with_artifact_label("vs08_crash");
    enable_instrumentation(&mut run);

    let initial_count = run.engine().full_state().animal.total_count();

    // 8 weeks of overfeeding with no water changes.
    for day in 1..=56 {
        run.apply_action(PlayerAction::Feed { grams: 0.5 })?;
        run.step_hours(24)?;

        let week = day / 7;
        if day % 7 == 0 {
            run.checkpoint(&format!("week_{week}"));

            match week {
                1 => {
                    run.assert_envelope(
                        "crash_week1",
                        &Envelope::default()
                            .tan_mg_n_per_l(1.0, 30.0)
                            .shrimp_count(5, 20)
                            .ph(5.0, 8.5),
                    );
                }
                2 => {
                    run.assert_envelope(
                        "crash_week2",
                        &Envelope::default()
                            .tan_mg_n_per_l(3.0, 60.0)
                            .shrimp_count(0, 15)
                            .ph(4.5, 8.5),
                    );
                }
                4 => {
                    run.assert_envelope(
                        "crash_week4",
                        &Envelope::default()
                            .tan_mg_n_per_l(5.0, 100.0)
                            .shrimp_count(0, 5)
                            .ph(4.5, 8.5),
                    );
                }
                8 => {
                    // After 8 weeks of 0.5g/day overfeeding with no water
                    // changes and dead shrimp decomposing, TAN can reach
                    // extreme levels. The specific ceiling depends on
                    // decomposition rates but 300+ is realistic.
                    run.assert_envelope(
                        "crash_week8",
                        &Envelope::default()
                            .tan_mg_n_per_l(10.0, 400.0)
                            .shrimp_count(0, 0)
                            .ph(4.5, 8.5),
                    );
                }
                _ => {}
            }
        }
    }

    let final_snap = run.snapshot();

    // At least half the population should have died by the end.
    if final_snap.total_shrimp_count > initial_count / 2 {
        run.record_failure(
            "population_crash",
            format!(
                "crash scenario should lose ≥50% of population: initial={initial_count}, final={}",
                final_snap.total_shrimp_count,
            ),
        );
    }

    let observed = format!(
        "initial={initial_count}, final={}, TAN={:.2}, NO2={:.2}, pH={:.3}",
        final_snap.total_shrimp_count,
        final_snap.tan_mg_n_per_l,
        final_snap.nitrite_mg_n_per_l,
        final_snap.ph,
    );
    eprintln!("vs08_stocking_density_crash: {observed}");

    Ok(finish_probe(
        "VS-08 Stocking density crash",
        observed,
        vec![run],
    ))
}

// ===========================================================================
// Calibration workflow: structured report rows for all shipped scenarios
// ===========================================================================

pub fn run_calibration_suite(parameter_variant: &str) -> Result<CalibrationReport, Box<dyn Error>> {
    let mut rows = Vec::new();
    for definition in validation_scenarios() {
        rows.push(run_calibration_scenario(definition.id, parameter_variant)?);
    }
    Ok(CalibrationReport::from_rows(parameter_variant, rows))
}

pub fn run_calibration_scenario(
    scenario_id: &str,
    parameter_variant: &str,
) -> Result<ScenarioRow, Box<dyn Error>> {
    match scenario_id {
        "vs01_cycling_timeline" => calibrate_vs01(parameter_variant),
        "vs02_aeration_effects" => calibrate_vs02(parameter_variant),
        "vs03_day_night_ph_swing" => calibrate_vs03(parameter_variant),
        "vs04_source_water_differentiation" => calibrate_vs04(parameter_variant),
        "vs05_shrimp_breeding_thermal_window" => calibrate_vs05(parameter_variant),
        "vs06_algae_plant_competition" => calibrate_vs06(parameter_variant),
        "vs07_nitrate_removal_denitrification" => calibrate_vs07(parameter_variant),
        "vs08_stocking_density_crash" => calibrate_vs08(parameter_variant),
        _ => Err(format!("unknown validation scenario id '{scenario_id}'").into()),
    }
}

fn calibrate_vs01(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs01_cycling_timeline");
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlantedWithCoarsePorous),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    let run = HarnessRun::with_overrides(definition.seed, "medium_planted", overrides)?
        .with_artifact_label(definition.id);
    let mut calibration = CalibrationRun::new(run, parameter_variant);
    calibration.enable_instrumentation();

    for day in 1..=56 {
        calibration.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        calibration.step_hours(24)?;

        if day % 7 == 0 {
            calibration.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: "hard_shrimp".to_string(),
            })?;
            calibration.step_hours(1)?;

            match day / 7 {
                2 => calibration.check_envelope(
                    "week_2",
                    &Envelope::default()
                        .ph(6.5, 8.5)
                        .tan_mg_n_per_l(0.0, 15.0)
                        .nitrite_mg_n_per_l(0.0, 5.0)
                        .nitrate_mg_n_per_l(0.0, 15.0)
                        .do_min(6.0),
                ),
                4 => calibration.check_envelope(
                    "week_4",
                    &Envelope::default()
                        .ph(6.5, 8.5)
                        .tan_mg_n_per_l(0.0, 10.0)
                        .nitrite_mg_n_per_l(0.0, 10.0)
                        .nitrate_mg_n_per_l(0.5, 25.0)
                        .do_min(5.5),
                ),
                6 => calibration.check_envelope(
                    "week_6",
                    &Envelope::default()
                        .ph(6.5, 8.5)
                        .tan_mg_n_per_l(0.0, 3.0)
                        .nitrite_mg_n_per_l(0.0, 3.0)
                        .nitrate_mg_n_per_l(2.0, 40.0)
                        .do_min(5.5),
                ),
                8 => calibration.check_envelope(
                    "week_8",
                    &Envelope::default()
                        .ph(6.5, 8.5)
                        .tan_mg_n_per_l(0.0, 2.0)
                        .nitrite_mg_n_per_l(0.0, 2.0)
                        .nitrate_mg_n_per_l(5.0, 60.0)
                        .do_min(5.5),
                ),
                _ => {}
            }
        }
    }

    let snap = calibration.inner().snapshot();
    let observed = format!(
        "week8: TAN={:.3} NO2={:.3} NO3={:.3} pH={:.3} maturity={:.3}",
        snap.tan_mg_n_per_l,
        snap.nitrite_mg_n_per_l,
        snap.nitrate_mg_n_per_l,
        snap.ph,
        snap.biofilter_maturity_index,
    );

    Ok(annotate_row(definition, calibration.finish(), observed))
}

fn calibrate_vs02(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs02_aeration_effects");
    let make_state = |aerated: bool| -> TankState {
        let mut state = TankState::new(definition.seed);
        let volume_l = state.water_volume_l();

        state.water.temperature_c = 25.0;
        state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
        state.water.alkalinity_meq_total = 2.0 * volume_l;
        state.water.dissolved_oxygen_mg_total = 3.0 * volume_l;
        state.water.ammonia_total_mg_n_total = 4.0 * volume_l;
        state.microbe.ammonia_oxidizer_biomass_g = 1.0;
        state.microbe.nitrite_oxidizer_biomass_g = 0.5;
        state.microbe.set_decomposer_total(0.5);
        state.filter_state.biofilter_maturity_index = 0.6;
        state.process_params.reaeration_kla_base = 0.01;
        state.hardware.aeration.enabled = aerated;
        state.hardware.aeration.intensity = if aerated { 1.0 } else { 0.0 };

        resolve_carbonate_state(&mut state.water, volume_l);
        state
    };

    let mut aerated_run = HarnessRun::from_state(definition.seed, "vs02_aerated", make_state(true))
        .with_artifact_label("vs02_aerated");
    let mut passive_run =
        HarnessRun::from_state(definition.seed, "vs02_passive", make_state(false))
            .with_artifact_label("vs02_passive");
    enable_instrumentation(&mut aerated_run);
    enable_instrumentation(&mut passive_run);

    aerated_run.step_hours(24)?;
    passive_run.step_hours(24)?;

    let aerated_snap = aerated_run.snapshot();
    let passive_snap = passive_run.snapshot();
    let do_gap = aerated_snap.do_mg_l - passive_snap.do_mg_l;
    let ph_gap = aerated_snap.ph - passive_snap.ph;
    let observed = format!(
        "aerated: DO={:.2} pH={:.3} | passive: DO={:.2} pH={:.3} | DO_gap={:.2} pH_gap={:.3}",
        aerated_snap.do_mg_l,
        aerated_snap.ph,
        passive_snap.do_mg_l,
        passive_snap.ph,
        do_gap,
        ph_gap,
    );

    let state = aerated_run.engine().full_state();
    let fields = vec![
        FieldCheck::observed_only("aerated_do_mg_l", aerated_snap.do_mg_l),
        FieldCheck::observed_only("passive_do_mg_l", passive_snap.do_mg_l),
        classify_field_f64("do_gap_mg_l", do_gap, 0.5, f64::INFINITY),
        FieldCheck::observed_only("aerated_ph", aerated_snap.ph),
        FieldCheck::observed_only("passive_ph", passive_snap.ph),
        classify_field_f64("ph_gap", ph_gap, 0.2, f64::INFINITY),
    ];

    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "comparison_24h",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![("aerated", aerated_run), ("passive", passive_run)],
    ))
}

fn calibrate_vs03(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs03_day_night_ph_swing");
    let mut state = TankState::new(definition.seed);
    let volume_l = state.water_volume_l();

    state.plant_guilds[0].biomass_g = 18.0;
    state.plant_guilds[1].biomass_g = 12.0;
    state.algae.set_periphyton_total(2.0);
    state.algae.suspended_biomass_g = 0.5;
    state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    state.water.alkalinity_meq_total = 1.5 * volume_l;
    state.water.temperature_c = 25.0;
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;
    state.animal.adult.count = 12;
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.08;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.03;
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.4;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.15;
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.flow_lph = 0.0;
    resolve_carbonate_state(&mut state.water, volume_l);
    state.environment.hour_of_day = 0;
    state.environment.day = 1;

    let photoperiod = state.hardware.light.photoperiod_hours;
    let mut end_of_dark = None;
    let mut end_of_light = None;
    for hour in 0..24u8 {
        let now = is_light_on(hour, photoperiod);
        let next = is_light_on((hour + 1) % 24, photoperiod);
        if !now && next {
            end_of_dark = Some((hour + 1) % 24);
        }
        if now && !next {
            end_of_light = Some((hour + 1) % 24);
        }
    }
    let end_of_dark = end_of_dark.expect("photoperiod should define end-of-dark");
    let end_of_light = end_of_light.expect("photoperiod should define end-of-light");

    let mut run = HarnessRun::from_state(definition.seed, definition.id, state)
        .with_artifact_label(definition.id);
    enable_instrumentation(&mut run);

    let mut ph_end_of_light = Vec::new();
    let mut ph_end_of_dark = Vec::new();
    for _ in 0..72 {
        run.step_hours(1)?;
        let state = run.engine().full_state();
        if state.environment.day >= 2 {
            let hour = state.environment.hour_of_day;
            if hour == end_of_light {
                ph_end_of_light.push(run.snapshot().ph);
            }
            if hour == end_of_dark {
                ph_end_of_dark.push(run.snapshot().ph);
            }
        }
    }

    let max_light = ph_end_of_light
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let min_dark = ph_end_of_dark.iter().copied().fold(f64::INFINITY, f64::min);
    let swing = max_light - min_dark;
    let observed = format!(
        "light_max_pH={:.3} dark_min_pH={:.3} swing={:.3} (samples: light={}, dark={})",
        max_light,
        min_dark,
        swing,
        ph_end_of_light.len(),
        ph_end_of_dark.len(),
    );

    let mut fields = vec![
        FieldCheck::observed_only("light_sample_count", ph_end_of_light.len() as f64),
        FieldCheck::observed_only("dark_sample_count", ph_end_of_dark.len() as f64),
        classify_field_f64("ph_swing", swing, 0.2, 1.0),
    ];
    if ph_end_of_light.len() < 2 || ph_end_of_dark.len() < 2 {
        fields.push(fail_field(
            "transition_samples",
            "insufficient light/dark transition samples",
        ));
    } else {
        for (index, (light_ph, dark_ph)) in ph_end_of_light
            .iter()
            .zip(ph_end_of_dark.iter())
            .enumerate()
        {
            fields.push(classify_field_f64(
                &format!("cycle_{}_light_minus_dark", index + 1),
                light_ph - dark_ph,
                0.001,
                f64::INFINITY,
            ));
        }
    }

    let state = run.engine().full_state();
    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "day_night_cycles",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![("day_night", run)],
    ))
}

fn calibrate_vs04(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs04_source_water_differentiation");
    let hard = load_source_profile("hard_shrimp");
    let ro = load_source_profile("ro_like");
    let volume_l = 30.0;

    let hard_state = equilibrium_tank(&hard, volume_l, definition.seed);
    let ro_state = equilibrium_tank(&ro, volume_l, definition.seed);
    let mut hard_run = HarnessRun::from_state(definition.seed, "vs04_hard", hard_state)
        .with_artifact_label("vs04_hard");
    let mut ro_run =
        HarnessRun::from_state(definition.seed, "vs04_ro", ro_state).with_artifact_label("vs04_ro");
    enable_instrumentation(&mut hard_run);
    enable_instrumentation(&mut ro_run);

    hard_run.step_hours(100)?;
    ro_run.step_hours(100)?;

    let hard_snap = hard_run.snapshot();
    let ro_snap = ro_run.snapshot();
    let ph_gap = hard_snap.ph - ro_snap.ph;
    let observed = format!(
        "hard_shrimp: pH={:.3} | ro_like: pH={:.3} | gap={:.3}",
        hard_snap.ph, ro_snap.ph, ph_gap,
    );

    let state = hard_run.engine().full_state();
    let fields = vec![
        classify_field_f64("hard_shrimp_ph", hard_snap.ph, 7.0, 8.5),
        classify_field_f64("ro_like_ph", ro_snap.ph, 5.5, 7.0),
        classify_field_f64("ph_gap", ph_gap, 1.0, f64::INFINITY),
    ];

    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "equilibrium_100h",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![("hard_shrimp", hard_run), ("ro_like", ro_run)],
    ))
}

fn calibrate_vs05(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs05_shrimp_breeding_thermal_window");
    let cool_state = breeding_state(definition.seed, 25.0);
    let warm_state = breeding_state(definition.seed, 31.0);
    let mut cool_run = HarnessRun::from_state(definition.seed, "vs05_cool", cool_state)
        .with_artifact_label("vs05_cool");
    let mut warm_run = HarnessRun::from_state(definition.seed, "vs05_warm", warm_state)
        .with_artifact_label("vs05_warm");
    enable_instrumentation(&mut cool_run);
    enable_instrumentation(&mut warm_run);

    for day in 1..=60 {
        cool_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        warm_run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        cool_run.step_hours(24)?;
        warm_run.step_hours(24)?;

        if day % 5 == 0 {
            cool_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "breed_source".to_string(),
            })?;
            warm_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "breed_source".to_string(),
            })?;
            cool_run.step_hours(1)?;
            warm_run.step_hours(1)?;
        }
    }

    let cool_snap = cool_run.snapshot();
    let warm_snap = warm_run.snapshot();
    let cool_state = cool_run.engine().full_state().clone();
    let cool_cycles = cool_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpBerried)
        .count()
        .min(
            cool_state
                .event_log
                .iter()
                .filter(|event| event.kind == EventKind::ShrimpHatched)
                .count(),
        ) as f64;
    let cool_offspring = (cool_snap.juveniles_count + cool_snap.sub_adult_count) as f64;
    let warm_offspring = (warm_snap.juveniles_count + warm_snap.sub_adult_count) as f64;

    let observed = format!(
        "cool: pop={} cycles={} juv={} sub={} readiness={:.3} | warm: pop={} juv={} sub={} readiness={:.3}",
        cool_snap.total_shrimp_count,
        cool_cycles as u32,
        cool_snap.juveniles_count,
        cool_snap.sub_adult_count,
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.total_shrimp_count,
        warm_snap.juveniles_count,
        warm_snap.sub_adult_count,
        warm_snap.shrimp_reproductive_readiness,
    );

    let state = cool_run.engine().full_state();
    let fields = vec![
        classify_field_f64(
            "cool_final_population",
            cool_snap.total_shrimp_count as f64,
            11.0,
            f64::INFINITY,
        ),
        classify_field_f64("cool_complete_cycles", cool_cycles, 2.0, f64::INFINITY),
        classify_field_f64("cool_offspring_count", cool_offspring, 1.0, f64::INFINITY),
        classify_field_f64(
            "cool_minus_warm_offspring",
            cool_offspring - warm_offspring,
            0.001,
            f64::INFINITY,
        ),
        classify_field_f64(
            "cool_minus_warm_readiness",
            cool_snap.shrimp_reproductive_readiness - warm_snap.shrimp_reproductive_readiness,
            0.001,
            f64::INFINITY,
        ),
        FieldCheck::observed_only("warm_final_population", warm_snap.total_shrimp_count as f64),
    ];

    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "day_60",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![("cool_25c", cool_run), ("warm_31c", warm_run)],
    ))
}

fn calibrate_vs06(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs06_algae_plant_competition");
    let mut state = TankState::new(definition.seed);
    let volume_l = state.water_volume_l();
    state.plant_guilds[0].biomass_g = 0.0;
    state.plant_guilds[1].biomass_g = 8.0;
    state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
    state.water.nitrate_mg_n_total = 2.0 * volume_l;
    state.water.phosphate_mg_p_total = 0.1 * volume_l;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 1.0;
    state.hardware.light.photoperiod_hours = 14.0;
    state.algae.suspended_biomass_g = 0.5;
    state.algae.set_periphyton_total(2.0);
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let initial_snap = TankSnapshot::from_state(&state);
    let initial_plant = initial_snap.total_plant_biomass_g;
    let initial_algae_total =
        initial_snap.suspended_algae_biomass_g + initial_snap.periphyton_biomass_g;
    let initial_health = initial_snap.root_feeding_rosette_health_index;

    let mut run = HarnessRun::from_state(definition.seed, definition.id, state)
        .with_artifact_label(definition.id);
    enable_instrumentation(&mut run);

    for _ in 0..90 {
        run.apply_action(PlayerAction::Feed { grams: 0.02 })?;
        run.step_hours(24)?;
    }

    let final_snap = run.snapshot();
    let final_plant = final_snap.total_plant_biomass_g;
    let final_algae_total = final_snap.suspended_algae_biomass_g + final_snap.periphyton_biomass_g;
    let final_health = final_snap.root_feeding_rosette_health_index;
    let observed = format!(
        "plants: {initial_plant:.2}g->{final_plant:.2}g health: {initial_health:.3}->{final_health:.3} | algae: {initial_algae_total:.3}g->{final_algae_total:.3}g nuisance: {:.3}->{:.3}",
        initial_snap.algae_nuisance_index,
        final_snap.algae_nuisance_index,
    );

    let state = run.engine().full_state();
    let fields = vec![
        classify_field_f64(
            "algae_fraction_of_initial",
            final_algae_total / initial_algae_total,
            0.5,
            f64::INFINITY,
        ),
        classify_field_f64(
            "plant_biomass_multiplier",
            final_plant / initial_plant,
            f64::NEG_INFINITY,
            1.2,
        ),
        classify_field_f64(
            "plant_health_drop",
            initial_health - final_health,
            0.001,
            f64::INFINITY,
        ),
    ];

    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "day_90",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![("competition_tank", run)],
    ))
}

fn calibrate_vs07(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs07_nitrate_removal_denitrification");
    const VS07_WATER_CHANGE_SOURCE: &str = "vs07_buffered_low_nitrate";

    let mut buffered_source = load_source_profile("hard_shrimp");
    buffered_source.nitrate_mg_n_per_l = 0.0;

    let normalize_fast_stems = |state: &mut TankState, biomass_g: f64| {
        for plant in &mut state.plant_guilds {
            match plant.guild {
                PlantGuild::FastStem => {
                    plant.biomass_g = biomass_g;
                    plant.health_index = 0.95;
                }
                _ => {
                    plant.biomass_g = 0.0;
                }
            }
        }
    };

    let prepare_vs07_biology = |state: &mut TankState| {
        normalize_fast_stems(state, 18.0);
        state.microbe.decomposer_biomass_g = 8.0;
        state.detritus.fine_detritus_g_total = 2.0;
        state.microfauna.population_index = 0.8;
        state.algae.set_periphyton_total(4.0);
        state.algae.suspended_biomass_g = 0.4;
    };

    let planted_overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlanted),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    let mut planted_state = tank_scenarios::seeded_state_with_full_overrides(
        definition.seed,
        "medium_planted",
        planted_overrides,
    )?;
    planted_state.source_water_catalog.insert(
        VS07_WATER_CHANGE_SOURCE.to_string(),
        buffered_source.clone(),
    );
    prepare_vs07_biology(&mut planted_state);
    planted_state.microbe.denitrifier_activity_index = 1.0;
    planted_state
        .process_params
        .denitrification_vmax_mg_n_per_l_per_hour = 0.2;
    planted_state
        .process_params
        .denitrification_pore_water_mixing_factor = 1.0;
    for layer in &mut planted_state.substrate_layers {
        layer.depth_cm = 10.0;
        layer.porosity = 0.32;
        layer.nutrient_store_mg_n_total = 0.0;
        layer.nutrient_store_mg_p_total = 0.0;
    }
    let vol = planted_state.water_volume_l();
    planted_state.water.dissolved_organic_carbon_mg_c_total = 12.0 * vol;
    planted_state.water.nitrate_mg_n_total = 12.0 * vol;
    planted_state.refresh_habitat_registry();

    let mut planted_run = HarnessRun::from_state(definition.seed, "vs07_planted", planted_state)
        .with_artifact_label("vs07_planted");
    enable_instrumentation(&mut planted_run);

    let bare_overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 2.0,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("hard_shrimp".to_string()),
        substrate_preset: Some(StartupSubstratePreset::InertSand),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours12),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(true),
        initial_adult_shrimp_count: Some(0),
        ..StartupOverrides::default()
    };
    let mut bare_state = tank_scenarios::seeded_state_with_full_overrides(
        definition.seed,
        "medium_planted",
        bare_overrides,
    )?;
    bare_state
        .source_water_catalog
        .insert(VS07_WATER_CHANGE_SOURCE.to_string(), buffered_source);
    prepare_vs07_biology(&mut bare_state);
    bare_state.substrate_layers.clear();
    bare_state
        .process_params
        .denitrification_vmax_mg_n_per_l_per_hour = 0.2;
    bare_state
        .process_params
        .denitrification_pore_water_mixing_factor = 1.0;
    let bare_vol = bare_state.water_volume_l();
    bare_state.water.nitrate_mg_n_total = 12.0 * bare_vol;
    bare_state.water.dissolved_organic_carbon_mg_c_total = 12.0 * bare_vol;
    bare_state.refresh_habitat_registry();

    let mut bare_run = HarnessRun::from_state(definition.seed, "vs07_bare", bare_state)
        .with_artifact_label("vs07_bare");
    enable_instrumentation(&mut bare_run);

    for day in 1..=120 {
        planted_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        bare_run.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        planted_run.step_hours(24)?;
        bare_run.step_hours(24)?;

        if day % 7 == 0 {
            planted_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: VS07_WATER_CHANGE_SOURCE.to_string(),
            })?;
            bare_run.apply_action(PlayerAction::WaterChangePercent {
                percent: 15.0,
                source_profile_id: VS07_WATER_CHANGE_SOURCE.to_string(),
            })?;
            planted_run.step_hours(1)?;
            bare_run.step_hours(1)?;
        }
    }

    let planted_snap = planted_run.snapshot();
    let bare_snap = bare_run.snapshot();
    let planted_n2 = planted_run.engine().full_state().cumulative_n2_export_mg_n;
    let bare_n2 = bare_run.engine().full_state().cumulative_n2_export_mg_n;
    let planted_denitrifier_act = planted_run
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;
    let observed = format!(
        "planted: NO3={:.2} N2_export={:.2}mg_N denitrifier_act={:.3} | bare: NO3={:.2} N2_export={:.2}mg_N",
        planted_snap.nitrate_mg_n_per_l,
        planted_n2,
        planted_denitrifier_act,
        bare_snap.nitrate_mg_n_per_l,
        bare_n2,
    );

    let state = planted_run.engine().full_state();
    let fields = vec![
        classify_field_f64(
            "bare_minus_planted_no3_mg_n_per_l",
            bare_snap.nitrate_mg_n_per_l - planted_snap.nitrate_mg_n_per_l,
            0.001,
            f64::INFINITY,
        ),
        classify_field_f64("planted_n2_export_mg_n", planted_n2, 1.0, f64::INFINITY),
        classify_field_f64("bare_n2_export_mg_n", bare_n2, f64::NEG_INFINITY, 0.1),
        classify_field_f64(
            "planted_denitrifier_activity",
            planted_denitrifier_act,
            0.1,
            f64::INFINITY,
        ),
    ];

    Ok(finalize_custom_row(
        definition,
        parameter_variant,
        "day_120",
        state.environment.day,
        state.environment.hour_of_day,
        fields,
        observed,
        vec![
            ("planted_substrate", planted_run),
            ("bare_bottom", bare_run),
        ],
    ))
}

fn calibrate_vs08(parameter_variant: &str) -> Result<ScenarioRow, Box<dyn Error>> {
    let definition = scenario_definition("vs08_stocking_density_crash");
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
    let mut state = TankState::new(definition.seed);
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
    state.microbe.set_decomposer_total(0.15);
    state.microbe.ammonia_oxidizer_biomass_g = 0.3;
    state.microbe.nitrite_oxidizer_biomass_g = 0.2;
    state.microbe.comammox_biomass_g = 0.1;
    state.filter_state.biofilter_maturity_index = 0.5;
    state.hardware.aeration.enabled = false;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.4;
    state.hardware.light.photoperiod_hours = 8.0;
    state.animal.adult.count = 20;
    state.animal.set_population_condition_index(0.6);
    state.animal.molt_stress_index = 0.2;
    state.animal.reproductive_readiness_index = 0.1;
    state.process_params = ProcessParams::default();
    state.shrimp_params.base_spawn_rate = 0.0;
    state.reseed_stability_tracker();

    let run = HarnessRun::from_state(definition.seed, definition.id, state)
        .with_artifact_label(definition.id);
    let mut calibration = CalibrationRun::new(run, parameter_variant);
    calibration.enable_instrumentation();
    let initial_count = calibration
        .inner()
        .engine()
        .full_state()
        .animal
        .total_count();

    for day in 1..=56 {
        calibration.apply_action(PlayerAction::Feed { grams: 0.5 })?;
        calibration.step_hours(24)?;

        if day % 7 == 0 {
            match day / 7 {
                1 => calibration.check_envelope(
                    "week_1",
                    &Envelope::default()
                        .tan_mg_n_per_l(1.0, 30.0)
                        .shrimp_count(5, 20)
                        .ph(5.0, 8.5),
                ),
                2 => calibration.check_envelope(
                    "week_2",
                    &Envelope::default()
                        .tan_mg_n_per_l(3.0, 60.0)
                        .shrimp_count(0, 15)
                        .ph(4.5, 8.5),
                ),
                4 => calibration.check_envelope(
                    "week_4",
                    &Envelope::default()
                        .tan_mg_n_per_l(5.0, 100.0)
                        .shrimp_count(0, 5)
                        .ph(4.5, 8.5),
                ),
                8 => calibration.check_envelope(
                    "week_8",
                    &Envelope::default()
                        .tan_mg_n_per_l(10.0, 400.0)
                        .shrimp_count(0, 0)
                        .ph(4.5, 8.5),
                ),
                _ => {}
            }
        }
    }

    let final_snap = calibration.inner().snapshot();
    if final_snap.total_shrimp_count > initial_count / 2 {
        calibration.inner_mut().record_failure(
            "population_crash",
            format!(
                "crash scenario should lose >=50% of population: initial={initial_count}, final={}",
                final_snap.total_shrimp_count
            ),
        );
    }
    let observed = format!(
        "initial={initial_count}, final={}, TAN={:.2}, NO2={:.2}, pH={:.3}",
        final_snap.total_shrimp_count,
        final_snap.tan_mg_n_per_l,
        final_snap.nitrite_mg_n_per_l,
        final_snap.ph,
    );

    Ok(annotate_row(definition, calibration.finish(), observed))
}

// ===========================================================================
// Summary runner: executes all 8 probes and prints a validation report
// ===========================================================================

type ProbeFn = Box<dyn Fn() -> Result<ProbeResult, Box<dyn Error>>>;

pub fn validation_suite_summary() {
    let probes: Vec<(&str, ProbeFn)> = vec![
        (
            "VS-01 Cycling timeline",
            Box::new(run_vs01_cycling_timeline),
        ),
        (
            "VS-02 Aeration effects",
            Box::new(|| run_vs02_aeration_effects().map_err(|e| e.into())),
        ),
        (
            "VS-03 Day/night pH swing",
            Box::new(|| run_vs03_day_night_ph_swing().map_err(|e| e.into())),
        ),
        (
            "VS-04 Source-water differentiation",
            Box::new(|| run_vs04_source_water_differentiation().map_err(|e| e.into())),
        ),
        ("VS-05 Shrimp breeding", Box::new(run_vs05_shrimp_breeding)),
        (
            "VS-06 Algae-plant competition",
            Box::new(|| run_vs06_algae_plant_competition().map_err(|e| e.into())),
        ),
        ("VS-07 Nitrate removal", Box::new(run_vs07_nitrate_removal)),
        (
            "VS-08 Stocking density crash",
            Box::new(run_vs08_stocking_density_crash),
        ),
    ];

    let mut results: Vec<ProbeResult> = Vec::new();
    let mut any_failed = false;

    eprintln!("\n============================================================");
    eprintln!("  VALIDATION SUITE — Tank Simulator");
    eprintln!("============================================================\n");

    for (label, runner) in &probes {
        let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(runner)) {
            Ok(Ok(probe)) => probe,
            Ok(Err(err)) => ProbeResult {
                name: label,
                passed: false,
                observed: String::new(),
                failure_detail: format!("error: {err}"),
            },
            Err(panic) => ProbeResult {
                name: label,
                passed: false,
                observed: String::new(),
                failure_detail: format!("panic: {}", panic_message(panic)),
            },
        };

        let status = if result.passed { "PASS" } else { "FAIL" };
        if !result.passed {
            any_failed = true;
        }
        eprintln!("  [{status}] {label}");
        if !result.observed.is_empty() {
            eprintln!("         {}", result.observed);
        }
        if !result.passed && !result.failure_detail.is_empty() {
            for line in result.failure_detail.lines().take(3) {
                eprintln!("         ! {line}");
            }
        }
        results.push(result);
    }

    let passed_count = results.iter().filter(|r| r.passed).count();
    let total = results.len();
    let high_total = validation_scenarios()
        .iter()
        .filter(|scenario| scenario.confidence == ValidationConfidence::High)
        .count();
    let high_passed = results
        .iter()
        .zip(validation_scenarios().iter())
        .filter(|(result, scenario)| {
            result.passed && scenario.confidence == ValidationConfidence::High
        })
        .count();
    let medium_total = validation_scenarios()
        .iter()
        .filter(|scenario| scenario.confidence == ValidationConfidence::Medium)
        .count();
    let medium_passed = results
        .iter()
        .zip(validation_scenarios().iter())
        .filter(|(result, scenario)| {
            result.passed && scenario.confidence == ValidationConfidence::Medium
        })
        .count();

    eprintln!("\n============================================================");
    eprintln!("  SUMMARY: {passed_count}/{total} scenarios passed");
    eprintln!("  High-confidence:   {high_passed}/{high_total} passed");
    eprintln!("  Medium-confidence: {medium_passed}/{medium_total} passed");
    eprintln!("============================================================\n");

    assert!(
        !any_failed,
        "{} of {total} validation scenarios failed",
        total - passed_count,
    );
}
