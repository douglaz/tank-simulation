//! E2e harness exemplar: nano tank degradation under neglect.
//!
//! Demonstrates the harness detecting envelope violations when a tank is
//! overfed and abandoned, and verifies that failure artifacts are written
//! to a deterministic path.

use tank_core::{PlayerAction, SimSeed};
use tank_harness::{Envelope, HarnessRun};
use tank_scenarios::{
    ScenarioGeometryOverrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

#[test]
fn nano_neglect_artifacts_on_violation() -> Result<(), Box<dyn std::error::Error>> {
    let overrides = StartupOverrides {
        geometry: ScenarioGeometryOverrides {
            size_scale: 0.5,
            fill_ratio: 1.0,
        },
        source_water_profile_id: Some("soft_acidic".to_string()),
        substrate_preset: Some(StartupSubstratePreset::InertSand),
        plant_selection: Some(StartupPlantSelection::FastStemOnly),
        filter_enabled: Some(true),
        light_preset: Some(StartupLightPreset::Hours6),
        heater_preset: Some(StartupHeaterPreset::Celsius25),
        aeration_enabled: Some(false),
        initial_adult_shrimp_count: Some(5),
    };

    let mut run = HarnessRun::with_overrides(SimSeed(77), "nano_cycle", overrides)?;
    run.enable_instrumentation();

    // Overfeed for a week, then abandon
    for _ in 0..7 {
        run.apply_action(PlayerAction::Feed { grams: 1.5 })?;
        run.step_hours(24)?;
    }

    // 30 days of neglect (no feeding, no water changes)
    run.step_hours(24 * 30)?;

    // This envelope is intentionally strict — we expect violations after neglect
    let strict_envelope = Envelope::default()
        .ph(6.5, 7.5)
        .tan_mg_l(0.0, 0.5)
        .nitrite_mg_l(0.0, 0.5)
        .do_min(5.0);

    run.assert_envelope("post_neglect", &strict_envelope);

    let artifact_dir = run.artifact_dir();
    let result = run.finish();

    // We expect failures from the strict envelope
    assert!(
        result.is_err(),
        "strict envelope should produce violations after neglect"
    );
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("assertion failure"),
        "error should mention assertion failures"
    );
    assert!(
        err_msg.contains("artifacts:"),
        "error should include artifact path"
    );

    // Verify artifacts were written
    assert!(
        artifact_dir.join("metadata.json").exists(),
        "metadata.json should exist"
    );
    assert!(
        artifact_dir.join("checkpoints.json").exists(),
        "checkpoints.json should exist"
    );
    assert!(
        artifact_dir.join("assertion_summary.txt").exists(),
        "assertion_summary.txt should exist"
    );
    assert!(
        artifact_dir.join("budget_ledger.json").exists(),
        "budget_ledger.json should exist (instrumentation was enabled)"
    );
    assert!(
        artifact_dir.join("trace.jsonl").exists(),
        "trace.jsonl should exist (instrumentation was enabled)"
    );

    // Verify metadata.json is valid and contains expected fields
    let meta_raw = std::fs::read_to_string(artifact_dir.join("metadata.json"))?;
    let meta: serde_json::Value = serde_json::from_str(&meta_raw)?;
    assert_eq!(meta["seed"], 77);
    assert_eq!(meta["scenario_id"], "nano_cycle");
    assert!(
        meta["assertion_failures"].as_array().unwrap().len() > 0,
        "metadata should record assertion failures"
    );

    // Verify trace.jsonl contains valid JSON lines
    let trace_raw = std::fs::read_to_string(artifact_dir.join("trace.jsonl"))?;
    let first_line = trace_raw
        .lines()
        .next()
        .expect("trace should have at least one line");
    let tick: serde_json::Value = serde_json::from_str(first_line)?;
    assert!(
        tick["tick_index"].is_number(),
        "trace line should have tick_index"
    );

    // Deterministic path check: same seed+scenario = same path
    let expected_dir = std::env::temp_dir()
        .join("tank_harness")
        .join("nano_cycle_seed77");
    assert_eq!(artifact_dir, expected_dir);

    // Clean up artifacts
    let _ = std::fs::remove_dir_all(&artifact_dir);

    Ok(())
}

#[test]
fn nano_healthy_no_violations() -> Result<(), Box<dyn std::error::Error>> {
    let mut run = HarnessRun::new(SimSeed(55), "nano_cycle")?;
    run.enable_instrumentation();

    // Gentle care for 14 days
    let gentle_envelope = Envelope::default()
        .ph(4.0, 9.5)
        .temperature_c(15.0, 35.0)
        .do_min(0.1);

    for day in 1..=14 {
        run.apply_action(PlayerAction::Feed { grams: 0.05 })?;
        run.step_hours(24)?;

        if day % 7 == 0 {
            run.apply_action(PlayerAction::WaterChangePercent {
                percent: 20.0,
                source_profile_id: "soft_acidic".to_string(),
            })?;
            run.step_hours(1)?;
            run.assert_envelope(&format!("week_{}", day / 7), &gentle_envelope);
        }
    }

    // Should pass — wide envelope with gentle care
    run.finish().map_err(|e| e.into())
}
