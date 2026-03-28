//! E2e harness exemplar: medium planted tank with weekly maintenance.
//!
//! Demonstrates seeded scenario execution, periodic envelope assertions,
//! budget/tracing instrumentation, and deterministic checkpoint capture.

use tank_core::{PlayerAction, SimSeed};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

#[test]
fn medium_planted_weekly_maintenance_envelope() -> Result<(), Box<dyn std::error::Error>> {
    let overrides = StartupOverrides {
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
    };

    let mut run = HarnessRun::with_overrides(SimSeed(42), "medium_planted", overrides)?
        .with_artifact_label("medium_planted_weekly_maintenance");
    run.enable_instrumentation();

    // Phase 1: Fishless cycle (30 days, no feeding, no shrimp yet)
    run.step_hours(24 * 30)?;

    let cycle_envelope = Envelope::default()
        .ph(5.5, 8.5)
        .temperature_c(22.0, 28.0)
        .do_min(1.0);
    run.assert_envelope("post_cycle", &cycle_envelope);

    // Phase 2: Stocked with weekly maintenance (60 days)
    let maintenance_envelope = Envelope::default()
        .ph(5.5, 8.5)
        .temperature_c(22.0, 28.0)
        .do_min(1.0)
        .plant_biomass_g(0.0, 500.0);

    for week in 1..=8 {
        // Daily feeding for 7 days
        for _ in 0..7 {
            run.apply_action(PlayerAction::Feed { grams: 0.2 })?;
            run.step_hours(24)?;
        }

        // Weekly water change
        run.apply_action(PlayerAction::WaterChangePercent {
            percent: 25.0,
            source_profile_id: "moderate".to_string(),
        })?;
        run.step_hours(1)?;

        run.assert_envelope(&format!("week_{week}"), &maintenance_envelope);
    }

    // Final checkpoint with snapshot assertion
    run.assert_snapshot("final_finite", |snap| {
        if !snap.ph.is_finite() {
            return Err("pH is not finite".to_string());
        }
        if !snap.water_temp_c.is_finite() {
            return Err("temperature is not finite".to_string());
        }
        if !snap.do_mg_l.is_finite() {
            return Err("DO is not finite".to_string());
        }
        Ok(())
    });

    run.finish().map_err(|e| e.into())
}

#[test]
fn medium_planted_determinism() -> Result<(), Box<dyn std::error::Error>> {
    let seed = SimSeed(99);

    // Run the same scenario twice
    let checkpoints_a = run_short_scenario(seed)?;
    let checkpoints_b = run_short_scenario(seed)?;

    // Same seed + same schedule = identical checkpoints
    assert_eq!(
        checkpoints_a.len(),
        checkpoints_b.len(),
        "checkpoint counts must match"
    );
    for (a, b) in checkpoints_a.iter().zip(checkpoints_b.iter()) {
        assert_eq!(a.label, b.label);
        assert_eq!(a.day, b.day);
        assert_eq!(a.hour, b.hour);
        let json_a = serde_json::to_string(&a.snapshot)?;
        let json_b = serde_json::to_string(&b.snapshot)?;
        assert_eq!(json_a, json_b, "checkpoint '{}' diverged", a.label);
    }

    Ok(())
}

fn run_short_scenario(
    seed: SimSeed,
) -> Result<Vec<tank_harness::Checkpoint>, Box<dyn std::error::Error>> {
    let mut run =
        HarnessRun::new(seed, "medium_planted")?.with_artifact_label("medium_planted_determinism");
    run.enable_instrumentation();

    for day in 1..=14 {
        run.apply_action(PlayerAction::Feed { grams: 0.15 })?;
        run.step_hours(24)?;

        if day % 7 == 0 {
            run.checkpoint(&format!("day_{day}"));
        }
    }

    let checkpoints = run.checkpoints().to_vec();
    run.finish()?;
    Ok(checkpoints)
}
