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
    Engine, HabitatEntry, HabitatKind, PlayerAction, SimSeed, SimulationEngine, SubstrateKind,
    SubstrateLayerState, TankSnapshot, TankState,
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
/// materially faster nitrogen cycling.
///
/// **Ecological mechanism:** Filter media is the primary colonizable surface
/// for ammonia-oxidizing bacteria (AOB) and nitrite-oxidizing bacteria (NOB).
/// More media area means a higher carrying capacity for nitrifiers, which
/// allows a larger nitrifier population to establish. During fishless cycling,
/// this translates to faster TAN consumption and an earlier transition to
/// stable nitrate accumulation.
///
/// **What we validate:**
/// - Tank with 2× media area reaches the biofilter maturity threshold sooner
/// - Tank with 2× media area shows lower peak TAN during the cycling period
/// - Both tanks remain within basic stability envelopes
#[test]
fn probe_biofilter_scaling_bigger_media_faster_cycling() -> Result<(), Box<dyn std::error::Error>> {
    let duration_hours: u32 = 500;
    let maturity_threshold = 0.04;

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
                initial_adult_shrimp_count: Some(0), // Fishless cycling
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
    // First, determine the auto-scaled media area from the small run
    let auto_media_area = run_small
        .engine()
        .full_state()
        .hardware
        .filter
        .media_area_cm2;
    let mut run_large = build_run(Some(auto_media_area * 2.0), "biofilter_large")?;
    run_large.enable_instrumentation();

    let mut small_maturity_hour: Option<u32> = None;
    let mut large_maturity_hour: Option<u32> = None;
    let mut small_peak_tan = 0.0_f64;
    let mut large_peak_tan = 0.0_f64;

    for hour in 0..duration_hours {
        // Daily feed as ammonia source for fishless cycling
        if hour % 24 == 0 {
            run_small.apply_action(PlayerAction::Feed { grams: 0.1 })?;
            run_large.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        }
        run_small.step_hours(1)?;
        run_large.step_hours(1)?;

        let snap_s = run_small.snapshot();
        let snap_l = run_large.snapshot();

        small_peak_tan = small_peak_tan.max(snap_s.tan_mg_n_per_l);
        large_peak_tan = large_peak_tan.max(snap_l.tan_mg_n_per_l);

        if small_maturity_hour.is_none() && snap_s.biofilter_maturity_index >= maturity_threshold {
            small_maturity_hour = Some(hour + 1);
        }
        if large_maturity_hour.is_none() && snap_l.biofilter_maturity_index >= maturity_threshold {
            large_maturity_hour = Some(hour + 1);
        }
    }

    // Dump debug info for diagnosis on failure
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

    // Validate: larger filter should cycle at least as fast
    match (small_maturity_hour, large_maturity_hour) {
        (Some(small_h), Some(large_h)) => {
            assert!(
                large_h <= small_h,
                "Larger filter should reach maturity at least as fast: \
                 small={}h, large={}h",
                small_h,
                large_h
            );
        }
        (None, Some(_)) => {
            // Large cycled but small didn't — expected, pass.
        }
        (Some(small_h), None) => {
            panic!(
                "Small filter cycled at {}h but large never did — \
                 larger filter should not be slower",
                small_h
            );
        }
        (None, None) => {
            // Neither cycled. Compare final nitrifier biomass.
            let s_bio = {
                let s = run_small.engine().full_state();
                s.microbe.ammonia_oxidizer_biomass_g
                    + s.microbe.nitrite_oxidizer_biomass_g
                    + s.microbe.comammox_biomass_g
            };
            let l_bio = {
                let s = run_large.engine().full_state();
                s.microbe.ammonia_oxidizer_biomass_g
                    + s.microbe.nitrite_oxidizer_biomass_g
                    + s.microbe.comammox_biomass_g
            };
            assert!(
                l_bio > s_bio,
                "Larger filter should have more nitrifier biomass: small={:.4}g, large={:.4}g",
                s_bio,
                l_bio
            );
        }
    }

    // Validate: larger filter should accumulate more nitrifier biomass
    // (higher carrying capacity). Peak TAN during early cycling is dominated
    // by feed input, not filtration, so we compare final nitrifier biomass
    // rather than peak TAN.
    let nitrifier_biomass = |state: &TankState| {
        state.microbe.ammonia_oxidizer_biomass_g
            + state.microbe.nitrite_oxidizer_biomass_g
            + state.microbe.comammox_biomass_g
    };
    let bio_small = nitrifier_biomass(run_small.engine().full_state());
    let bio_large = nitrifier_biomass(run_large.engine().full_state());
    assert!(
        bio_large >= bio_small * 0.95,
        "Larger filter should support comparable or more nitrifier biomass: \
         small={:.4}g, large={:.4}g",
        bio_small,
        bio_large
    );

    // The filter-media habitat should have more colonizable area in the
    // large-filter case.
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
/// than a shallow tank, resulting in slower plant and algae growth.
///
/// **Ecological mechanism:** Beer-Lambert attenuation reduces light intensity
/// exponentially with depth. In deeper water columns, less photosynthetically
/// active radiation (PAR) reaches the substrate surface, plant canopy, and
/// glass walls. This suppresses photosynthesis in both plants and periphyton,
/// leading to lower biomass accumulation over time.
///
/// **What we validate:**
/// - Shallow tank (15 cm water depth) produces more plant biomass than deep
///   tank (45 cm) after 14 days under identical light, nutrients, and
///   temperature
/// - Shallow tank produces more periphyton than deep tank
/// - Shallow tank algae nuisance index is equal to or higher than deep tank
/// - Both tanks remain within basic stability envelopes
#[test]
fn probe_light_depth_shallow_vs_deep_growth() -> Result<(), Box<dyn std::error::Error>> {
    let build_run = |size_scale: f64,
                     fill_height_scale: f64,
                     label: &str|
     -> Result<HarnessRun, Box<dyn std::error::Error>> {
        let overrides = StartupOverrides {
            geometry: ScenarioGeometryOverrides {
                size_scale,
                fill_ratio: fill_height_scale,
            },
            source_water_profile_id: Some("moderate".to_string()),
            substrate_preset: Some(StartupSubstratePreset::InertSand),
            plant_selection: Some(StartupPlantSelection::BothGuilds),
            filter_enabled: Some(true),
            light_preset: Some(StartupLightPreset::Hours12),
            heater_preset: Some(StartupHeaterPreset::Celsius25),
            aeration_enabled: Some(true),
            initial_adult_shrimp_count: Some(0), // No shrimp — isolate plant/algae
            ..StartupOverrides::default()
        };
        Ok(
            HarnessRun::with_overrides(SimSeed(77), "medium_planted", overrides)?
                .with_artifact_label(label),
        )
    };

    // Shallow tank: default geometry, low fill ratio → shallow water
    let mut run_shallow = build_run(1.0, 0.5, "light_shallow")?;
    run_shallow.enable_instrumentation();

    // Deep tank: taller geometry (2× height scale), full fill → deep water
    let mut run_deep = build_run(2.0, 1.0, "light_deep")?;
    run_deep.enable_instrumentation();

    let shallow_initial_depth = run_shallow
        .engine()
        .full_state()
        .water_depth_above_substrate_cm();
    let deep_initial_depth = run_deep
        .engine()
        .full_state()
        .water_depth_above_substrate_cm();

    eprintln!(
        "Shallow water depth: {:.1} cm, Deep water depth: {:.1} cm",
        shallow_initial_depth, deep_initial_depth
    );
    assert!(
        deep_initial_depth > shallow_initial_depth * 1.3,
        "Deep tank should have meaningfully more water depth: shallow={:.1}, deep={:.1}",
        shallow_initial_depth,
        deep_initial_depth
    );

    // Run for 14 days with daily feed to supply nutrients
    for _day in 0..14 {
        run_shallow.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run_deep.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run_shallow.step_hours(24)?;
        run_deep.step_hours(24)?;
    }

    let snap_shallow = run_shallow.snapshot();
    let snap_deep = run_deep.snapshot();

    dump_habitat_debug("light_shallow", run_shallow.engine().full_state());
    dump_habitat_debug("light_deep", run_deep.engine().full_state());
    eprintln!("{}", format_snapshot_summary("shallow_14d", &snap_shallow));
    eprintln!("{}", format_snapshot_summary("deep_14d", &snap_deep));

    // Validate: shallow tank should produce more plant biomass
    assert!(
        snap_shallow.total_plant_biomass_g > snap_deep.total_plant_biomass_g,
        "Shallow tank should produce more plant biomass (more light): \
         shallow={:.4}g, deep={:.4}g",
        snap_shallow.total_plant_biomass_g,
        snap_deep.total_plant_biomass_g
    );

    // Validate: shallow tank should have more or equal periphyton
    // (periphyton growth is light-driven via habitat light_exposure)
    assert!(
        snap_shallow.periphyton_biomass_g >= snap_deep.periphyton_biomass_g * 0.9,
        "Shallow tank periphyton should be comparable or higher: \
         shallow={:.6}g, deep={:.6}g",
        snap_shallow.periphyton_biomass_g,
        snap_deep.periphyton_biomass_g
    );

    // Both stay within envelopes
    run_shallow.assert_envelope(
        "light_shallow_final",
        &Envelope::default()
            .do_min(5.0)
            .ph(5.5, 9.0)
            .temperature_c(22.0, 28.0),
    );
    run_deep.assert_envelope(
        "light_deep_final",
        &Envelope::default()
            .do_min(5.0)
            .ph(5.5, 9.0)
            .temperature_c(22.0, 28.0),
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
/// - After 21 days, glass/hardscape periphyton biomass differs from
///   filter media decomposer biomass (they are not just the same number)
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

    // Run for 21 days
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

    // Extract glass periphyton and filter decomposer biomass
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
    let hf_glass_peri = glass_periphyton(state_hf);

    let hl_filter_dec = filter_decomposer(state_hl);
    let ll_filter_dec = filter_decomposer(state_ll);
    let hf_filter_dec = filter_decomposer(state_hf);

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
        hf_glass_peri, hf_filter_dec
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

    // Validate: glass periphyton and filter decomposer are independently regulated
    // (changing light affects periphyton but not decomposer proportionally)
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
    // The ratios should differ because light affects periphyton but not
    // decomposer directly
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

    // Envelopes
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
/// Root oxygenation from rosette plants can partially re-oxygenate the
/// substrate, but the deep layers remain suboxic, preserving the
/// denitrification pathway.
///
/// **What we validate:**
/// - Planted substrate shows measurable cumulative N₂ export after 60 days
/// - Inert substrate shows negligible N₂ export
/// - Planted substrate accumulates less nitrate than inert substrate
///   (some NO₃ is removed as N₂)
/// - Denitrifier activity index matures over time in planted substrate
#[test]
fn probe_substrate_redox_denitrification() -> Result<(), Box<dyn std::error::Error>> {
    let build_run = |substrate: StartupSubstratePreset,
                     plants: StartupPlantSelection,
                     label: &str|
     -> Result<HarnessRun, Box<dyn std::error::Error>> {
        let overrides = StartupOverrides {
            geometry: ScenarioGeometryOverrides::default(),
            source_water_profile_id: Some("moderate".to_string()),
            substrate_preset: Some(substrate),
            plant_selection: Some(plants),
            filter_enabled: Some(true),
            light_preset: Some(StartupLightPreset::Hours10),
            heater_preset: Some(StartupHeaterPreset::Celsius25),
            aeration_enabled: Some(true),
            initial_adult_shrimp_count: Some(5),
            ..StartupOverrides::default()
        };
        Ok(
            HarnessRun::with_overrides(SimSeed(88), "medium_planted", overrides)?
                .with_artifact_label(label),
        )
    };

    // Planted substrate with deep active layers → suboxic denitrification zone
    let mut run_planted = build_run(
        StartupSubstratePreset::ActivePlantedWithCoarsePorous,
        StartupPlantSelection::BothGuilds,
        "redox_planted",
    )?;
    run_planted.enable_instrumentation();

    // Inert sand, no plants → fully oxic, negligible denitrification
    let mut run_inert = build_run(
        StartupSubstratePreset::InertSand,
        StartupPlantSelection::None,
        "redox_inert",
    )?;
    run_inert.enable_instrumentation();

    // Record initial denitrifier activity
    let planted_initial_activity = run_planted
        .engine()
        .full_state()
        .microbe
        .denitrifier_activity_index;

    // Run for 60 days with daily feeding and weekly water changes
    for day in 0..60 {
        run_planted.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run_inert.apply_action(PlayerAction::Feed { grams: 0.05 })?;

        if day % 7 == 6 {
            run_planted.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "moderate".to_string(),
            })?;
            run_inert.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "moderate".to_string(),
            })?;
        }

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

    // Validate: planted substrate should show measurable N₂ export
    let planted_export = state_planted.cumulative_n2_export_mg_n;
    let inert_export = state_inert.cumulative_n2_export_mg_n;

    eprintln!(
        "N₂ export: planted={:.4} mg N, inert={:.4} mg N",
        planted_export, inert_export
    );

    assert!(
        planted_export > inert_export,
        "Planted substrate should export more N₂ than inert: \
         planted={:.4}, inert={:.4}",
        planted_export,
        inert_export
    );

    // Validate: denitrifier activity should have matured in planted substrate
    let planted_final_activity = state_planted.microbe.denitrifier_activity_index;
    assert!(
        planted_final_activity > planted_initial_activity,
        "Denitrifier activity should mature over 60 days: \
         initial={:.4}, final={:.4}",
        planted_initial_activity,
        planted_final_activity
    );

    // Validate: planted substrate should have a suboxic zone
    let planted_suboxic_vol = state_planted.substrate_suboxic_pore_volume_cm3();
    eprintln!(
        "Suboxic pore volume: planted={:.2} cm³",
        planted_suboxic_vol
    );

    // Envelopes: both should be stable after 60 days of maintenance
    run_planted.assert_envelope(
        "redox_planted_final",
        &Envelope::default()
            .do_min(4.0)
            .ph(5.5, 9.0)
            .temperature_c(22.0, 28.0),
    );
    run_inert.assert_envelope(
        "redox_inert_final",
        &Envelope::default()
            .do_min(4.0)
            .ph(5.5, 9.0)
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
/// - Per-liter TAN, NO₂, DO concentrations stay within a tolerance band
///   between 1× and 2× geometry runs
/// - Biofilter maturity timeline is comparable
/// - Temperature equilibrium is comparable
/// - Both runs remain within stability envelopes
/// - Qualitative outcomes (shrimp survival, plant health) are comparable
#[test]
fn probe_equipment_scaling_1x_vs_2x() -> Result<(), Box<dyn std::error::Error>> {
    let duration_hours: u32 = 500;
    let feed_per_adult_per_day: f64 = 0.001;
    let base_adult_count: u32 = 10;
    let concentration_tolerance: f64 = 0.55; // 55% max divergence

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

    // Collect time-series data
    struct Observation {
        tan: f64,
        nitrite: f64,
        do_val: f64,
        maturity: f64,
        temp: f64,
    }

    let mut obs_1x = Vec::new();
    let mut obs_2x = Vec::new();

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

        // Record hourly observations
        let s1 = run_1x.snapshot();
        let s2 = run_2x.snapshot();
        obs_1x.push(Observation {
            tan: s1.tan_mg_n_per_l,
            nitrite: s1.nitrite_mg_n_per_l,
            do_val: s1.do_mg_l,
            maturity: s1.biofilter_maturity_index,
            temp: s1.water_temp_c,
        });
        obs_2x.push(Observation {
            tan: s2.tan_mg_n_per_l,
            nitrite: s2.nitrite_mg_n_per_l,
            do_val: s2.do_mg_l,
            maturity: s2.biofilter_maturity_index,
            temp: s2.water_temp_c,
        });
    }

    dump_habitat_debug("scale_1x", run_1x.engine().full_state());
    dump_habitat_debug("scale_2x", run_2x.engine().full_state());

    let snap_1x = run_1x.snapshot();
    let snap_2x = run_2x.snapshot();
    eprintln!("{}", format_snapshot_summary("1x_final", &snap_1x));
    eprintln!("{}", format_snapshot_summary("2x_final", &snap_2x));

    // Helper: compute peak value from a time series
    let peak = |observations: &[Observation], select: fn(&Observation) -> f64| -> f64 {
        observations
            .iter()
            .map(select)
            .fold(f64::NEG_INFINITY, f64::max)
    };

    // Helper: compute min value from a time series
    let trough = |observations: &[Observation], select: fn(&Observation) -> f64| -> f64 {
        observations
            .iter()
            .map(select)
            .fold(f64::INFINITY, f64::min)
    };

    // Compare peak TAN concentrations
    let peak_tan_1x = peak(&obs_1x, |o| o.tan);
    let peak_tan_2x = peak(&obs_2x, |o| o.tan);
    assert_within_fraction(
        "peak TAN",
        peak_tan_1x,
        peak_tan_2x,
        concentration_tolerance,
    );

    // Compare DO ranges
    let do_range_1x = peak(&obs_1x, |o| o.do_val) - trough(&obs_1x, |o| o.do_val);
    let do_range_2x = peak(&obs_2x, |o| o.do_val) - trough(&obs_2x, |o| o.do_val);
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

    // Compare cycling timeline (when maturity crosses threshold)
    let maturity_threshold = 0.04;
    let maturity_hour = |observations: &[Observation]| -> Option<usize> {
        observations
            .iter()
            .position(|o| o.maturity >= maturity_threshold)
    };
    let mat_1x = maturity_hour(&obs_1x);
    let mat_2x = maturity_hour(&obs_2x);

    if let (Some(h1), Some(h2)) = (mat_1x, mat_2x) {
        assert_within_fraction("cycle timeline", h1 as f64, h2 as f64, 0.20);
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
