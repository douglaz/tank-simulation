//! Habitat/geometry scenario probes (tanksim-6e5.5.7).
//!
//! Five scenario-level tests that confirm geometry and habitat abstractions
//! change behavior for mechanistic ecological reasons, not just because
//! parameters were retuned.
//!
//! Each probe is a named test function with a doc comment explaining the
//! ecological mechanism being validated. Assertions use directional comparisons
//! (A > B) and envelope ranges so the tests remain resilient to reasonable
//! parameter retuning.
//!
//! On failure, habitat-specific state (per-habitat biomass, area, exposure
//! modifiers) is dumped via a structured debug helper for diagnosis.
//!
//! Set `TANK_E2E_VERBOSE=1` for full trace dumps even on success.
//!
//! Default file runs execute the aggregate summary once:
//! `cargo test --test e2e_habitat_geometry_probes -- --nocapture`
//!
//! Individual probe entrypoints are `#[ignore]` so the summary runner does not
//! double the wall-clock cost in default CI runs. Run a single probe explicitly
//! with `-- --ignored`, for example:
//! `cargo test --test e2e_habitat_geometry_probes probe_substrate_redox_denitrification -- --ignored --nocapture`

use tank_core::{
    systems::{
        chemistry::resolve_carbonate_state, nitrogen_cycle::compute_biofilter_carrying_capacity,
    },
    HabitatKind, PlantGuild, PlayerAction, SimSeed, SimulationEngine, SubstrateKind,
    SubstrateLayerState, TankSnapshot, TankState, WaterState,
};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

// ---------------------------------------------------------------------------
// Debug helpers — dump habitat-specific state on failure
// ---------------------------------------------------------------------------

/// Format per-habitat biomass, area, and exposure modifiers for diagnostic
/// output when an assertion fails.
fn format_habitat_debug(state: &TankState) -> String {
    let mut out = String::new();
    out.push_str("=== Habitat Registry ===\n");
    for entry in &state.habitat_registry {
        out.push_str(&format!(
            "  {:?}: area={:.1} cm², flow={:.3}, O₂={:.3}, light={:.3}\n",
            entry.kind,
            entry.colonizable_area_cm2,
            entry.flow_exposure,
            entry.oxygen_exposure,
            entry.light_exposure,
        ));
    }

    out.push_str("=== Periphyton by habitat ===\n");
    for (kind, biomass) in &state.algae.periphyton_by_habitat {
        out.push_str(&format!("  {:?}: {:.6} g\n", kind, biomass));
    }
    out.push_str(&format!(
        "  total: {:.6} g\n",
        state.algae.periphyton_biomass_g
    ));

    out.push_str("=== Decomposer by habitat ===\n");
    for (kind, biomass) in &state.microbe.decomposer_by_habitat {
        out.push_str(&format!("  {:?}: {:.6} g\n", kind, biomass));
    }
    out.push_str(&format!(
        "  total: {:.6} g\n",
        state.microbe.decomposer_biomass_g
    ));

    out.push_str("=== Nitrifier biomass ===\n");
    out.push_str(&format!(
        "  AOB: {:.6} g, NOB: {:.6} g, comammox: {:.6} g\n",
        state.microbe.ammonia_oxidizer_biomass_g,
        state.microbe.nitrite_oxidizer_biomass_g,
        state.microbe.comammox_biomass_g,
    ));

    out.push_str("=== Filter state ===\n");
    out.push_str(&format!(
        "  maturity: {:.4}, clogging: {:.4}, media_area: {:.0} cm²\n",
        state.filter_state.biofilter_maturity_index,
        state.filter_state.clogging_index,
        state.hardware.filter.media_area_cm2,
    ));

    out.push_str("=== Substrate ===\n");
    for (i, layer) in state.substrate_layers.iter().enumerate() {
        out.push_str(&format!(
            "  layer {}: {:?} depth={:.1} cm, O₂_pen={:.2} cm, porosity={:.2}\n",
            i, layer.kind, layer.depth_cm, layer.o2_penetration_depth_cm, layer.porosity,
        ));
    }
    out.push_str(&format!(
        "  total_depth: {:.1} cm, O₂_pen_shared: {:.2} cm\n",
        state.substrate_depth_cm(),
        state.substrate_o2_penetration_depth_cm(),
    ));
    out.push_str(&format!(
        "  suboxic_pore_vol: {:.2} cm³\n",
        state.substrate_suboxic_pore_volume_cm3(),
    ));

    out.push_str("=== Denitrification ===\n");
    out.push_str(&format!(
        "  activity_index: {:.4}, cumulative_n2_export: {:.4} mg N\n",
        state.microbe.denitrifier_activity_index, state.cumulative_n2_export_mg_n,
    ));

    out.push_str("=== Geometry ===\n");
    out.push_str(&format!(
        "  L×W×H: {:.0}×{:.0}×{:.0} cm, fill: {:.0} cm\n",
        state.geometry.length_cm,
        state.geometry.width_cm,
        state.geometry.height_cm,
        state.geometry.fill_height_cm,
    ));
    out.push_str(&format!(
        "  volume: {:.2} L, footprint: {:.0} cm²\n",
        state.water_volume_l(),
        state.geometry.footprint_area_cm2(),
    ));

    out
}

/// Print habitat debug info to stderr (useful during test diagnosis).
fn dump_habitat_debug(label: &str, state: &TankState) {
    eprintln!("--- {} ---\n{}", label, format_habitat_debug(state));
}

/// Format a snapshot's key chemistry and biology values.
fn format_snapshot_summary(label: &str, snap: &TankSnapshot) -> String {
    format!(
        "{}: pH={:.2}, TAN={:.4}, NO2={:.4}, NO3={:.2}, DO={:.2}, \
         plants={:.2}g, periphyton={:.4}g, algae_nuisance={:.3}, \
         biofilter_mat={:.3}, shrimp={}, temp={:.1}C, vol={:.1}L",
        label,
        snap.ph,
        snap.tan_mg_n_per_l,
        snap.nitrite_mg_n_per_l,
        snap.nitrate_mg_n_per_l,
        snap.do_mg_l,
        snap.total_plant_biomass_g,
        snap.periphyton_biomass_g,
        snap.algae_nuisance_index,
        snap.biofilter_maturity_index,
        snap.total_shrimp_count,
        snap.water_temp_c,
        snap.water_volume_l,
    )
}

fn format_clearance_summary(small: Option<u32>, large: Option<u32>) -> String {
    match (small, large) {
        (Some(small_hour), Some(large_hour)) => format!("{small_hour}h->{large_hour}h"),
        (Some(small_hour), None) => format!("{small_hour}h->not-cleared"),
        (None, Some(large_hour)) => format!("not-cleared->{large_hour}h"),
        (None, None) => "not-cleared->not-cleared (terminal reduced-N fallback)".to_string(),
    }
}

