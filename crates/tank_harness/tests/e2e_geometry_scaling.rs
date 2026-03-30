//! Integration tests: geometry scaling preserves relative dynamics.
//!
//! These tests compare 1× vs 2× geometry versions of the same scenario to verify
//! that scaled hardware, plant biomass, and microbes produce consistent relative
//! behavior rather than testing absolute values (which the unit tests cover).

use tank_core::{PlayerAction, SimSeed};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupOverrides, StartupPlantSelection, StartupSubstratePreset,
};

/// Build a medium_planted run at a given size_scale with standard overrides.
fn medium_planted_run(
    seed: u64,
    size_scale: f64,
    label: &str,
) -> Result<HarnessRun, Box<dyn std::error::Error>> {
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("moderate".to_string()),
        substrate_preset: Some(StartupSubstratePreset::ActivePlantedWithCoarsePorous),
        plant_selection: Some(StartupPlantSelection::BothGuilds),
        filter_enabled: Some(true),
        ..StartupOverrides::default()
    };
    let run = HarnessRun::with_overrides(SimSeed(seed), "medium_planted", overrides)?
        .with_artifact_label(label);
    Ok(run)
}

// ---------------------------------------------------------------------------
// 1× vs 2× geometry comparative test
// ---------------------------------------------------------------------------

/// Verifies that relative relationships between 1× and 2× geometry tanks hold:
/// - 2× has 8× volume, 4× footprint, 8× filter flow
/// - Both tanks cycle (biofilter maturity > floor after 30 days)
/// - Plant biomass is higher in 2× (4× footprint)
/// - Water chemistry is broadly similar (same feed-to-volume ratio)
#[test]
fn geometry_2x_preserves_relative_dynamics() -> Result<(), Box<dyn std::error::Error>> {
    let mut run_1x = medium_planted_run(42, 1.0, "geom_scale_1x")?;
    let mut run_2x = medium_planted_run(42, 2.0, "geom_scale_2x")?;
    run_1x.enable_instrumentation();
    run_2x.enable_instrumentation();

    // Record initial state
    let snap_1x_initial = run_1x.snapshot();
    let snap_2x_initial = run_2x.snapshot();

    // Verify initial scaling relationships
    // Net water volume ratio isn't exactly 8× because substrate depth doesn't
    // scale with geometry (fixed preset), so substrate displacement is a smaller
    // fraction in the 2× tank. Expect roughly 8–10×.
    let vol_ratio = snap_2x_initial.water_volume_l / snap_1x_initial.water_volume_l;
    assert!(
        vol_ratio > 7.5 && vol_ratio < 11.0,
        "2× geometry should have ~8-10× water volume: ratio={vol_ratio:.2}"
    );

    // 2× should have more initial plant biomass (4× footprint → 4× per guild)
    assert!(
        snap_2x_initial.total_plant_biomass_g > snap_1x_initial.total_plant_biomass_g * 3.0,
        "2× should have >3× plant biomass: {:.1}g vs {:.1}g",
        snap_2x_initial.total_plant_biomass_g,
        snap_1x_initial.total_plant_biomass_g
    );

    // Feed proportionally to volume: 0.1g for 1× (~58L), 0.8g for 2× (~460L)
    let feed_1x = 0.1;
    let feed_2x = feed_1x * vol_ratio;

    // Run 30 days of cycling with proportional feeding
    for _ in 1..=30 {
        run_1x.apply_action(PlayerAction::Feed { grams: feed_1x })?;
        run_2x.apply_action(PlayerAction::Feed { grams: feed_2x })?;
        run_1x.step_hours(24)?;
        run_2x.step_hours(24)?;
    }

    let snap_1x = run_1x.snapshot();
    let snap_2x = run_2x.snapshot();

    // Both should have measurable biofilter maturity after 30 days
    assert!(
        snap_1x.biofilter_maturity_index > 0.01,
        "1× biofilter should show maturity after 30 days: {:.4}",
        snap_1x.biofilter_maturity_index
    );
    assert!(
        snap_2x.biofilter_maturity_index > 0.01,
        "2× biofilter should show maturity after 30 days: {:.4}",
        snap_2x.biofilter_maturity_index
    );

    // With proportional feeding and scaled microbes, maturity should be in the
    // same order of magnitude (within 5× of each other)
    let maturity_ratio = snap_1x.biofilter_maturity_index / snap_2x.biofilter_maturity_index;
    assert!(
        maturity_ratio > 0.2 && maturity_ratio < 5.0,
        "maturity should be same order of magnitude: 1×={:.4}, 2×={:.4}, ratio={maturity_ratio:.2}",
        snap_1x.biofilter_maturity_index,
        snap_2x.biofilter_maturity_index
    );

    // Both should have similar pH (proportional feeding maintains concentration)
    assert!(
        (snap_1x.ph - snap_2x.ph).abs() < 2.0,
        "pH should be similar with proportional feeding: 1×={:.2}, 2×={:.2}",
        snap_1x.ph,
        snap_2x.ph
    );

    // Both should have similar temperature
    assert!(
        (snap_1x.water_temp_c - snap_2x.water_temp_c).abs() < 3.0,
        "temperature should be similar: 1×={:.1}C, 2×={:.1}C",
        snap_1x.water_temp_c,
        snap_2x.water_temp_c
    );

    // 2× should still have more plant biomass than 1×
    assert!(
        snap_2x.total_plant_biomass_g > snap_1x.total_plant_biomass_g,
        "2× should maintain more plant biomass: {:.1}g vs {:.1}g",
        snap_2x.total_plant_biomass_g,
        snap_1x.total_plant_biomass_g
    );

    run_1x.finish()?;
    run_2x.finish()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Envelope compliance at both scales
// ---------------------------------------------------------------------------

/// Both 1× and 2× tanks should pass a broad common envelope after cycling.
/// This verifies that scaling doesn't push parameters into unreasonable ranges.
#[test]
fn both_scales_pass_common_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let scales = [(1.0, "common_env_1x"), (2.0, "common_env_2x")];

    for (scale, label) in scales {
        let mut run = medium_planted_run(42, scale, label)?;

        // 30 days cycling with proportional feed
        let volume_l = run.snapshot().water_volume_l;
        let feed_g = 0.1 * volume_l / 57.6; // proportional to 0.1g for 57.6L base

        for _ in 1..=30 {
            run.apply_action(PlayerAction::Feed { grams: feed_g })?;
            run.step_hours(24)?;
        }

        // Broad envelope that both scales should satisfy
        run.assert_envelope(
            &format!("post_cycle_{scale}x"),
            &Envelope::default()
                .ph(5.0, 9.5)
                .temperature_c(20.0, 28.0)
                .do_min(6.0)
                .biofilter_maturity(0.01, 1.0)
                .plant_biomass_g(1.0, 100.0),
        );

        run.finish()?;
    }

    Ok(())
}
