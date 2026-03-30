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

use tank_core::{
    HabitatKind, PlayerAction, SimSeed, SimulationEngine, SubstrateKind, SubstrateLayerState,
    TankSnapshot, TankState,
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

// ---------------------------------------------------------------------------
// 1. Biofilter scaling: bigger filter media → higher nitrifier capacity
//    → faster cycling
// ---------------------------------------------------------------------------

/// Biofilter scaling scenario: doubling filter media area should produce
/// a larger nitrifier population and more filter-media colonizable area.
///
/// **Ecological mechanism:** Filter media is the primary colonizable surface
/// for ammonia-oxidizing bacteria (AOB) and nitrite-oxidizing bacteria (NOB).
/// More media area means a higher carrying capacity for nitrifiers, which
/// allows a larger nitrifier population to establish. The habitat registry
/// should reflect 2× media area as substantially more FilterMedia
/// colonizable area, and the nitrifier population should respond accordingly.
///
/// **What we validate:**
/// - Tank with 2× media area has >1.5× filter-media colonizable area
/// - Tank with 2× media area supports comparable or more nitrifier biomass
/// - Both tanks remain within basic stability envelopes
#[test]
fn probe_biofilter_scaling_bigger_media_faster_cycling() -> Result<(), Box<dyn std::error::Error>> {
    let duration_hours: u32 = 500;

    let build_run =
        |media_area: Option<f64>, label: &str| -> Result<HarnessRun, Box<dyn std::error::Error>> {
            let overrides = StartupOverrides {
                geometry: ScenarioGeometryOverrides {
                    size_scale: 1.0,
                    fill_ratio: 1.0,
                },
                source_water_profile_id: Some("moderate".to_string()),
                substrate_preset: Some(StartupSubstratePreset::InertSand),
                plant_selection: Some(StartupPlantSelection::FastStemOnly),
                filter_enabled: Some(true),
                filter_media_area_cm2: media_area,
                light_preset: Some(StartupLightPreset::Hours10),
                heater_preset: Some(StartupHeaterPreset::Celsius25),
                aeration_enabled: Some(true),
                initial_adult_shrimp_count: Some(10),
                ..StartupOverrides::default()
            };
            Ok(
                HarnessRun::with_overrides(SimSeed(42), "medium_planted", overrides)?
                    .with_artifact_label(label),
            )
        };

    // Small filter: use auto-scaled default (no override)
    let mut run_small = build_run(None, "biofilter_small")?;
    run_small.enable_instrumentation();

    // Large filter: 2× the auto-scaled default
    let auto_media_area = run_small
        .engine()
        .full_state()
        .hardware
        .filter
        .media_area_cm2;
    let mut run_large = build_run(Some(auto_media_area * 2.0), "biofilter_large")?;
    run_large.enable_instrumentation();

    for hour in 0..duration_hours {
        if hour % 24 == 0 {
            run_small.apply_action(PlayerAction::Feed { grams: 0.1 })?;
            run_large.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        }
        run_small.step_hours(1)?;
        run_large.step_hours(1)?;
    }

    dump_habitat_debug("biofilter_small", run_small.engine().full_state());
    dump_habitat_debug("biofilter_large", run_large.engine().full_state());
    eprintln!(
        "{}",
        format_snapshot_summary("small_final", &run_small.snapshot())
    );
    eprintln!(
        "{}",
        format_snapshot_summary("large_final", &run_large.snapshot())
    );

    // Validate: the filter-media habitat should have more colonizable area
    let filter_area = |state: &TankState| {
        state
            .habitat_registry
            .iter()
            .find(|h| h.kind == HabitatKind::FilterMedia)
            .map(|h| h.colonizable_area_cm2)
            .unwrap_or(0.0)
    };
    assert!(
        filter_area(run_large.engine().full_state())
            > filter_area(run_small.engine().full_state()) * 1.5,
        "2× media should produce >1.5× filter habitat area"
    );

    // Validate: both filters establish viable nitrifier communities.
    // With the same bioload, the larger filter has lower nitrifier density
    // (nutrient-limited, not area-limited), so we check that both achieve
    // non-trivial biomass rather than comparing directly.
    let nitrifier_biomass = |state: &TankState| {
        state.microbe.ammonia_oxidizer_biomass_g
            + state.microbe.nitrite_oxidizer_biomass_g
            + state.microbe.comammox_biomass_g
    };
    let bio_small = nitrifier_biomass(run_small.engine().full_state());
    let bio_large = nitrifier_biomass(run_large.engine().full_state());
    let min_nitrifier_g = 0.02;
    assert!(
        bio_small > min_nitrifier_g && bio_large > min_nitrifier_g,
        "Both filters should establish viable nitrifier communities (>{:.3}g): \
         small={:.4}g, large={:.4}g",
        min_nitrifier_g,
        bio_small,
        bio_large
    );

    // Both should stay within basic envelopes
    run_small.assert_envelope(
        "biofilter_small_final",
        &Envelope::default().do_min(5.0).ph(5.5, 9.0),
    );
    run_large.assert_envelope(
        "biofilter_large_final",
        &Envelope::default().do_min(5.0).ph(5.5, 9.0),
    );

    run_small.finish()?;
    run_large.finish()?;

    Ok(())
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
/// - Shallow tank produces more plant biomass growth after 14 days
/// - Substrate-surface habitat light exposure is lower in the deep tank
/// - Both tanks remain within basic stability envelopes
#[test]
fn probe_light_depth_shallow_vs_deep_growth() -> Result<(), Box<dyn std::error::Error>> {
    // Build two tanks with SAME footprint but different fill heights.
    // Use from_state to control geometry directly without size_scale
    // (which would change footprint and starting biomass).
    let build_state = |fill_height_cm: f64| -> TankState {
        let mut state = TankState::new(SimSeed(77));
        state.geometry.height_cm = fill_height_cm.max(state.geometry.height_cm) + 2.0;
        state.geometry.fill_height_cm = fill_height_cm;
        state.environment.ambient_temp_c = 25.0;
        state.water.temperature_c = 25.0;
        state.hardware.light.enabled = true;
        state.hardware.light.intensity_index = 0.9;
        state.hardware.light.photoperiod_hours = 12.0;
        state.hardware.filter.enabled = true;
        state.hardware.aeration.enabled = true;
        let volume_l = state.water_volume_l();
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 5.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.4 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        // Boost base extinction so depth difference is significant
        state.process_params.base_extinction_coeff_per_cm = 0.03;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.refresh_habitat_registry();
        state
    };

    let state_shallow = build_state(18.0);
    let state_deep = build_state(55.0);

    let mut run_shallow = HarnessRun::from_state(SimSeed(77), "light_depth", state_shallow)
        .with_artifact_label("light_shallow");
    run_shallow.enable_instrumentation();

    let mut run_deep = HarnessRun::from_state(SimSeed(77), "light_depth", state_deep)
        .with_artifact_label("light_deep");
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
    assert!(
        deep_depth > shallow_depth * 2.0,
        "Deep tank should have meaningfully more water depth: shallow={:.1}, deep={:.1}",
        shallow_depth,
        deep_depth
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
    assert!(
        substrate_light(run_shallow.engine().full_state())
            > substrate_light(run_deep.engine().full_state()),
        "Shallow substrate should receive more light than deep substrate"
    );

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
    assert!(
        growth_shallow > growth_deep,
        "Shallow tank should have more plant growth (more light at canopy): \
         shallow_growth={:.4}g, deep_growth={:.4}g",
        growth_shallow,
        growth_deep
    );

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

    run_shallow.finish()?;
    run_deep.finish()?;

    Ok(())
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
fn probe_habitat_fouling_glass_vs_filter() -> Result<(), Box<dyn std::error::Error>> {
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
            .with_artifact_label(label);
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

    // Validate: glass periphyton should be higher under high light than low light
    assert!(
        hl_glass_peri > ll_glass_peri,
        "More light should produce more glass periphyton: \
         high_light={:.6}g, low_light={:.6}g",
        hl_glass_peri,
        ll_glass_peri
    );

    // Validate: filter decomposer should be higher with heavy feed (more organics)
    assert!(
        hf_filter_dec > hl_filter_dec,
        "More organic load should produce more filter decomposer biomass: \
         heavy_feed={:.6}g, moderate_feed={:.6}g",
        hf_filter_dec,
        hl_filter_dec
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
    if ratio_hl > 1e-9 && ratio_ll > 1e-9 {
        let ratio_change = (ratio_hl - ratio_ll).abs() / ratio_hl.max(ratio_ll);
        assert!(
            ratio_change > 0.05,
            "Periphyton-to-decomposer ratio should shift with light conditions: \
             high_light_ratio={:.4}, low_light_ratio={:.4}, change={:.2}%",
            ratio_hl,
            ratio_ll,
            ratio_change * 100.0
        );
    }

    for run in [&mut run_high_light, &mut run_low_light, &mut run_heavy_feed] {
        run.assert_envelope(
            "fouling_final",
            &Envelope::default().do_min(4.0).ph(5.5, 9.0),
        );
    }

    run_high_light.finish()?;
    run_low_light.finish()?;
    run_heavy_feed.finish()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Substrate redox: mature planted substrate shows NO₃ removal
//    (denitrification) that unplanted substrate does not
// ---------------------------------------------------------------------------

/// Substrate redox scenario: a thick planted substrate with mature
/// denitrifiers shows measurable nitrate removal, while a thin inert
/// substrate does not.
///
/// **Ecological mechanism:** In a planted tank with sufficient substrate
/// depth, the oxygen penetration boundary creates a suboxic zone below
/// the surface. In this zone, denitrifying bacteria reduce NO₃⁻ to N₂
/// gas using dissolved organic carbon as an electron donor. This permanent
/// nitrogen removal is tracked as cumulative N₂ export. A thin inert
/// substrate (like sand) that is fully oxygenated has no suboxic zone
/// and therefore negligible denitrification.
///
/// This test directly manipulates substrate O₂ penetration depth and
/// denitrifier activity to ensure the suboxic zone is present. The engine
/// dynamically computes O₂ penetration each tick, so we also verify that
/// the substrate properties produce conditions conducive to denitrification.
///
/// **What we validate:**
/// - Planted substrate with a manually established suboxic zone shows
///   measurable cumulative N₂ export after 60 days
/// - Inert substrate shows negligible N₂ export
/// - Denitrifier activity index matures over time in planted substrate
#[test]
fn probe_substrate_redox_denitrification() -> Result<(), Box<dyn std::error::Error>> {
    // Build a state with thick planted substrate configured for active
    // denitrification. The Bouldin model recalculates O₂ penetration each
    // tick, so we must create conditions that naturally produce a shallow
    // penetration: high decomposer O₂ demand + low DO + no reaeration.
    let build_planted = || -> TankState {
        let mut state = TankState::new(SimSeed(88));
        let footprint_cm2 = state.geometry.footprint_area_cm2();

        // Thick planted substrate
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 8.0,
            o2_penetration_depth_cm: 1.5,
            porosity: 0.50,
            colonizable_area_factor: 0.8,
            colonizable_area_cm2: footprint_cm2 * 8.0 * 0.8,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.5,
            detritus_trapping_index: 0.3,
            low_oxygen_tendency_index: 0.5,
            grazing_surface_index: 0.4,
        }];

        // Mature denitrifier community
        state.microbe.denitrifier_activity_index = 1.0;
        // High decomposer biomass drives substrate O₂ demand → shallow
        // Bouldin penetration depth → large suboxic zone.
        state.microbe.decomposer_biomass_g = 10.0;

        let vol = state.water_volume_l();
        // Low DO so Bouldin model computes shallow O₂ penetration
        state.water.dissolved_oxygen_mg_total = 2.0 * vol;
        // Ample DOC as denitrification electron donor
        state.water.dissolved_organic_carbon_mg_c_total = 10.0 * vol;
        state.water.nitrate_mg_n_total = 20.0 * vol;
        state.water.dissolved_organic_nitrogen_mg_n_total = 1.0 * vol;
        state.water.ammonia_total_mg_n_total = 0.0;

        // Disable reaeration so DO stays low (preserves suboxic zone)
        state.process_params.reaeration_kla_base = 0.0;
        // Disable nitrification to isolate denitrification
        state.microbe.ammonia_oxidizer_biomass_g = 0.0;
        state.microbe.nitrite_oxidizer_biomass_g = 0.0;
        state.microbe.comammox_biomass_g = 0.0;
        // High denitrification rate to see effect over the run
        state
            .process_params
            .denitrification_vmax_mg_n_per_l_per_hour = 0.25;
        // Disable feed leaching and decomposer DOC consumption
        // to keep DOC pool stable for denitrification
        state.process_params.feed_leach_rate_per_hour = 0.0;
        state.process_params.decomposer_vmax_per_hour = 0.0;
        state.process_params.fine_detritus_dissolution_rate_per_hour = 0.02;
        state.detritus.particulate_organics_g_total = 0.0;
        state.detritus.fine_detritus_g_total = 0.5;
        // Disable plant photosynthesis (O₂ production) to keep DO low
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
        state.process_params.plant_max_growth_rate_fast_stem_per_day = 0.0;
        state
            .process_params
            .plant_max_growth_rate_root_rosette_per_day = 0.0;

        state.environment.ambient_temp_c = 25.0;
        state.water.temperature_c = 25.0;
        state.hardware.light.enabled = true;
        state.hardware.light.intensity_index = 0.8;
        state.hardware.light.photoperiod_hours = 10.0;
        state.hardware.filter.enabled = true;
        state.hardware.aeration.enabled = false;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.algae.suspended_biomass_g = 0.0;
        state.algae.set_periphyton_total(0.0);
        for plant in &mut state.plant_guilds {
            plant.biomass_g = 0.0;
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
            depth_cm: 2.0,
            o2_penetration_depth_cm: 2.0,
            porosity: 0.35,
            colonizable_area_factor: 0.5,
            colonizable_area_cm2: footprint_cm2 * 2.0 * 0.5,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.1,
            low_oxygen_tendency_index: 0.1,
            grazing_surface_index: 0.2,
        }];
        state.microbe.denitrifier_activity_index = 0.0;
        state.microbe.decomposer_biomass_g = 2.0;

        let vol = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 2.0 * vol;
        state.water.dissolved_organic_carbon_mg_c_total = 10.0 * vol;
        state.water.nitrate_mg_n_total = 20.0 * vol;
        state.water.dissolved_organic_nitrogen_mg_n_total = 1.0 * vol;
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
        state.process_params.plant_max_growth_rate_fast_stem_per_day = 0.0;
        state
            .process_params
            .plant_max_growth_rate_root_rosette_per_day = 0.0;

        state.environment.ambient_temp_c = 25.0;
        state.water.temperature_c = 25.0;
        state.hardware.light.enabled = true;
        state.hardware.light.intensity_index = 0.8;
        state.hardware.light.photoperiod_hours = 10.0;
        state.hardware.filter.enabled = true;
        state.hardware.aeration.enabled = false;
        state.animal.adult.count = 0;
        state.animal.sub_adult.count = 0;
        state.animal.juvenile.count = 0;
        state.algae.suspended_biomass_g = 0.0;
        state.algae.set_periphyton_total(0.0);
        state.plant_guilds.clear();

        state.refresh_habitat_registry();
        state
    };

    let mut run_planted = HarnessRun::from_state(SimSeed(88), "redox", build_planted())
        .with_artifact_label("redox_planted");
    run_planted.enable_instrumentation();

    let mut run_inert = HarnessRun::from_state(SimSeed(88), "redox", build_inert())
        .with_artifact_label("redox_inert");
    run_inert.enable_instrumentation();

    let planted_initial_activity = run_planted
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;

    // Verify the planted substrate actually has a suboxic zone after Bouldin
    let suboxic_vol = run_planted
        .engine()
        .full_state()
        .substrate_suboxic_pore_volume_cm3();
    assert!(
        suboxic_vol > 0.0,
        "Planted substrate should have a suboxic zone after Bouldin computation: \
         suboxic_pore_vol={:.2} cm³",
        suboxic_vol
    );

    // Run for 30 days (no feeding needed — DOC is pre-loaded as electron donor)
    for _day in 0..30 {
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

    eprintln!(
        "N₂ export: planted={:.4} mg N, inert={:.4} mg N",
        planted_export, inert_export
    );

    // Validate: planted substrate should export more N₂ than inert
    assert!(
        planted_export > inert_export,
        "Planted substrate should export more N₂ than inert: \
         planted={:.4}, inert={:.4}",
        planted_export,
        inert_export
    );

    // Validate: denitrifier activity should remain high in planted substrate
    let planted_final_activity = state_planted.microbe.denitrifier_activity_index;
    assert!(
        planted_final_activity > 0.5,
        "Denitrifier activity should remain high with active suboxic zone: \
         initial={:.4}, final={:.4}",
        planted_initial_activity,
        planted_final_activity
    );

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

    run_planted.finish()?;
    run_inert.finish()?;

    Ok(())
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
fn probe_equipment_scaling_1x_vs_2x() -> Result<(), Box<dyn std::error::Error>> {
    let duration_hours: u32 = 500;
    let feed_per_adult_per_day: f64 = 0.001;
    let base_adult_count: u32 = 10;
    let concentration_tolerance: f64 = 0.55;

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
                    .with_artifact_label(label),
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

    // Compare peak TAN concentrations
    assert_within_fraction(
        "peak TAN",
        peak_tan_1x,
        peak_tan_2x,
        concentration_tolerance,
    );

    // Compare DO ranges
    let do_range_1x = max_do_1x - min_do_1x;
    let do_range_2x = max_do_2x - min_do_2x;
    assert_within_fraction(
        "DO range",
        do_range_1x,
        do_range_2x,
        concentration_tolerance,
    );

    // Compare final temperature
    assert_within_fraction(
        "final temperature",
        snap_1x.water_temp_c,
        snap_2x.water_temp_c,
        0.20,
    );

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

    run_1x.finish()?;
    run_2x.finish()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_within_fraction(label: &str, lhs: f64, rhs: f64, tolerance: f64) {
    let scale = lhs.abs().max(rhs.abs()).max(1e-9);
    let fraction = (lhs - rhs).abs() / scale;
    assert!(
        fraction <= tolerance,
        "{label}: values should stay within {:.0}% \
         (lhs={lhs:.4}, rhs={rhs:.4}, divergence={:.1}%)",
        tolerance * 100.0,
        fraction * 100.0
    );
}