struct ProbeResult {
    name: &'static str,
    passed: bool,
    observed: String,
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

fn record_failure_all(runs: &mut [&mut HarnessRun], label: &str, message: String) {
    for run in runs.iter_mut() {
        run.record_failure(label, message.clone());
    }
}

fn record_check(runs: &mut [&mut HarnessRun], label: &str, condition: bool, message: String) {
    if !condition {
        record_failure_all(runs, label, message);
    }
}

fn record_within_fraction(
    runs: &mut [&mut HarnessRun],
    label: &str,
    lhs: f64,
    rhs: f64,
    tolerance: f64,
) {
    let scale = lhs.abs().max(rhs.abs()).max(1e-9);
    let fraction = (lhs - rhs).abs() / scale;
    record_check(
        runs,
        label,
        fraction <= tolerance,
        format!(
            "{label}: values should stay within {:.0}% \
             (lhs={lhs:.4}, rhs={rhs:.4}, divergence={:.1}%)",
            tolerance * 100.0,
            fraction * 100.0
        ),
    );
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

// ---------------------------------------------------------------------------
// 1. Biofilter scaling: bigger filter media → higher nitrifier capacity
//    → lower reduced-nitrogen burden (and faster clearance when it occurs)
// ---------------------------------------------------------------------------

/// Biofilter scaling scenario: substantially larger filter media should
/// produce a larger nitrifier population and more filter-media colonizable
/// area.
///
/// **Ecological mechanism:** Filter media is the primary colonizable surface
/// for ammonia-oxidizing bacteria (AOB) and nitrite-oxidizing bacteria (NOB).
/// More media area means a higher carrying capacity for nitrifiers, which
/// allows a larger nitrifier population to establish. The habitat registry
/// should reflect the larger media pack as substantially more `FilterMedia`
/// colonizable area, and the nitrifier population should respond accordingly.
///
/// **What we validate:**
/// - Tank with larger media has substantially more filter habitat area and
///   higher nitrifier carrying capacity
/// - Under the same ammonia challenge, the larger filter clears combined
///   reduced nitrogen (TAN + NO₂) at least as fast, or if neither run fully
///   clears the pulse inside the horizon, it still ends with lower reduced-N
///   plus more nitrifier biomass from post-seeding growth
/// - Both tanks remain within basic stability envelopes
// Individual probes stay ignored by default because the summary runner below
// already executes each multi-day scenario once in ordinary `cargo test` runs.
#[test]
#[ignore = "run explicitly with -- --ignored when iterating on a single habitat probe"]
fn probe_biofilter_scaling_bigger_media_higher_capacity() -> Result<(), Box<dyn std::error::Error>>
{
    require_probe_pass(run_probe_biofilter_scaling_bigger_media_higher_capacity())
}

fn run_probe_biofilter_scaling_bigger_media_higher_capacity(
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_biofilter_scaling_bigger_media_higher_capacity_with_suffix(None)
}

fn run_probe_biofilter_scaling_bigger_media_higher_capacity_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    const SMALL_MEDIA_CM2: f64 = 500.0;
    const LARGE_MEDIA_CM2: f64 = 4000.0;
    const INITIAL_CAPACITY_FRACTION: f64 = 0.35;
    const INITIAL_TAN_MG_N_PER_L: f64 = 4.0;
    const DURATION_HOURS: u32 = 24 * 28;
    const REDUCED_N_CLEARANCE_THRESHOLD: f64 = 1.0;

    let base_state = |seed: SimSeed, media_area_cm2: f64| -> TankState {
        let mut state = TankState::new(seed);
        state.geometry.length_cm = 60.0;
        state.geometry.width_cm = 30.0;
        state.geometry.height_cm = 36.0;
        state.geometry.fill_height_cm = 32.0;
        let footprint_cm2 = state.geometry.footprint_area_cm2();
        let mut substrate = SubstrateLayerState::default();
        substrate.colonizable_area_cm2 = substrate.derived_colonizable_area_cm2(footprint_cm2);
        state.substrate_layers = vec![substrate];

        let volume_l = state.water_volume_l();
        state.water = WaterState::default_for_volume_l(volume_l);
        state.water.temperature_c = 25.0;
        state.environment.ambient_temp_c = 25.0;
        state.water.ammonia_total_mg_n_total = INITIAL_TAN_MG_N_PER_L * volume_l;
        state.water.dissolved_oxygen_mg_total = 8.5 * volume_l;
        state.water.alkalinity_meq_total = 8.0 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 35.0 * volume_l;
        resolve_carbonate_state(&mut state.water, volume_l);

        state.hardware.filter.enabled = true;
        state.hardware.filter.media_area_cm2 = media_area_cm2;
        state.hardware.filter.flow_lph = 400.0;
        state.hardware.filter.cleanliness_index = 1.0;
        state.filter_state.clogging_index = 0.0;
        state.filter_state.biofilter_maturity_index = INITIAL_CAPACITY_FRACTION;
        state.hardware.aeration.enabled = true;
        state.hardware.aeration.intensity = 0.5;
        state.hardware.light.enabled = false;
        state.plant_guilds.clear();
        state.algae.set_periphyton_total(0.0);
        state.algae.suspended_biomass_g = 0.0;
        state.microbe.decomposer_biomass_g = 0.0;
        state.microbe.decomposer_by_habitat.clear();
        state.microbe.denitrifier_activity_index = 0.0;
        state.microfauna.population_index = 0.0;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.detritus.particulate_organics_g_total = 0.0;
        state.detritus.fine_detritus_g_total = 0.0;
        state.detritus.dissolved_feed_residue_g_total = 0.0;
        state.process_params.feed_leach_rate_per_hour = 0.0;
        state.process_params.decomposer_vmax_per_hour = 0.0;
        state.refresh_habitat_registry();
        state
    };

    let fixed_seeded_total_nitrifier_g = {
        let reference_state = base_state(SimSeed(42), SMALL_MEDIA_CM2);
        compute_biofilter_carrying_capacity(
            &reference_state.habitat_registry,
            reference_state
                .process_params
                .nitrifier_base_density_g_per_cm2,
        ) * INITIAL_CAPACITY_FRACTION
    };

    let build_state = |seed: SimSeed, media_area_cm2: f64| -> TankState {
        let mut state = base_state(seed, media_area_cm2);
        let carrying_capacity_g = compute_biofilter_carrying_capacity(
            &state.habitat_registry,
            state.process_params.nitrifier_base_density_g_per_cm2,
        );
        let seeded_total_nitrifier_g = fixed_seeded_total_nitrifier_g.min(carrying_capacity_g);
        let seeded_capacity_fraction = if carrying_capacity_g > 0.0 {
            seeded_total_nitrifier_g / carrying_capacity_g
        } else {
            0.0
        };
        state.microbe.ammonia_oxidizer_biomass_g = seeded_total_nitrifier_g * 0.45;
        state.microbe.nitrite_oxidizer_biomass_g = seeded_total_nitrifier_g * 0.45;
        state.microbe.comammox_biomass_g = seeded_total_nitrifier_g * 0.1;
        state.filter_state.biofilter_maturity_index = seeded_capacity_fraction;
        state.refresh_habitat_registry();
        state
    };

    let mut run_small = HarnessRun::from_state(
        SimSeed(42),
        "biofilter_capacity",
        build_state(SimSeed(42), SMALL_MEDIA_CM2),
    )
    .with_artifact_label(probe_artifact_label("biofilter_small", artifact_suffix));
    run_small.enable_instrumentation();
    let mut run_large = HarnessRun::from_state(
        SimSeed(42),
        "biofilter_capacity",
        build_state(SimSeed(42), LARGE_MEDIA_CM2),
    )
    .with_artifact_label(probe_artifact_label("biofilter_large", artifact_suffix));
    run_large.enable_instrumentation();

    let filter_area = |state: &TankState| {
        state
            .habitat_registry
            .iter()
            .find(|habitat| habitat.kind == HabitatKind::FilterMedia)
            .map(|habitat| habitat.colonizable_area_cm2)
            .unwrap_or(0.0)
    };
    let nitrifier_biomass = |state: &TankState| {
        state.microbe.ammonia_oxidizer_biomass_g
            + state.microbe.nitrite_oxidizer_biomass_g
            + state.microbe.comammox_biomass_g
    };

    let filter_area_small = filter_area(run_small.engine().full_state());
    let filter_area_large = filter_area(run_large.engine().full_state());
    let initial_nitrifier_small = nitrifier_biomass(run_small.engine().full_state());
    let initial_nitrifier_large = nitrifier_biomass(run_large.engine().full_state());
    let capacity_small = compute_biofilter_carrying_capacity(
        &run_small.engine().full_state().habitat_registry,
        run_small
            .engine()
            .full_state()
            .process_params
            .nitrifier_base_density_g_per_cm2,
    );
    let capacity_large = compute_biofilter_carrying_capacity(
        &run_large.engine().full_state().habitat_registry,
        run_large
            .engine()
            .full_state()
            .process_params
            .nitrifier_base_density_g_per_cm2,
    );

    let mut reduced_n_exposure_small =
        run_small.snapshot().tan_mg_n_per_l + run_small.snapshot().nitrite_mg_n_per_l;
    let mut reduced_n_exposure_large =
        run_large.snapshot().tan_mg_n_per_l + run_large.snapshot().nitrite_mg_n_per_l;
    let mut reduced_n_clearance_small = None;
    let mut reduced_n_clearance_large = None;

    for hour in 0..DURATION_HOURS {
        run_small.step_hours(1)?;
        run_large.step_hours(1)?;

        let snap_small = run_small.snapshot();
        let snap_large = run_large.snapshot();
        let reduced_n_small = snap_small.tan_mg_n_per_l + snap_small.nitrite_mg_n_per_l;
        let reduced_n_large = snap_large.tan_mg_n_per_l + snap_large.nitrite_mg_n_per_l;
        reduced_n_exposure_small += reduced_n_small;
        reduced_n_exposure_large += reduced_n_large;

        let clearance_hour = hour + 1;
        if reduced_n_clearance_small.is_none() && reduced_n_small <= REDUCED_N_CLEARANCE_THRESHOLD {
            reduced_n_clearance_small = Some(clearance_hour);
        }
        if reduced_n_clearance_large.is_none() && reduced_n_large <= REDUCED_N_CLEARANCE_THRESHOLD {
            reduced_n_clearance_large = Some(clearance_hour);
        }
    }

    let snap_small = run_small.snapshot();
    let snap_large = run_large.snapshot();
    let final_reduced_n_small = snap_small.tan_mg_n_per_l + snap_small.nitrite_mg_n_per_l;
    let final_reduced_n_large = snap_large.tan_mg_n_per_l + snap_large.nitrite_mg_n_per_l;
    let nitrifier_small = nitrifier_biomass(run_small.engine().full_state());
    let nitrifier_large = nitrifier_biomass(run_large.engine().full_state());
    let clearance_summary =
        format_clearance_summary(reduced_n_clearance_small, reduced_n_clearance_large);

    dump_habitat_debug("biofilter_small", run_small.engine().full_state());
    dump_habitat_debug("biofilter_large", run_large.engine().full_state());
    eprintln!("{}", format_snapshot_summary("small_final", &snap_small));
    eprintln!("{}", format_snapshot_summary("large_final", &snap_large));
    eprintln!(
        "Biofilter metrics: filter_area small={filter_area_small:.1} cm² large={filter_area_large:.1} cm²; \
         capacity small={capacity_small:.4}g large={capacity_large:.4}g; \
         reduced-N exposure small={reduced_n_exposure_small:.2} large={reduced_n_exposure_large:.2}; \
         final TAN small={:.3} large={:.3}; final NO₂ small={:.3} large={:.3}; \
         final reduced-N small={final_reduced_n_small:.3} large={final_reduced_n_large:.3}; \
         clearance {clearance_summary}",
        snap_small.tan_mg_n_per_l,
        snap_large.tan_mg_n_per_l,
        snap_small.nitrite_mg_n_per_l,
        snap_large.nitrite_mg_n_per_l,
    );

    {
        let mut runs = [&mut run_small, &mut run_large];
        record_check(
            &mut runs,
            "biofilter_seeded_biomass",
            (initial_nitrifier_large - initial_nitrifier_small).abs() <= 1.0e-9,
            format!(
                "both media packs should start from the same absolute seeded nitrifier biomass: \
                 small={initial_nitrifier_small:.4} g, large={initial_nitrifier_large:.4} g"
            ),
        );
        record_check(
            &mut runs,
            "biofilter_filter_area",
            filter_area_large > filter_area_small * 1.5,
            format!(
                "8× media should expand filter habitat area by >1.5×: \
                 small={filter_area_small:.1} cm², large={filter_area_large:.1} cm²"
            ),
        );
        record_check(
            &mut runs,
            "biofilter_capacity",
            capacity_large > capacity_small * 1.25,
            format!(
                "larger media should materially increase nitrifier capacity: \
                 small={capacity_small:.4} g, large={capacity_large:.4} g"
            ),
        );
        record_check(
            &mut runs,
            "biofilter_terminal_reduced_n",
            final_reduced_n_large < final_reduced_n_small,
            format!(
                "after the same seeded biomass warm-up, the larger filter should finish with lower TAN+NO₂: \
                 small={final_reduced_n_small:.3}, large={final_reduced_n_large:.3}"
            ),
        );
        record_check(
            &mut runs,
            "biofilter_nitrifier_biomass",
            nitrifier_large > nitrifier_small,
            format!(
                "larger filter should support more nitrifier biomass by the end of the run: \
                 small={nitrifier_small:.4} g, large={nitrifier_large:.4} g"
            ),
        );
        match (reduced_n_clearance_small, reduced_n_clearance_large) {
            (Some(small_hour), Some(large_hour)) => record_check(
                &mut runs,
                "biofilter_reduced_n_clearance",
                large_hour <= small_hour,
                format!(
                    "larger filter should clear TAN+NO₂ to <= {REDUCED_N_CLEARANCE_THRESHOLD:.1} mg N/L no later than the smaller filter: \
                     small={small_hour}h, large={large_hour}h"
                ),
            ),
            (None, Some(_)) => {}
            (Some(small_hour), None) => record_failure_all(
                &mut runs,
                "biofilter_reduced_n_clearance",
                format!(
                    "smaller filter cleared TAN+NO₂ by {small_hour}h but the larger filter never did"
                ),
            ),
            (None, None) => record_check(
                &mut runs,
                "biofilter_no_clearance_fallback",
                final_reduced_n_large < final_reduced_n_small
                    && snap_large.nitrite_mg_n_per_l <= snap_small.nitrite_mg_n_per_l,
                format!(
                    "if neither filter clears TAN+NO₂ inside the horizon, the larger media pack should still finish with lower total reduced N and no worse NO₂: \
                     reduced-N small={final_reduced_n_small:.4} large={final_reduced_n_large:.4}, \
                     NO₂ small={:.4} large={:.4}",
                    snap_small.nitrite_mg_n_per_l,
                    snap_large.nitrite_mg_n_per_l,
                ),
            ),
        }
    }

    run_small.assert_envelope(
        "biofilter_small_final",
        &Envelope::default()
            .do_min(5.0)
            .ph(6.5, 9.0)
            .tan_mg_n_per_l(0.0, 8.0),
    );
    run_large.assert_envelope(
        "biofilter_large_final",
        &Envelope::default()
            .do_min(5.0)
            .ph(6.5, 9.0)
            .tan_mg_n_per_l(0.0, 8.0),
    );

    Ok(finish_probe(
        "biofilter_scaling",
        format!(
            "area {filter_area_small:.0}->{filter_area_large:.0} cm², capacity {capacity_small:.3}->{capacity_large:.3} g, \
             reduced-N exposure {reduced_n_exposure_small:.2}->{reduced_n_exposure_large:.2}, \
             final TAN {tan_small:.3}->{tan_large:.3}, final NO₂ {no2_small:.3}->{no2_large:.3}, \
             final reduced-N {final_reduced_n_small:.3}->{final_reduced_n_large:.3}, clearance {clearance_summary}",
            tan_small = snap_small.tan_mg_n_per_l,
            tan_large = snap_large.tan_mg_n_per_l,
            no2_small = snap_small.nitrite_mg_n_per_l,
            no2_large = snap_large.nitrite_mg_n_per_l,
        ),
        vec![run_small, run_large],
    ))
}

// ---------------------------------------------------------------------------
// 2. Light-depth: deep tank → less bottom light → different plant/algae
//    behavior vs shallow tank
// ---------------------------------------------------------------------------

/// Light-depth scenario: a deep tank receives less light at the substrate
/// than a shallow tank, resulting in slower plant growth.
///
/// **Ecological mechanism:** Beer-Lambert attenuation reduces light intensity
/// exponentially with depth. In deeper water columns, less photosynthetically
/// active radiation (PAR) reaches the substrate surface, plant canopy, and
/// glass walls. This suppresses photosynthesis, leading to lower biomass
/// accumulation over time.
///
/// **What we validate:**
/// - Same-footprint tanks with different fill heights (shallow ~15 cm vs
///   deep ~52 cm above substrate) produce different growth patterns
/// - Shallow tank achieves net plant biomass gain while the deeper run lags
/// - Substrate-surface habitat light exposure is lower in the deep tank
/// - Both tanks remain within basic stability envelopes
#[test]
#[ignore = "run explicitly with -- --ignored when iterating on a single habitat probe"]
fn probe_light_depth_shallow_vs_deep_growth() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_light_depth_shallow_vs_deep_growth())
}

