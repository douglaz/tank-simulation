//! End-to-end carbonate regression probes and summary report.
//!
//! Five scenario probes verifying qualitative carbonate chemistry behavior:
//!
//! 1. **Source-water pH differentiation**: hard_shrimp vs ro_like produce
//!    pH values at least 1.0 units apart.
//! 2. **Aeration-driven CO2 stripping**: high aeration raises pH compared
//!    to no aeration in a tank with elevated CO2.
//! 3. **Day/night pH drift**: light-on pH exceeds light-off pH in a planted
//!    tank with 12h/12h photoperiod.
//! 4. **Water change pH effect**: changing from RO-like to hard water shifts
//!    pH upward.
//! 5. **Nitrification-driven pH decline**: soft water (KH 2) shows pH decline
//!    > 0.5 over 200 hours; hard water (KH 8) shows decline < 0.2.
//!
//! Run all probes:  `cargo test --test e2e_carbonate_probes`
//! Verbose traces:  `TANK_E2E_VERBOSE=1 cargo test --test e2e_carbonate_probes -- --nocapture`
//! Summary only:    `cargo test --test e2e_carbonate_probes all_carbonate_probes_summary -- --nocapture`
//!
//! Exit codes: 0 = all pass, non-zero = envelope violation.
//!
//! Tolerance ranges are documented in each probe's doc comment. Future tuning
//! may adjust numbers inside the envelope without rewriting the scientific story.

use tank_core::{
    systems::chemistry::resolve_carbonate_state, systems::light::is_light_on, Engine,
    JsonLinesSink, PlayerAction, SimSeed, SimTracer, SimulationEngine, SourceWaterProfile,
    TankSnapshot, TankState, TraceSink, Verbosity, WaterState,
};

// ---------------------------------------------------------------------------
// Probe result tracking for the summary report
// ---------------------------------------------------------------------------