fn run_probe_light_depth_shallow_vs_deep_growth() -> Result<ProbeResult, Box<dyn std::error::Error>>
{
    run_probe_light_depth_shallow_vs_deep_growth_with_suffix(None)
}

fn run_probe_light_depth_shallow_vs_deep_growth_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    // Build two tanks with SAME footprint but different fill heights.
    // Use from_state to control geometry directly without size_scale
    // (which would change footprint and starting biomass).
    let build_state = |fill_height_cm: f64| -> TankState {
        let mut state = TankState::new(SimSeed(77));
        let old_volume_l = state.water_volume_l();
        state.geometry.height_cm = fill_height_cm.max(state.geometry.height_cm) + 2.0;
        state.geometry.fill_height_cm = fill_height_cm;
        let volume_l = state.water_volume_l();
        state
            .water
            .rescale_totals_for_volume(old_volume_l, volume_l);
        state.environment.ambient_temp_c = 25.0;
        state.water.temperature_c = 25.0;
        state
            .plant_guilds
            .retain(|plant| plant.guild == PlantGuild::FastStem);
        if let Some(plant) = state.plant_guilds.first_mut() {
            plant.biomass_g = 6.0;
            plant.health_index = 0.95;
        }
        state.hardware.light.enabled = true;
        state.hardware.light.intensity_index = 1.0;
        state.hardware.light.photoperiod_hours = 12.0;
        state.hardware.filter.enabled = true;
        state.hardware.aeration.enabled = true;
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 12.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.8 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 30.0 * volume_l;
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        // Keep attenuation strong enough to separate shallow vs deep runs
        // while still letting the shallow water column support net growth.
        state.process_params.base_extinction_coeff_per_cm = 0.025;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.reseed_stability_tracker();
        state.refresh_habitat_registry();
        state
    };

    let state_shallow = build_state(18.0);
    let state_deep = build_state(55.0);

    let mut run_shallow = HarnessRun::from_state(SimSeed(77), "light_depth", state_shallow)
        .with_artifact_label(probe_artifact_label("light_shallow", artifact_suffix));
    run_shallow.enable_instrumentation();

    let mut run_deep = HarnessRun::from_state(SimSeed(77), "light_depth", state_deep)
        .with_artifact_label(probe_artifact_label("light_deep", artifact_suffix));
    run_deep.enable_instrumentation();

    let shallow_depth = run_shallow
        .engine()
        .full_state()
        .water_depth_above_substrate_cm();
    let deep_depth = run_deep
        .engine()
        .full_state()
        .water_depth_above_substrate_cm();
    eprintln!(
        "Shallow water depth: {:.1} cm, Deep water depth: {:.1} cm",
        shallow_depth, deep_depth
    );

    // Verify habitat light exposure is lower in the deep tank
    let substrate_light = |state: &TankState| -> f64 {
        state
            .habitat_registry
            .iter()
            .find(|h| h.kind == HabitatKind::SubstrateSurface)
            .map(|h| h.light_exposure)
            .unwrap_or(0.0)
    };
    let substrate_light_shallow = substrate_light(run_shallow.engine().full_state());
    let substrate_light_deep = substrate_light(run_deep.engine().full_state());

    // Record initial plant biomass (same for both — same footprint, same defaults)
    let initial_plant_shallow: f64 = run_shallow
        .engine()
        .full_state()
        .plant_guilds
        .iter()
        .map(|p| p.biomass_g)
        .sum();
    let initial_plant_deep: f64 = run_deep
        .engine()
        .full_state()
        .plant_guilds
        .iter()
        .map(|p| p.biomass_g)
        .sum();

    // Run for 14 days
    for _day in 0..14 {
        run_shallow.step_hours(24)?;
        run_deep.step_hours(24)?;
    }

    let snap_shallow = run_shallow.snapshot();
    let snap_deep = run_deep.snapshot();

    dump_habitat_debug("light_shallow", run_shallow.engine().full_state());
    dump_habitat_debug("light_deep", run_deep.engine().full_state());
    eprintln!("{}", format_snapshot_summary("shallow_14d", &snap_shallow));
    eprintln!("{}", format_snapshot_summary("deep_14d", &snap_deep));

    // Validate: shallow tank should produce more plant biomass growth
    let growth_shallow = snap_shallow.total_plant_biomass_g - initial_plant_shallow;
    let growth_deep = snap_deep.total_plant_biomass_g - initial_plant_deep;
    {
        let mut runs = [&mut run_shallow, &mut run_deep];
        record_check(
            &mut runs,
            "light_depth_geometry",
            deep_depth > shallow_depth * 2.0,
            format!(
                "deep tank should have meaningfully more water depth: \
                 shallow={shallow_depth:.1} cm, deep={deep_depth:.1} cm"
            ),
        );
        record_check(
            &mut runs,
            "light_depth_substrate_light",
            substrate_light_shallow > substrate_light_deep,
            format!(
                "shallow substrate should receive more light than deep substrate: \
                 shallow={substrate_light_shallow:.4}, deep={substrate_light_deep:.4}"
            ),
        );
        record_check(
            &mut runs,
            "light_depth_growth",
            growth_shallow > growth_deep,
            format!(
                "shallow tank should have more plant growth (more light at canopy): \
                 shallow_growth={growth_shallow:.4} g, deep_growth={growth_deep:.4} g"
            ),
        );
        record_check(
            &mut runs,
            "light_depth_positive_growth",
            growth_shallow > 0.0,
            format!(
                "shallow tank should achieve net positive plant growth under the brighter water column: \
                 shallow_growth={growth_shallow:.4} g, deep_growth={growth_deep:.4} g"
            ),
        );
    }

    // Both stay within envelopes
    run_shallow.assert_envelope(
        "light_shallow_final",
        &Envelope::default()
            .do_min(3.0)
            .ph(5.0, 9.5)
            .temperature_c(20.0, 30.0),
    );
    run_deep.assert_envelope(
        "light_deep_final",
        &Envelope::default()
            .do_min(3.0)
            .ph(5.0, 9.5)
            .temperature_c(20.0, 30.0),
    );

    Ok(finish_probe(
        "light_depth",
        format!(
            "depth {shallow_depth:.1}->{deep_depth:.1} cm, substrate light {substrate_light_shallow:.4}->{substrate_light_deep:.4}, \
             plant growth {growth_shallow:.4}->{growth_deep:.4} g"
        ),
        vec![run_shallow, run_deep],
    ))
}

// ---------------------------------------------------------------------------
// 3. Habitat-specific fouling: glass periphyton vs filter biofilm
//    developing at different rates
// ---------------------------------------------------------------------------

/// Habitat-specific fouling scenario: periphyton on glass/hardscape develops
/// differently from biofilm on filter media because their environmental
/// exposure profiles differ.
///
/// **Ecological mechanism:** Periphyton on glass/hardscape is light-dependent
/// — it grows via photosynthesis on illuminated surfaces. Filter media biofilm
/// (decomposers and nitrifiers) is flow- and oxygen-dependent but receives
/// negligible light. This means:
/// - Glass periphyton responds to photoperiod and light intensity
/// - Filter biofilm responds to organic matter supply, flow, and oxygen
/// - Their growth rates and biomass accumulation patterns diverge
///
/// **What we validate:**
/// - Glass periphyton responds to light: higher light intensity → more
///   glass periphyton
/// - Filter decomposer biomass responds to organic load: more feed →
///   more filter decomposer biomass
/// - The ratio of glass periphyton to filter decomposer changes with
///   conditions (not a fixed proportion)
#[test]
#[ignore = "run explicitly with -- --ignored when iterating on a single habitat probe"]
fn probe_habitat_fouling_glass_vs_filter() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_habitat_fouling_glass_vs_filter())
}