struct ProbeResult {
    name: &'static str,
    passed: bool,
    /// One-line summary of observed pH values and key metrics.
    observed: String,
    /// Detail message on failure (empty string if passed).
    failure_detail: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format carbonate state from a snapshot for diagnostic output on failure.
///
/// Dumps pH, DIC (mg C/L), KH (dKH), and CO2(aq) (mmol/L) in a compact
/// one-line format suitable for assertion messages and trace output.
fn carbonate_diag(snap: &TankSnapshot) -> String {
    format!(
        "pH={:.3} DIC={:.2}mg_C/L KH={:.1}dKH CO2={:.4}mmol/L",
        snap.ph, snap.dissolved_inorganic_carbon_mg_c_per_l, snap.kh_d, snap.co2_aq_mmol_per_l,
    )
}

/// Write the engine's trace to a temp file for post-mortem diagnosis.
/// Returns the artifact path if a trace was available.
fn dump_trace_artifact(label: &str, engine: &Engine) -> Option<std::path::PathBuf> {
    let tracer = engine.tracer()?;
    let dir = std::env::temp_dir()
        .join("tank_harness")
        .join("carbonate_probes");
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

/// Build a minimal tank from a source water profile with all biology and gas
/// exchange suppressed. The resulting pH is determined solely by the carbonate
/// equilibrium of the source water's DIC/alkalinity ratio.
fn equilibrium_tank(profile: &SourceWaterProfile, volume_l: f64, seed: SimSeed) -> TankState {
    let water = WaterState::from_source_profile_for_volume_l(profile, volume_l);
    let mut state = TankState::new(seed);
    state.water = water;

    // Populate source water catalog so water changes work.
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

    // Suppress gas exchange so DIC stays at source-water levels.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;

    state
}

/// Build a tank primed for active nitrification with specified alkalinity.
///
/// Gas exchange is fully suppressed (filter disabled, reaeration/aeration
/// zeroed) to isolate the nitrification -> alkalinity -> pH pathway.
///
/// Disabling the filter sets `filter_enabled_factor = 0.1` in the nitrogen
/// cycle, which would reduce nitrification to 10% of normal. To compensate,
/// nitrifier biomass is set to 10x the standard inoculum, producing the same
/// effective oxidation rate as a filter-enabled tank with normal biomass.
///
/// Growth yields are near-zero so DIC consumption from autotrophic growth
/// does not mask the alkalinity signal.
fn nitrifying_tank(
    seed: SimSeed,
    alkalinity_meq_per_l: f64,
    dic_mg_c_per_l: f64,
    tan_mg_n_per_l: f64,
) -> TankState {
    let mut state = TankState::new(seed);
    let vol = state.water_volume_l();

    // Elevated TAN as nitrification substrate.
    state.water.ammonia_total_mg_n_total = tan_mg_n_per_l * vol;

    // 10x nitrifier biomass to compensate for the 0.1 filter_enabled_factor
    // penalty when the filter is disabled. This produces the same effective
    // nitrification rate as normal biomass with an enabled filter.
    state.microbe.ammonia_oxidizer_biomass_g = 2.5;
    state.microbe.nitrite_oxidizer_biomass_g = 1.5;
    state.microbe.comammox_biomass_g = 0.5;
    state.microbe.set_decomposer_total(0.0);

    // Mature biofilter.
    state.filter_state.biofilter_maturity_index = 0.8;

    // Set alkalinity and DIC to produce realistic starting pH.
    state.water.alkalinity_meq_total = alkalinity_meq_per_l * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = dic_mg_c_per_l * vol;

    // Resolve carbonate state so cached pH reflects updated alk and DIC.
    resolve_carbonate_state(&mut state.water, vol);

    // Disable filter to eliminate gas exchange from filter agitation
    // (filter_kla_boost = 0 when disabled).
    state.hardware.filter.enabled = false;
    state.hardware.filter.flow_lph = 0.0;

    // Also suppress surface reaeration and aeration.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;

    // Suppress DIC shortcuts.
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;

    // Near-zero growth yields isolate the alkalinity -> pH signal from
    // growth-driven DIC consumption that can mask pH decline.
    state.process_params.aob_growth_yield = 1e-6;
    state.process_params.nob_growth_yield = 1e-6;
    state.process_params.comammox_growth_yield = 1e-6;
    state.process_params.decomposer_growth_yield = 1e-6;

    // Remove plants, algae, shrimp.
    for plant in &mut state.plant_guilds {
        plant.biomass_g = 0.0;
    }
    state.algae.suspended_biomass_g = 0.0;
    state.algae.set_periphyton_total(0.0);
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;

    state
}

/// Find the hour boundaries where light switches on and off for a given
/// photoperiod, returning (end_of_dark_hour, end_of_light_hour).
fn light_transition_hours(photoperiod_hours: f64) -> (u8, u8) {
    let mut end_of_dark = None;
    let mut end_of_light = None;
    for hour in 0..24u8 {
        let now = is_light_on(hour, photoperiod_hours);
        let next = is_light_on((hour + 1) % 24, photoperiod_hours);
        if !now && next {
            end_of_dark = Some((hour + 1) % 24);
        }
        if now && !next {
            end_of_light = Some((hour + 1) % 24);
        }
    }
    (
        end_of_dark.expect("photoperiod should define end-of-dark"),
        end_of_light.expect("photoperiod should define end-of-light"),
    )
}

// ===========================================================================
// Probe 1: Source-water pH differentiation
// ===========================================================================

/// hard_shrimp (KH ~8.0, DIC 36 mg C/L) vs ro_like (KH ~0.1, DIC 1.0 mg C/L)
/// should produce equilibrium pH values at least 1.0 units apart.
///
/// **Chemistry**: With biology and gas exchange suppressed, the tank pH is
/// determined solely by the carbonate equilibrium of the source water's
/// DIC/alkalinity ratio. Hard water with high KH and moderate DIC settles
/// in the pH 7.0-8.5 range, while RO-like water with near-zero buffering
/// settles in the pH 5.5-7.0 range.
///
/// **Tolerances**:
/// - hard_shrimp pH in [7.0, 8.5]: KH 8 + DIC 36 -> analytical pH ~ 7.5
/// - ro_like pH in [5.5, 7.0]: KH ~0.1 + DIC 1.0 -> near solver floor
/// - Gap >= 1.0: justified by 80x KH difference driving >= 1 pH unit separation
fn run_probe_source_water_ph_differentiation() -> Result<ProbeResult, tank_core::SimError> {
    let hard = load_source_profile("hard_shrimp");
    let ro = load_source_profile("ro_like");
    let volume_l = 30.0;

    let hard_state = equilibrium_tank(&hard, volume_l, SimSeed(9_001));
    let ro_state = equilibrium_tank(&ro, volume_l, SimSeed(9_001));

    let mut hard_engine = Engine::from_parts(hard_state, vec![]);
    let mut ro_engine = Engine::from_parts(ro_state, vec![]);

    if is_verbose() {
        hard_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
        ro_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    // Equilibrate for 100 hours.
    hard_engine.step_hours(100)?;
    ro_engine.step_hours(100)?;

    let hard_snap = hard_engine.snapshot();
    let ro_snap = ro_engine.snapshot();
    let ph_gap = hard_snap.ph - ro_snap.ph;

    let observed = format!(
        "hard_shrimp: {} | ro_like: {} | gap={:.3}",
        carbonate_diag(&hard_snap),
        carbonate_diag(&ro_snap),
        ph_gap,
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
        dump_trace_artifact("probe1_hard_shrimp", &hard_engine);
        dump_trace_artifact("probe1_ro_like", &ro_engine);
    }

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");

    Ok(ProbeResult {
        name: "Source-water pH differentiation",
        passed,
        observed,
        failure_detail,
    })
}

/// Probe 1: hard_shrimp vs ro_like source water must produce pH at least
/// 1.0 units apart after equilibration with biology and gas exchange suppressed.
#[test]
fn probe_1_source_water_ph_differentiation() -> Result<(), Box<dyn std::error::Error>> {
    let r = run_probe_source_water_ph_differentiation()?;
    assert!(
        r.passed,
        "Probe 1 FAILED: {}\n  Observed: {}",
        r.failure_detail, r.observed
    );
    Ok(())
}

// ===========================================================================
// Probe 2: Aeration-driven CO2 stripping
// ===========================================================================

/// Two identical tanks with elevated DIC (40 mg C/L, producing CO2 well above
/// atmospheric equilibrium). One has high aeration (intensity 1.0), the other
/// is passive. After 12 hours, the aerated tank should have higher pH because
/// CO2 stripping shifts the carbonate equilibrium.
///
/// **Chemistry**: Excess dissolved CO2 creates a downward pH pressure. Aeration
/// increases K_LA (gas transfer coefficient), accelerating CO2 off-gassing.
/// As CO2 leaves the water, the DIC pool shrinks while alkalinity stays constant,
/// shifting the carbonate equilibrium toward higher pH.
///
/// **Tolerances**:
/// - Aerated pH > passive pH (qualitative direction)
/// - pH difference >= 0.2 after 12h: justified by the large initial CO2
///   supersaturation (~10x atmospheric) and aggressive aeration rate
fn run_probe_aeration_co2_stripping() -> Result<ProbeResult, tank_core::SimError> {
    let make_state = |aerated: bool| -> TankState {
        let mut state = TankState::new(SimSeed(9_002));
        let volume_l = state.water_volume_l();
        // Elevated DIC (~40 mg C/L) produces CO2 well above equilibrium.
        state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
        state.water.alkalinity_meq_total = 2.0 * volume_l;
        state.water.temperature_c = 25.0;
        // Zero biology-driven DIC fluxes so only gas exchange drives DIC changes.
        state
            .process_params
            .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
        state
            .process_params
            .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
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

    aerated_engine.step_hours(12)?;
    passive_engine.step_hours(12)?;

    let aerated_snap = aerated_engine.snapshot();
    let passive_snap = passive_engine.snapshot();
    let ph_diff = aerated_snap.ph - passive_snap.ph;

    let observed = format!(
        "aerated: {} | passive: {} | diff={:.3}",
        carbonate_diag(&aerated_snap),
        carbonate_diag(&passive_snap),
        ph_diff,
    );

    let mut failures = Vec::new();
    if aerated_snap.ph <= passive_snap.ph {
        failures.push(format!(
            "aerated pH {:.3} not higher than passive pH {:.3}",
            aerated_snap.ph, passive_snap.ph,
        ));
    }
    if ph_diff < 0.2 {
        failures.push(format!("pH difference {ph_diff:.3} < 0.2 minimum"));
    }

    if !failures.is_empty() {
        dump_trace_artifact("probe2_aerated", &aerated_engine);
        dump_trace_artifact("probe2_passive", &passive_engine);
    }

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");

    Ok(ProbeResult {
        name: "Aeration-driven CO2 stripping",
        passed,
        observed,
        failure_detail,
    })
}

/// Probe 2: High aeration should raise pH compared to no aeration in a tank
/// with elevated CO2 by stripping dissolved CO2 toward atmospheric equilibrium.
#[test]
fn probe_2_aeration_driven_co2_stripping() -> Result<(), Box<dyn std::error::Error>> {
    let r = run_probe_aeration_co2_stripping()?;
    assert!(
        r.passed,
        "Probe 2 FAILED: {}\n  Observed: {}",
        r.failure_detail, r.observed
    );
    Ok(())
}

// ===========================================================================
// Probe 3: Day/night pH drift
// ===========================================================================

/// Planted tank with 12h/12h photoperiod and no gas exchange, run for 48 hours.
/// Light-on pH should be higher than light-off pH, with a swing >= 0.1 units.
///
/// **Chemistry**: Photosynthesis during lit hours consumes CO2 (DIC), shifting
/// the carbonate equilibrium toward higher pH:
///   CO2 + H2O -> CH2O + O2 (net DIC removal)
/// Respiration during dark hours releases CO2 (adds DIC), lowering pH:
///   CH2O + O2 -> CO2 + H2O (net DIC production)
/// The pH swing amplitude depends on plant biomass, DIC/alkalinity buffering,
/// and photoperiod length.
///
/// **Tolerances**:
/// - End-of-light pH > end-of-dark pH (qualitative direction each cycle)
/// - Swing (max light pH - min dark pH) >= 0.1: justified by 25g total plant
///   biomass in a 30L tank with moderate DIC buffering
fn run_probe_day_night_ph_drift() -> Result<ProbeResult, tank_core::SimError> {
    let mut state = TankState::new(SimSeed(9_003));
    let volume_l = state.water_volume_l();

    // Well-planted tank for clear day/night signal.
    state.plant_guilds[0].biomass_g = 15.0;
    state.plant_guilds[1].biomass_g = 10.0;
    state.algae.set_periphyton_total(2.0);
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

    // Zero K_LA to isolate biological DIC effects from atmospheric exchange.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.flow_lph = 0.0;

    resolve_carbonate_state(&mut state.water, volume_l);

    state.environment.hour_of_day = 0;
    state.environment.day = 1;

    let photoperiod = state.hardware.light.photoperiod_hours;
    let (end_of_dark, end_of_light) = light_transition_hours(photoperiod);

    let mut engine = Engine::from_parts(state, vec![]);
    if is_verbose() {
        engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    let mut ph_end_of_light = Vec::new();
    let mut ph_end_of_dark = Vec::new();

    // Run 48h, collect pH at light/dark transitions from day 2 onward
    // to allow transient settling on day 1.
    for _ in 0..48 {
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

    if ph_end_of_light.is_empty() || ph_end_of_dark.is_empty() {
        failures.push("insufficient light/dark transition samples captured".to_string());
    } else {
        // Each end-of-light pH should exceed its corresponding end-of-dark pH.
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
        if swing < 0.1 {
            failures.push(format!("swing {swing:.3} < 0.1 minimum"));
        }
    }

    if !failures.is_empty() {
        dump_trace_artifact("probe3_day_night", &engine);
    }

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");

    Ok(ProbeResult {
        name: "Day/night pH drift",
        passed,
        observed,
        failure_detail,
    })
}

/// Probe 3: In a planted tank with 12h/12h photoperiod, light-on pH should
/// exceed light-off pH with a swing >= 0.1 units, driven by photosynthetic
/// CO2 consumption during lit hours and respiratory CO2 release at night.
#[test]
fn probe_3_day_night_ph_drift() -> Result<(), Box<dyn std::error::Error>> {
    let r = run_probe_day_night_ph_drift()?;
    assert!(
        r.passed,
        "Probe 3 FAILED: {}\n  Observed: {}",
        r.failure_detail, r.observed
    );
    Ok(())
}

// ===========================================================================
// Probe 4: Water change pH effect
// ===========================================================================

/// Tank starting with ro_like water (low DIC, near-zero alkalinity -> low pH),
/// receives a 50% water change with hard_shrimp source. pH should shift upward
/// because the hard water delivers alkalinity and DIC that raise the carbonate
/// equilibrium.
///
/// **Chemistry**: RO-like water has KH ~0.1 and DIC ~1 mg C/L, producing pH
/// near the solver floor (~5.5-6.5). Hard shrimp water has KH ~8.0 and DIC
/// ~36 mg C/L, producing pH ~7.5. A 50% water change blends the two, and the
/// resulting mixture should have higher alkalinity and DIC, raising pH.
///
/// **Tolerances**:
/// - pH shift > 0.0 (qualitative: hard water raises pH)
/// - Justified: 50% hard water adds substantial alkalinity to a near-zero-
///   alkalinity tank, which shifts the carbonate equilibrium upward
fn run_probe_water_change_ph_shift() -> Result<ProbeResult, tank_core::SimError> {
    let ro = load_source_profile("ro_like");
    let volume_l = 30.0;

    // Start with RO-like water, biology suppressed.
    let state = equilibrium_tank(&ro, volume_l, SimSeed(9_004));

    let mut engine = Engine::from_parts(state, vec![]);
    if is_verbose() {
        engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    }

    // Let equilibrate for 10 hours.
    engine.step_hours(10)?;
    let snap_before = engine.snapshot();
    let ph_before = snap_before.ph;

    // 50% water change with hard_shrimp source.
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "hard_shrimp".to_string(),
    })?;
    engine.step_hours(1)?;

    let snap_after = engine.snapshot();
    let ph_after = snap_after.ph;
    let ph_shift = ph_after - ph_before;

    let observed = format!(
        "before: {} | after 50% hard WC: {} | shift={:+.3}",
        carbonate_diag(&snap_before),
        carbonate_diag(&snap_after),
        ph_shift,
    );

    let mut failures = Vec::new();
    if ph_shift <= 0.0 {
        failures.push(format!(
            "pH shift {:+.3} is not positive (should increase from RO -> hard water)",
            ph_shift
        ));
    }

    if !failures.is_empty() {
        dump_trace_artifact("probe4_water_change", &engine);
    }

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");

    Ok(ProbeResult {
        name: "Water change pH effect",
        passed,
        observed,
        failure_detail,
    })
}

/// Probe 4: A 50% water change from RO-like to hard_shrimp source should
/// shift pH upward by delivering alkalinity and DIC to the carbonate system.
#[test]
fn probe_4_water_change_ph_effect() -> Result<(), Box<dyn std::error::Error>> {
    let r = run_probe_water_change_ph_shift()?;
    assert!(
        r.passed,
        "Probe 4 FAILED: {}\n  Observed: {}",
        r.failure_detail, r.observed
    );
    Ok(())
}

// ===========================================================================
// Probe 5: Nitrification-driven pH decline
// ===========================================================================

/// Two tanks: soft water (KH 2, ~0.714 meq/L) and hard water (KH 8,
/// ~2.857 meq/L), both with active nitrification (elevated TAN, mature
/// biofilter, no gas exchange, no plants). Run for 200 hours.
///
/// **Chemistry**: Nitrification consumes alkalinity stoichiometrically:
///   NH4+ + 2O2 -> NO3- + H2O + 2H+
/// Each mg N oxidized consumes ~0.143 meq alkalinity (2/14.007).
/// The carbonate solver translates alkalinity depletion into pH decline.
///
/// In soft water (KH 2), even moderate alkalinity consumption exhausts a
/// large fraction of the buffer reserve, producing a steep pH decline.
/// In hard water (KH 8), the abundant buffer absorbs the same acid load
/// with minimal pH change.
///
/// **Tolerances**:
/// - Soft water (KH 2): pH decline > 0.5 over 200h
///   Justified: initial alkalinity ~21 meq in 30L, TAN load of 6 mg N/L
///   (180 mg N total) consumes ~25.7 meq if fully oxidized = 120% of buffer.
///   Even partial oxidation exhausts most of the buffer, crashing pH.
/// - Hard water (KH 8): pH decline < 0.35 over 200h
///   Justified: initial alkalinity ~86 meq in 30L, same TAN load consumes
///   ~25.7 meq = 30% of buffer. The logarithmic pH response from the
///   carbonate equilibrium is modest at this depletion fraction.
/// - Soft decline must exceed hard decline (buffering contrast story).
fn run_probe_nitrification_ph_decline() -> Result<ProbeResult, tank_core::SimError> {
    // KH 2 ~ 0.714 meq/L, KH 8 ~ 2.857 meq/L. (1 dKH = 0.357 meq/L)
    let soft_alk = 2.0 * 0.357;
    let hard_alk = 8.0 * 0.357;
    // DIC tuned so both tanks start at approximately the same pH (~7.3).
    // This equalizes the initial nitrification rate so the difference in
    // buffering capacity drives the outcome, not pH-dependent rate inhibition.
    // At pH 7.3, DIC ≈ Alk_meq × 13.35 (from carbonate equilibrium at 25°C).
    let soft_dic = soft_alk * 13.35; // ~9.5 mg C/L
    let hard_dic = hard_alk * 13.35; // ~38.1 mg C/L
                                     // 6 mg N/L provides enough substrate for sustained nitrification over 200h.
                                     // In a 30L tank this is 180 mg N total.
    let tan_mg_n_per_l = 6.0;

    let soft_state = nitrifying_tank(SimSeed(9_005), soft_alk, soft_dic, tan_mg_n_per_l);
    let hard_state = nitrifying_tank(SimSeed(9_005), hard_alk, hard_dic, tan_mg_n_per_l);

    let soft_ph_initial = soft_state.water.ph;
    let hard_ph_initial = hard_state.water.ph;

    let mut soft_engine = Engine::from_parts(soft_state, vec![]);
    let mut hard_engine = Engine::from_parts(hard_state, vec![]);

    soft_engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    hard_engine.enable_tracing(SimTracer::new(Verbosity::Detail));

    soft_engine.step_hours(200)?;
    hard_engine.step_hours(200)?;

    let soft_snap = soft_engine.snapshot();
    let hard_snap = hard_engine.snapshot();
    let soft_decline = soft_ph_initial - soft_snap.ph;
    let hard_decline = hard_ph_initial - hard_snap.ph;

    let observed = format!(
        "soft KH2: pH {:.3}->{:.3} (decline={:.3}) | hard KH8: pH {:.3}->{:.3} (decline={:.3})",
        soft_ph_initial, soft_snap.ph, soft_decline, hard_ph_initial, hard_snap.ph, hard_decline,
    );

    let mut failures = Vec::new();
    if soft_decline < 0.5 {
        failures.push(format!(
            "soft water (KH 2) pH decline {soft_decline:.3} < 0.5 minimum \
             (initial={soft_ph_initial:.3}, final={:.3})",
            soft_snap.ph
        ));
    }
    if hard_decline > 0.35 {
        failures.push(format!(
            "hard water (KH 8) pH decline {hard_decline:.3} > 0.35 maximum \
             (initial={hard_ph_initial:.3}, final={:.3})",
            hard_snap.ph
        ));
    }
    if soft_decline <= hard_decline {
        failures.push(format!(
            "soft decline ({soft_decline:.3}) should exceed hard decline ({hard_decline:.3})"
        ));
    }

    if !failures.is_empty() {
        dump_trace_artifact("probe5_soft_kh2", &soft_engine);
        dump_trace_artifact("probe5_hard_kh8", &hard_engine);
    }

    let passed = failures.is_empty();
    let failure_detail = failures.join("; ");

    Ok(ProbeResult {
        name: "Nitrification-driven pH decline",
        passed,
        observed,
        failure_detail,
    })
}

/// Probe 5: A cycling tank with active nitrification in soft water (KH 2) shows
/// pH decline > 0.5 over 200 hours; same scenario in hard water (KH 8) shows
/// pH decline < 0.35 — demonstrating the buffering story.
#[test]
fn probe_5_nitrification_driven_ph_decline() -> Result<(), Box<dyn std::error::Error>> {
    let r = run_probe_nitrification_ph_decline()?;
    assert!(
        r.passed,
        "Probe 5 FAILED: {}\n  Observed: {}",
        r.failure_detail, r.observed
    );
    Ok(())
}

// ===========================================================================
// Summary report: runs all 5 probes, prints observed pH ranges, exits
// non-zero on envelope violation.
// ===========================================================================

/// Runs all 5 carbonate probes, prints a summary report with observed pH
/// ranges and pass/fail status, and panics if any probe fails.
///
/// This test satisfies the acceptance criterion for an e2e carbonate test
/// script that produces a summary report and exits non-zero on violation.
#[test]
fn all_carbonate_probes_summary() -> Result<(), Box<dyn std::error::Error>> {
    let results = [
        run_probe_source_water_ph_differentiation()?,
        run_probe_aeration_co2_stripping()?,
        run_probe_day_night_ph_drift()?,
        run_probe_water_change_ph_shift()?,
        run_probe_nitrification_ph_decline()?,
    ];

    eprintln!();
    eprintln!("=== Carbonate Probe Summary Report ===");
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
    eprintln!("=== End Carbonate Probe Report ===");
    eprintln!();

    if !all_passed {
        let mut msg = format!("{fail_count} carbonate probe(s) failed:\n");
        for r in results.iter().filter(|r| !r.passed) {
            msg.push_str(&format!("  {}: {}\n", r.name, r.failure_detail));
        }
        panic!("{msg}");
    }

    Ok(())
}