fn run_probe_habitat_fouling_glass_vs_filter() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_habitat_fouling_glass_vs_filter_with_suffix(None)
}

fn run_probe_habitat_fouling_glass_vs_filter_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let build_run = |light_preset: StartupLightPreset,
                     feed_g: f64,
                     label: &str|
     -> Result<(HarnessRun, f64), Box<dyn std::error::Error>> {
        let overrides = StartupOverrides {
            geometry: ScenarioGeometryOverrides::default(),
            source_water_profile_id: Some("moderate".to_string()),
            substrate_preset: Some(StartupSubstratePreset::InertSand),
            plant_selection: Some(StartupPlantSelection::None),
            filter_enabled: Some(true),
            light_preset: Some(light_preset),
            heater_preset: Some(StartupHeaterPreset::Celsius25),
            aeration_enabled: Some(true),
            initial_adult_shrimp_count: Some(0),
            ..StartupOverrides::default()
        };
        let mut run = HarnessRun::with_overrides(SimSeed(55), "medium_planted", overrides)?
            .with_artifact_label(probe_artifact_label(label, artifact_suffix));
        run.enable_instrumentation();
        Ok((run, feed_g))
    };

    // Scenario A: high light, moderate feed
    let (mut run_high_light, feed_a) =
        build_run(StartupLightPreset::Hours12, 0.1, "fouling_high_light")?;
    // Scenario B: low light, moderate feed
    let (mut run_low_light, feed_b) =
        build_run(StartupLightPreset::Hours6, 0.1, "fouling_low_light")?;
    // Scenario C: high light, heavy feed (more organics for decomposers)
    let (mut run_heavy_feed, feed_c) =
        build_run(StartupLightPreset::Hours12, 0.3, "fouling_heavy_feed")?;

    for _day in 0..21 {
        run_high_light.apply_action(PlayerAction::Feed { grams: feed_a })?;
        run_low_light.apply_action(PlayerAction::Feed { grams: feed_b })?;
        run_heavy_feed.apply_action(PlayerAction::Feed { grams: feed_c })?;
        run_high_light.step_hours(24)?;
        run_low_light.step_hours(24)?;
        run_heavy_feed.step_hours(24)?;
    }

    let state_hl = run_high_light.engine().full_state();
    let state_ll = run_low_light.engine().full_state();
    let state_hf = run_heavy_feed.engine().full_state();

    dump_habitat_debug("fouling_high_light", state_hl);
    dump_habitat_debug("fouling_low_light", state_ll);
    dump_habitat_debug("fouling_heavy_feed", state_hf);

    let glass_periphyton = |state: &TankState| -> f64 {
        state
            .algae
            .periphyton_by_habitat
            .get(&HabitatKind::GlassHardscape)
            .copied()
            .unwrap_or(0.0)
    };
    let filter_decomposer = |state: &TankState| -> f64 {
        state
            .microbe
            .decomposer_by_habitat
            .get(&HabitatKind::FilterMedia)
            .copied()
            .unwrap_or(0.0)
    };

    let hl_glass_peri = glass_periphyton(state_hl);
    let ll_glass_peri = glass_periphyton(state_ll);
    let hf_filter_dec = filter_decomposer(state_hf);
    let hl_filter_dec = filter_decomposer(state_hl);
    let ll_filter_dec = filter_decomposer(state_ll);

    eprintln!(
        "High light:  glass_peri={:.6}g, filter_dec={:.6}g",
        hl_glass_peri, hl_filter_dec
    );
    eprintln!(
        "Low light:   glass_peri={:.6}g, filter_dec={:.6}g",
        ll_glass_peri, ll_filter_dec
    );
    eprintln!(
        "Heavy feed:  glass_peri={:.6}g, filter_dec={:.6}g",
        glass_periphyton(state_hf),
        hf_filter_dec
    );

    // Validate: the periphyton-to-decomposer ratio shifts with light conditions
    let ratio_hl = if hl_filter_dec > 1e-9 {
        hl_glass_peri / hl_filter_dec
    } else {
        0.0
    };
    let ratio_ll = if ll_filter_dec > 1e-9 {
        ll_glass_peri / ll_filter_dec
    } else {
        0.0
    };
    let ratio_measurable = ratio_hl > 1e-9 && ratio_ll > 1e-9;
    let ratio_change = if ratio_measurable {
        (ratio_hl - ratio_ll).abs() / ratio_hl.max(ratio_ll)
    } else {
        0.0
    };
    {
        let mut runs = [&mut run_high_light, &mut run_low_light, &mut run_heavy_feed];
        record_check(
            &mut runs,
            "fouling_glass_periphyton",
            hl_glass_peri > ll_glass_peri,
            format!(
                "more light should produce more glass periphyton: \
                 high_light={hl_glass_peri:.6} g, low_light={ll_glass_peri:.6} g"
            ),
        );
        record_check(
            &mut runs,
            "fouling_filter_decomposer",
            hf_filter_dec > hl_filter_dec,
            format!(
                "more organic load should produce more filter decomposer biomass: \
                 heavy_feed={hf_filter_dec:.6} g, moderate_feed={hl_filter_dec:.6} g"
            ),
        );
        record_check(
            &mut runs,
            "fouling_ratio_measurable",
            ratio_measurable,
            format!(
                "periphyton-to-decomposer ratio needs measurable filter decomposer biomass in both light arms: \
                 high_light_ratio={ratio_hl:.4}, low_light_ratio={ratio_ll:.4}, \
                 high_light_filter_dec={hl_filter_dec:.6} g, low_light_filter_dec={ll_filter_dec:.6} g"
            ),
        );
        if ratio_measurable {
            record_check(
                &mut runs,
                "fouling_ratio_shift",
                ratio_change > 0.05,
                format!(
                    "periphyton-to-decomposer ratio should shift with light conditions: \
                     high_light_ratio={ratio_hl:.4}, low_light_ratio={ratio_ll:.4}, change={:.2}%",
                    ratio_change * 100.0
                ),
            );
        }
    }

    for run in [&mut run_high_light, &mut run_low_light, &mut run_heavy_feed] {
        run.assert_envelope(
            "fouling_final",
            &Envelope::default().do_min(4.0).ph(5.5, 9.0),
        );
    }

    Ok(finish_probe(
        "habitat_fouling",
        format!(
            "glass periphyton high={hl_glass_peri:.6} g low={ll_glass_peri:.6} g; \
             filter decomposer moderate={hl_filter_dec:.6} g heavy={hf_filter_dec:.6} g; \
             ratio shift={:.2}%",
            ratio_change * 100.0
        ),
        vec![run_high_light, run_low_light, run_heavy_feed],
    ))
}

// ---------------------------------------------------------------------------
// 4. Substrate redox: mature planted substrate shows NO₃ removal
//    (denitrification) that unplanted substrate does not
// ---------------------------------------------------------------------------

/// Substrate redox scenario: a thick live-planted substrate develops a
/// suboxic rhizosphere and measurable nitrate removal, while a thin inert
/// substrate does not.
///
/// **Ecological mechanism:** In a planted tank with sufficient substrate
/// depth, the oxygen penetration boundary creates a suboxic zone below
/// the surface. In this zone, denitrifying bacteria reduce NO₃⁻ to N₂
/// gas using dissolved organic carbon as an electron donor. This permanent
/// nitrogen removal is tracked as cumulative N₂ export. A thin inert
/// substrate (like sand) has far less suboxic volume and therefore
/// negligible denitrification.
///
/// This test sets up naturally suboxic conditions and lets the engine
/// dynamically compute O₂ penetration each tick. The planted arm includes
/// live rooted biomass, while both arms keep decomposer loading identical so
/// the comparison remains about substrate geometry and root-zone effects.
/// The inert control also gets active aeration, which makes the contrast
/// stricter: even with extra dissolved oxygen, the thin inert bed still lacks
/// enough suboxic volume to sustain planted-like denitrification.
///
/// **What we validate:**
/// - Live planted substrate starts with a real suboxic zone and measurable
///   rooted biomass/PlantSurfaces habitat
/// - Planted substrate shows measurable cumulative N₂ export after 60 days
/// - Planted substrate finishes with materially lower NO₃ than the inert control
/// - Inert substrate keeps far less suboxic volume, so it cannot match the
///   planted arm's denitrification outcome
#[test]
#[ignore = "run explicitly with -- --ignored when iterating on a single habitat probe"]
fn probe_substrate_redox_denitrification() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_substrate_redox_denitrification())
}

fn run_probe_substrate_redox_denitrification() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_substrate_redox_denitrification_with_suffix(None)
}

fn run_probe_substrate_redox_denitrification_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    const ROOTED_BIOMASS_G: f64 = 6.0;
    const DECOMPOSER_BIOMASS_G: f64 = 10.0;
    const DURATION_DAYS: u32 = 60;

    let configure_redox_baseline = |state: &mut TankState| {
        let volume_l = state.water_volume_l();

        // Start both arms below full maturity so the probe exercises the
        // denitrifier ramp/decay logic rather than preserving a seeded 1.0.
        state.microbe.denitrifier_activity_index = 0.1;
        // Keep decomposer loading identical across both arms so redox
        // divergence comes from substrate geometry plus live rooted plants.
        state.microbe.decomposer_biomass_g = DECOMPOSER_BIOMASS_G;

        state.water.dissolved_oxygen_mg_total = 2.0 * volume_l;
        state.water.dissolved_organic_carbon_mg_c_total = 10.0 * volume_l;
        state.water.nitrate_mg_n_total = 20.0 * volume_l;
        state.water.dissolved_organic_nitrogen_mg_n_total = 1.0 * volume_l;
        state.water.ammonia_total_mg_n_total = 0.0;

        state.process_params.reaeration_kla_base = 0.0;
        state.microbe.ammonia_oxidizer_biomass_g = 0.0;
        state.microbe.nitrite_oxidizer_biomass_g = 0.0;
        state.microbe.comammox_biomass_g = 0.0;
        state
            .process_params
            .denitrification_vmax_mg_n_per_l_per_hour = 0.25;
        state.process_params.feed_leach_rate_per_hour = 0.0;
        state.process_params.decomposer_vmax_per_hour = 0.0;
        state.process_params.fine_detritus_dissolution_rate_per_hour = 0.02;
        state.detritus.particulate_organics_g_total = 0.0;
        state.detritus.fine_detritus_g_total = 0.5;
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
        state.process_params.plant_max_growth_rate_fast_stem_per_day = 0.0;
        state
            .process_params
            .plant_max_growth_rate_root_rosette_per_day = 0.0;
        state.process_params.plant_respiration_fraction_per_day = 0.0;
        state.process_params.plant_senescence_fraction_per_day = 0.0;

        state.environment.ambient_temp_c = 25.0;
        state.water.temperature_c = 25.0;
        state.hardware.light.enabled = true;
        state.hardware.light.intensity_index = 0.8;
        state.hardware.light.photoperiod_hours = 10.0;
        state.hardware.filter.enabled = false;
        state.hardware.filter.flow_lph = 0.0;
        state.hardware.aeration.enabled = false;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.algae.suspended_biomass_g = 0.0;
        state.algae.set_periphyton_total(0.0);
    };

    let build_planted = || -> TankState {
        let mut state = TankState::new(SimSeed(88));
        let footprint_cm2 = state.geometry.footprint_area_cm2();

        // Thick planted substrate
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 12.0,
            o2_penetration_depth_cm: 1.5,
            porosity: 0.50,
            colonizable_area_factor: 0.8,
            colonizable_area_cm2: footprint_cm2 * 12.0 * 0.8,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.5,
            detritus_trapping_index: 0.3,
            low_oxygen_tendency_index: 0.5,
            grazing_surface_index: 0.4,
        }];

        configure_redox_baseline(&mut state);
        for plant in &mut state.plant_guilds {
            plant.biomass_g = if plant.guild == PlantGuild::RootFeedingRosette {
                ROOTED_BIOMASS_G
            } else {
                0.0
            };
        }

        state.refresh_habitat_registry();
        // Let Bouldin model compute the actual O₂ penetration depth
        tank_core::systems::substrate::step_substrate_zones(&mut state);
        state
    };

    let build_inert = || -> TankState {
        let mut state = TankState::new(SimSeed(88));
        let footprint_cm2 = state.geometry.footprint_area_cm2();
        // Thin inert sand — fully oxygenated (shallow substrate means
        // Bouldin penetration covers the entire bed)
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::InertSand,
            depth_cm: 1.0,
            o2_penetration_depth_cm: 1.0,
            porosity: 0.35,
            colonizable_area_factor: 0.5,
            colonizable_area_cm2: footprint_cm2 * 1.0 * 0.5,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.1,
            low_oxygen_tendency_index: 0.1,
            grazing_surface_index: 0.2,
        }];
        configure_redox_baseline(&mut state);
        for plant in &mut state.plant_guilds {
            plant.biomass_g = 0.0;
        }
        state.hardware.aeration.enabled = true;
        state.hardware.aeration.intensity = 0.6;

        state.refresh_habitat_registry();
        tank_core::systems::substrate::step_substrate_zones(&mut state);
        state
    };

    let mut run_planted = HarnessRun::from_state(SimSeed(88), "redox", build_planted())
        .with_artifact_label(probe_artifact_label("redox_planted", artifact_suffix));
    run_planted.enable_instrumentation();

    let mut run_inert = HarnessRun::from_state(SimSeed(88), "redox", build_inert())
        .with_artifact_label(probe_artifact_label("redox_inert", artifact_suffix));
    run_inert.enable_instrumentation();

    let planted_initial_activity = run_planted
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;
    let inert_initial_activity = run_inert
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;
    let planted_initial_suboxic_vol = run_planted
        .engine()
        .full_state()
        .substrate_suboxic_pore_volume_cm3();
    let inert_initial_suboxic_vol = run_inert
        .engine()
        .full_state()
        .substrate_suboxic_pore_volume_cm3();
    let planted_rooted_biomass: f64 = run_planted
        .engine()
        .full_state()
        .plant_guilds
        .iter()
        .filter(|plant| plant.guild == PlantGuild::RootFeedingRosette)
        .map(|plant| plant.biomass_g.max(0.0))
        .sum();
    let planted_surface_area = run_planted
        .engine()
        .full_state()
        .habitat_registry
        .iter()
        .find(|entry| entry.kind == HabitatKind::PlantSurfaces)
        .map(|entry| entry.colonizable_area_cm2)
        .unwrap_or(0.0);

    for _day in 0..DURATION_DAYS {
        run_planted.step_hours(24)?;
        run_inert.step_hours(24)?;
    }

    let state_planted = run_planted.engine().full_state();
    let state_inert = run_inert.engine().full_state();

    dump_habitat_debug("redox_planted", state_planted);
    dump_habitat_debug("redox_inert", state_inert);

    let snap_planted = run_planted.snapshot();
    let snap_inert = run_inert.snapshot();
    eprintln!("{}", format_snapshot_summary("planted_60d", &snap_planted));
    eprintln!("{}", format_snapshot_summary("inert_60d", &snap_inert));

    let planted_export = state_planted.cumulative_n2_export_mg_n;
    let inert_export = state_inert.cumulative_n2_export_mg_n;

    let planted_final_activity = state_planted.microbe.denitrifier_activity_index;
    let inert_final_activity = state_inert.microbe.denitrifier_activity_index;
    let planted_final_suboxic_vol = state_planted.substrate_suboxic_pore_volume_cm3();
    let inert_final_suboxic_vol = state_inert.substrate_suboxic_pore_volume_cm3();
    eprintln!(
        "N₂ export: planted={:.4} mg N, inert={:.4} mg N; denitrifier activity planted {:.4}->{:.4}, inert {:.4}->{:.4}; suboxic volume initial {:.2}->{:.2} cm³, inert {:.2}->{:.2} cm³",
        planted_export,
        inert_export,
        planted_initial_activity,
        planted_final_activity,
        inert_initial_activity,
        inert_final_activity,
        planted_initial_suboxic_vol,
        planted_final_suboxic_vol,
        inert_initial_suboxic_vol,
        inert_final_suboxic_vol,
    );
    {
        let mut runs = [&mut run_planted, &mut run_inert];
        record_check(
            &mut runs,
            "redox_live_rooted_plants",
            planted_rooted_biomass > 0.0 && planted_surface_area > 0.0,
            format!(
                "planted arm should include live rooted plants with non-zero PlantSurfaces area: \
                 rooted_biomass={planted_rooted_biomass:.2} g, plant_surface_area={planted_surface_area:.1} cm²"
            ),
        );
        record_check(
            &mut runs,
            "redox_suboxic_zone",
            planted_initial_suboxic_vol > inert_initial_suboxic_vol * 5.0,
            format!(
                "planted substrate should start with much more suboxic volume than the thin inert control: \
                 planted={planted_initial_suboxic_vol:.2} cm³, inert={inert_initial_suboxic_vol:.2} cm³"
            ),
        );
        record_check(
            &mut runs,
            "redox_n2_export",
            planted_export > inert_export * 2.0,
            format!(
                "planted substrate should export much more N₂ than inert: \
                 planted={planted_export:.4} mg N, inert={inert_export:.4} mg N"
            ),
        );
        record_check(
            &mut runs,
            "redox_denitrifier_activity",
            planted_final_activity > (planted_initial_activity + 0.25).max(0.35),
            format!(
                "denitrifier activity should mature upward to a clearly established final level in planted substrate: \
                 initial={planted_initial_activity:.4}, final={planted_final_activity:.4}"
            ),
        );
        record_check(
            &mut runs,
            "redox_final_suboxic_zone",
            planted_final_suboxic_vol > 0.0,
            format!(
                "planted substrate should retain a non-zero final suboxic pore volume after maturation: \
                 planted={planted_final_suboxic_vol:.2} cm³, inert={inert_final_suboxic_vol:.2} cm³"
            ),
        );
        record_check(
            &mut runs,
            "redox_inert_nitrate_floor",
            snap_inert.nitrate_mg_n_per_l > 1.0,
            format!(
                "inert control nitrate must stay meaningfully above zero before using a ratio comparison: \
                 planted={:.2} mg N/L, inert={:.2} mg N/L",
                snap_planted.nitrate_mg_n_per_l, snap_inert.nitrate_mg_n_per_l
            ),
        );
        record_check(
            &mut runs,
            "redox_nitrate_drawdown",
            snap_inert.nitrate_mg_n_per_l > 1.0
                && snap_planted.nitrate_mg_n_per_l < snap_inert.nitrate_mg_n_per_l * 0.5,
            format!(
                "planted substrate should finish with much lower nitrate than inert: \
                 planted={:.2} mg N/L, inert={:.2} mg N/L",
                snap_planted.nitrate_mg_n_per_l, snap_inert.nitrate_mg_n_per_l
            ),
        );
    }

    // Envelopes (relaxed DO min — reaeration is disabled so DO stays low)
    run_planted.assert_envelope(
        "redox_planted_final",
        &Envelope::default()
            .do_min(0.0)
            .ph(5.0, 9.5)
            .temperature_c(22.0, 28.0),
    );
    run_inert.assert_envelope(
        "redox_inert_final",
        &Envelope::default()
            .do_min(0.0)
            .ph(5.0, 9.5)
            .temperature_c(22.0, 28.0),
    );

    Ok(finish_probe(
        "substrate_redox",
        format!(
            "suboxic volume {planted_initial_suboxic_vol:.0}->{planted_final_suboxic_vol:.0} / {inert_initial_suboxic_vol:.0}->{inert_final_suboxic_vol:.0} cm³, \
             N₂ export {planted_export:.2}/{inert_export:.2} mg N, activity {planted_initial_activity:.2}->{planted_final_activity:.2} / {inert_initial_activity:.2}->{inert_final_activity:.2}"
        ),
        vec![run_planted, run_inert],
    ))
}

// ---------------------------------------------------------------------------
// 5. Equipment scaling: scaled-up tank with proportionally scaled equipment
//    shows similar per-liter behavior
// ---------------------------------------------------------------------------

/// Equipment scaling scenario: a 2× geometry tank with proportionally scaled
/// hardware, plants, and stocking should show similar per-liter
/// concentrations and qualitative outcomes as a 1× tank.
///
/// **Ecological mechanism:** When all equipment (filter media, heater,
/// aeration), plant biomass, and stocking density scale proportionally with
/// tank volume, the per-liter bioload, filtration capacity, and nutrient
/// dynamics should remain roughly equivalent. This test validates that
/// geometry scaling preserves ecological equivalence rather than introducing
/// artifacts from absolute-value calculations.
///
/// **What we validate:**
/// - Per-liter TAN, DO concentrations stay within a tolerance band
///   between 1× and 2× geometry runs
/// - Temperature equilibrium is comparable
/// - Both runs remain within stability envelopes
#[test]
#[ignore = "run explicitly with -- --ignored when iterating on a single habitat probe"]
fn probe_equipment_scaling_1x_vs_2x() -> Result<(), Box<dyn std::error::Error>> {
    require_probe_pass(run_probe_equipment_scaling_1x_vs_2x())
}

fn run_probe_equipment_scaling_1x_vs_2x() -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_equipment_scaling_1x_vs_2x_with_suffix(None)
}

fn run_probe_equipment_scaling_1x_vs_2x_with_suffix(
    artifact_suffix: Option<&str>,
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    let duration_hours: u32 = 500;
    let feed_per_adult_per_day: f64 = 0.001;
    let base_adult_count: u32 = 10;
    let concentration_tolerance: f64 = 0.40;

    let build_run =
        |size_scale: f64, label: &str| -> Result<HarnessRun, Box<dyn std::error::Error>> {
            let adult_count = (base_adult_count as f64 * size_scale.powi(3))
                .round()
                .max(1.0) as u32;
            let overrides = StartupOverrides {
                geometry: ScenarioGeometryOverrides {
                    size_scale,
                    fill_ratio: 1.0,
                },
                source_water_profile_id: Some("moderate".to_string()),
                substrate_preset: Some(StartupSubstratePreset::InertSand),
                plant_selection: Some(StartupPlantSelection::BothGuilds),
                filter_enabled: Some(true),
                light_preset: Some(StartupLightPreset::Hours10),
                heater_preset: Some(StartupHeaterPreset::Celsius25),
                aeration_enabled: Some(true),
                initial_adult_shrimp_count: Some(adult_count),
                ..StartupOverrides::default()
            };
            Ok(
                HarnessRun::with_overrides(SimSeed(42), "medium_planted", overrides)?
                    .with_artifact_label(probe_artifact_label(label, artifact_suffix)),
            )
        };

    let mut run_1x = build_run(1.0, "scale_1x")?;
    run_1x.enable_instrumentation();
    let mut run_2x = build_run(2.0, "scale_2x")?;
    run_2x.enable_instrumentation();

    let adults_1x = run_1x.snapshot().adult_shrimp_count;
    let adults_2x = run_2x.snapshot().adult_shrimp_count;
    let vol_1x = run_1x.snapshot().water_volume_l;
    let vol_2x = run_2x.snapshot().water_volume_l;
    eprintln!(
        "1× setup: {} adults, {:.1} L; 2× setup: {} adults, {:.1} L",
        adults_1x, vol_1x, adults_2x, vol_2x
    );

    let mut peak_tan_1x = 0.0_f64;
    let mut peak_tan_2x = 0.0_f64;
    let mut min_do_1x = f64::INFINITY;
    let mut min_do_2x = f64::INFINITY;
    let mut max_do_1x = f64::NEG_INFINITY;
    let mut max_do_2x = f64::NEG_INFINITY;

    let daily_feed_1x = adults_1x as f64 * feed_per_adult_per_day;
    let daily_feed_2x = adults_2x as f64 * feed_per_adult_per_day;

    for hour in 0..duration_hours {
        if hour % 24 == 0 {
            run_1x.apply_action(PlayerAction::Feed {
                grams: daily_feed_1x,
            })?;
            run_2x.apply_action(PlayerAction::Feed {
                grams: daily_feed_2x,
            })?;
        }
        run_1x.step_hours(1)?;
        run_2x.step_hours(1)?;

        let s1 = run_1x.snapshot();
        let s2 = run_2x.snapshot();
        peak_tan_1x = peak_tan_1x.max(s1.tan_mg_n_per_l);
        peak_tan_2x = peak_tan_2x.max(s2.tan_mg_n_per_l);
        min_do_1x = min_do_1x.min(s1.do_mg_l);
        min_do_2x = min_do_2x.min(s2.do_mg_l);
        max_do_1x = max_do_1x.max(s1.do_mg_l);
        max_do_2x = max_do_2x.max(s2.do_mg_l);
    }

    dump_habitat_debug("scale_1x", run_1x.engine().full_state());
    dump_habitat_debug("scale_2x", run_2x.engine().full_state());

    let snap_1x = run_1x.snapshot();
    let snap_2x = run_2x.snapshot();
    eprintln!("{}", format_snapshot_summary("1x_final", &snap_1x));
    eprintln!("{}", format_snapshot_summary("2x_final", &snap_2x));

    let do_range_1x = max_do_1x - min_do_1x;
    let do_range_2x = max_do_2x - min_do_2x;
    {
        let mut runs = [&mut run_1x, &mut run_2x];
        record_within_fraction(
            &mut runs,
            "peak TAN",
            peak_tan_1x,
            peak_tan_2x,
            concentration_tolerance,
        );
        record_within_fraction(
            &mut runs,
            "DO range",
            do_range_1x,
            do_range_2x,
            concentration_tolerance,
        );
        record_within_fraction(
            &mut runs,
            "final temperature",
            snap_1x.water_temp_c,
            snap_2x.water_temp_c,
            0.20,
        );
    }

    // Both should stay within envelopes
    run_1x.assert_envelope(
        "scale_1x_final",
        &Envelope::default()
            .temperature_c(23.0, 26.0)
            .do_min(6.5)
            .tan_mg_n_per_l(0.0, 8.0)
            .nitrite_mg_n_per_l(0.0, 5.0)
            .biofilter_maturity(0.03, 1.0)
            .plant_biomass_g(1.0, 400.0),
    );
    run_2x.assert_envelope(
        "scale_2x_final",
        &Envelope::default()
            .temperature_c(23.0, 26.0)
            .do_min(6.5)
            .tan_mg_n_per_l(0.0, 8.0)
            .nitrite_mg_n_per_l(0.0, 5.0)
            .biofilter_maturity(0.03, 1.0)
            .plant_biomass_g(1.0, 400.0),
    );

    Ok(finish_probe(
        "equipment_scaling",
        format!(
            "peak TAN {peak_tan_1x:.4}/{peak_tan_2x:.4}, DO range {do_range_1x:.4}/{do_range_2x:.4}, \
             final temp {}/{} C",
            snap_1x.water_temp_c, snap_2x.water_temp_c
        ),
        vec![run_1x, run_2x],
    ))
}

fn run_probe_biofilter_scaling_bigger_media_higher_capacity_summary(
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_biofilter_scaling_bigger_media_higher_capacity_with_suffix(Some("summary"))
}

fn run_probe_light_depth_shallow_vs_deep_growth_summary(
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_light_depth_shallow_vs_deep_growth_with_suffix(Some("summary"))
}

fn run_probe_habitat_fouling_glass_vs_filter_summary(
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_habitat_fouling_glass_vs_filter_with_suffix(Some("summary"))
}

fn run_probe_substrate_redox_denitrification_summary(
) -> Result<ProbeResult, Box<dyn std::error::Error>> {
    run_probe_substrate_redox_denitrification_with_suffix(Some("summary"))
}

fn run_probe_equipment_scaling_1x_vs_2x_summary() -> Result<ProbeResult, Box<dyn std::error::Error>>
{
    run_probe_equipment_scaling_1x_vs_2x_with_suffix(Some("summary"))
}

// ---------------------------------------------------------------------------
// Summary runner
// ---------------------------------------------------------------------------

/// Runs all 5 habitat/geometry probes, prints a summary report, and fails the
/// suite if any ecological envelope is violated.
#[test]
fn all_habitat_geometry_probes_summary() -> Result<(), Box<dyn std::error::Error>> {
    type ProbeFn = fn() -> Result<ProbeResult, Box<dyn std::error::Error>>;
    let probes: [(&str, ProbeFn); 5] = [
        (
            "biofilter_scaling",
            run_probe_biofilter_scaling_bigger_media_higher_capacity_summary,
        ),
        (
            "light_depth",
            run_probe_light_depth_shallow_vs_deep_growth_summary,
        ),
        (
            "habitat_fouling",
            run_probe_habitat_fouling_glass_vs_filter_summary,
        ),
        (
            "substrate_redox",
            run_probe_substrate_redox_denitrification_summary,
        ),
        (
            "equipment_scaling",
            run_probe_equipment_scaling_1x_vs_2x_summary,
        ),
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
    eprintln!("=== Habitat/Geometry Probe Summary Report ===");
    eprintln!("Structured traces are enabled for every probe run; set TANK_E2E_VERBOSE=1 to stream them on success.");
    eprintln!();
    for (index, result) in results.iter().enumerate() {
        let status = if result.passed { "PASS" } else { "FAIL" };
        eprintln!("  Probe {}: {} [{}]", index + 1, result.name, status);
        eprintln!("    {}", result.observed);
        if !result.passed {
            for line in result.failure_detail.lines() {
                eprintln!("    >> {line}");
            }
        }
    }
    let pass_count = results.iter().filter(|result| result.passed).count();
    let fail_count = results.len() - pass_count;
    eprintln!();
    eprintln!("  {pass_count} passed, {fail_count} failed");
    eprintln!();
    eprintln!("=== End Habitat/Geometry Probe Report ===");
    eprintln!();

    if fail_count > 0 {
        let mut message = format!("{fail_count} habitat/geometry probe(s) failed:\n");
        for result in results.iter().filter(|result| !result.passed) {
            message.push_str(&format!("  {}:\n", result.name));
            for line in result.failure_detail.lines() {
                message.push_str(&format!("    {line}\n"));
            }
        }
        return Err(message.into());
    }

    Ok(())
}
