//! Calibration report tests (tanksim-6e5.7.4).
//!
//! Validates the calibration report workflow: structured output, marginal
//! classification, artifact path inclusion, cross-run comparison, and
//! integration with the shared regression harness.
//!
//! Run all:    `cargo test --test calibration_report`
//! Verbose:    `TANK_E2E_VERBOSE=1 cargo test --test calibration_report -- --nocapture`

use tank_core::{PlayerAction, SimSeed, TankSnapshot, TankState};
use tank_harness::calibration::{
    check_classified, CalibrationReport, CalibrationRun, CheckStatus, ComparisonReport, ScenarioRow,
};
use tank_harness::{Envelope, HarnessRun};

// ---------------------------------------------------------------------------
// 1. test_calibration_report_structure
// ---------------------------------------------------------------------------

/// Report output includes: scenario_id, seed, parameter_variant, checkpoint_hours,
/// observed_values, envelope_bounds, pass/marginal/fail status.
#[test]
fn test_calibration_report_structure() -> Result<(), Box<dyn std::error::Error>> {
    let run =
        HarnessRun::new(SimSeed(9001), "medium_planted")?.with_artifact_label("report_structure");
    let mut cal = CalibrationRun::new(run, "default_params");
    cal.enable_instrumentation();
    cal.step_hours(168)?;

    cal.check_envelope(
        "week_1",
        &Envelope::default()
            .ph(4.0, 10.0)
            .tan_mg_n_per_l(0.0, 100.0)
            .do_min(2.0),
    );

    let row = cal.finish();

    assert_eq!(row.scenario_id, "medium_planted");
    assert_eq!(row.seed, 9001);
    assert_eq!(row.parameter_variant, "default_params");
    assert!(matches!(
        row.status,
        CheckStatus::Pass | CheckStatus::Marginal | CheckStatus::Fail
    ));

    assert_eq!(row.checkpoints.len(), 1);
    let cp = &row.checkpoints[0];
    assert_eq!(cp.label, "week_1");
    assert!(cp.checkpoint_hours > 0);
    assert!(!cp.fields.is_empty());

    for field in &cp.fields {
        assert!(!field.field.is_empty());
        assert!(
            field.observed.is_some() || field.status == CheckStatus::Fail,
            "non-finite observed value should be classified as fail"
        );
        assert!(matches!(
            field.status,
            CheckStatus::Pass | CheckStatus::Marginal | CheckStatus::Fail
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. test_report_machine_readable
// ---------------------------------------------------------------------------

/// Report output is valid JSON that can be parsed programmatically.
#[test]
fn test_report_machine_readable() -> Result<(), Box<dyn std::error::Error>> {
    let run =
        HarnessRun::new(SimSeed(9002), "medium_planted")?.with_artifact_label("machine_readable");
    let mut cal = CalibrationRun::new(run, "test_params");
    cal.step_hours(24)?;
    cal.check_envelope("day_1", &Envelope::default().ph(4.0, 10.0).do_min(2.0));

    let row = cal.finish();
    let report = CalibrationReport::from_rows("test_params", vec![row]);

    // Round-trip JSON serialization.
    let json = serde_json::to_string_pretty(&report)?;
    let parsed: CalibrationReport = serde_json::from_str(&json)?;

    assert_eq!(parsed.parameter_set, "test_params");
    assert_eq!(parsed.summary.total, 1);
    assert_eq!(parsed.scenarios.len(), 1);
    assert_eq!(parsed.scenarios[0].scenario_id, "medium_planted");
    assert!(!parsed.generated_at.is_empty());
    assert!(
        parsed.generated_at.contains('T'),
        "generated_at should be ISO-8601 format"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. test_report_includes_artifact_paths
// ---------------------------------------------------------------------------

/// Each failed or marginal row includes a path to the detailed trace/budget
/// artifact for that scenario run.
#[test]
fn test_report_includes_artifact_paths() -> Result<(), Box<dyn std::error::Error>> {
    // Craft a state where temperature is outside bounds.
    let mut state = TankState::new(SimSeed(9003));
    state.water.temperature_c = 35.0;
    let run = HarnessRun::from_state(SimSeed(9003), "medium_planted", state)
        .with_artifact_label("artifact_paths");
    let artifact_dir = run.artifact_dir();
    let _ = std::fs::remove_dir_all(&artifact_dir);

    let mut cal = CalibrationRun::new(run, "test_params");
    cal.enable_instrumentation();
    cal.check_envelope("hot_check", &Envelope::default().temperature_c(20.0, 30.0));

    let row = cal.finish();

    // Scenario-level artifact path for failed scenario.
    assert!(
        row.artifact_path.is_some(),
        "failed scenario should have artifact path"
    );

    // Checkpoint-level artifact path for failed checkpoint.
    let cp = &row.checkpoints[0];
    assert_eq!(cp.status, CheckStatus::Fail);
    assert!(
        cp.artifact_path.is_some(),
        "failed checkpoint should have artifact path"
    );

    // Artifact directory should exist on disk.
    assert!(
        artifact_dir.exists(),
        "artifact directory should exist at {}",
        artifact_dir.display()
    );

    let _ = std::fs::remove_dir_all(artifact_dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. test_report_comparison_across_runs
// ---------------------------------------------------------------------------

/// Two reports from different parameter sets can be compared: same scenario_id
/// rows are aligned, changes in pass/fail status are highlighted.
#[test]
fn test_report_comparison_across_runs() -> Result<(), Box<dyn std::error::Error>> {
    fn make_row(scenario_id: &str, status: CheckStatus, variant: &str) -> ScenarioRow {
        ScenarioRow {
            scenario_id: scenario_id.to_owned(),
            seed: 42,
            parameter_variant: variant.to_owned(),
            status,
            checkpoints: vec![],
            artifact_path: None,
        }
    }

    let report_v1 = CalibrationReport::from_rows(
        "params_v1",
        vec![
            make_row("nano_cycle", CheckStatus::Pass, "params_v1"),
            make_row("medium_planted", CheckStatus::Fail, "params_v1"),
            make_row("warm_room", CheckStatus::Marginal, "params_v1"),
        ],
    );
    let report_v2 = CalibrationReport::from_rows(
        "params_v2",
        vec![
            make_row("nano_cycle", CheckStatus::Marginal, "params_v2"),
            make_row("medium_planted", CheckStatus::Pass, "params_v2"),
            make_row("warm_room", CheckStatus::Marginal, "params_v2"),
        ],
    );

    let comparison = report_v1.compare(&report_v2);

    assert_eq!(comparison.before_parameter_set, "params_v1");
    assert_eq!(comparison.after_parameter_set, "params_v2");

    // nano_cycle changed Pass->Marginal, medium_planted Fail->Pass.
    // warm_room unchanged (Marginal->Marginal) should NOT appear.
    assert_eq!(comparison.changes.len(), 2);

    let nano = comparison
        .changes
        .iter()
        .find(|c| c.scenario_id == "nano_cycle")
        .expect("nano_cycle should have a status change");
    assert_eq!(nano.before, CheckStatus::Pass);
    assert_eq!(nano.after, CheckStatus::Marginal);

    let medium = comparison
        .changes
        .iter()
        .find(|c| c.scenario_id == "medium_planted")
        .expect("medium_planted should have a status change");
    assert_eq!(medium.before, CheckStatus::Fail);
    assert_eq!(medium.after, CheckStatus::Pass);

    // ComparisonReport should also be valid JSON.
    let json = serde_json::to_string(&comparison)?;
    let _: ComparisonReport = serde_json::from_str(&json)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. test_marginal_classification
// ---------------------------------------------------------------------------

/// A result within envelope but within 10% of the boundary is classified as
/// "marginal" (not just pass/fail).
#[test]
fn test_marginal_classification() -> Result<(), Box<dyn std::error::Error>> {
    // Temperature envelope: [25.0, 30.0], range=5, margin=0.5.
    let envelope = Envelope::default().temperature_c(25.0, 30.0);

    // Near lower bound -> marginal.
    let mut state = TankState::new(SimSeed(9005));
    state.water.temperature_c = 25.1;
    let snap = TankSnapshot::from_state(&state);
    let fields = check_classified(&snap, &envelope);
    let temp_check = fields
        .iter()
        .find(|f| f.field == "temperature_c")
        .expect("temperature_c field should be present");
    assert_eq!(
        temp_check.status,
        CheckStatus::Marginal,
        "25.1 should be marginal (within 10% of [25, 30] lower bound)"
    );

    // Near upper bound -> marginal.
    state.water.temperature_c = 29.8;
    let snap = TankSnapshot::from_state(&state);
    let fields = check_classified(&snap, &envelope);
    let temp_check = fields
        .iter()
        .find(|f| f.field == "temperature_c")
        .expect("temperature_c field should be present");
    assert_eq!(
        temp_check.status,
        CheckStatus::Marginal,
        "29.8 should be marginal (within 10% of [25, 30] upper bound)"
    );

    // Comfortably inside -> pass.
    state.water.temperature_c = 27.5;
    let snap = TankSnapshot::from_state(&state);
    let fields = check_classified(&snap, &envelope);
    let temp_check = fields
        .iter()
        .find(|f| f.field == "temperature_c")
        .expect("temperature_c field should be present");
    assert_eq!(
        temp_check.status,
        CheckStatus::Pass,
        "27.5 should be pass (center of [25, 30])"
    );

    // Outside bounds -> fail.
    state.water.temperature_c = 24.5;
    let snap = TankSnapshot::from_state(&state);
    let fields = check_classified(&snap, &envelope);
    let temp_check = fields
        .iter()
        .find(|f| f.field == "temperature_c")
        .expect("temperature_c field should be present");
    assert_eq!(
        temp_check.status,
        CheckStatus::Fail,
        "24.5 should be fail (below [25, 30])"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. test_report_uses_shared_harness
// ---------------------------------------------------------------------------

/// The calibration workflow calls the regression harness (A2e) rather than
/// reimplementing scenario execution.
#[test]
fn test_report_uses_shared_harness() -> Result<(), Box<dyn std::error::Error>> {
    let run =
        HarnessRun::new(SimSeed(9006), "medium_planted")?.with_artifact_label("shared_harness");
    let mut cal = CalibrationRun::new(run, "test_params");
    cal.enable_instrumentation();
    cal.step_hours(24)?;
    cal.check_envelope("day_1", &Envelope::default().ph(4.0, 10.0));

    // Verify the underlying HarnessRun has the checkpoint recorded.
    assert_eq!(
        cal.inner().checkpoints().len(),
        1,
        "harness should have recorded the checkpoint"
    );
    assert_eq!(cal.inner().checkpoints()[0].label, "day_1");

    // Verify the harness run is the one executing the scenario.
    assert_eq!(cal.inner().scenario_id(), "medium_planted");
    assert_eq!(cal.inner().seed().0, 9006);

    let _row = cal.finish();
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. test_full_calibration_workflow (integration)
// ---------------------------------------------------------------------------

/// Run calibration report on all shipped scenarios with current parameters.
/// Verify all scenarios produce a status (pass/marginal/fail). The report
/// should complete in < 60 seconds for CI viability.
#[test]
fn test_full_calibration_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    let shipped_scenarios = ["nano_cycle", "medium_planted", "warm_room"];
    let mut rows = Vec::new();

    for (i, scenario_id) in shipped_scenarios.iter().enumerate() {
        let seed = SimSeed(7400 + i as u64);
        let run = HarnessRun::new(seed, scenario_id)?
            .with_artifact_label(&format!("calibration_{scenario_id}"));
        let mut cal = CalibrationRun::new(run, "default");
        cal.enable_instrumentation();

        // 4 weeks of cycling with daily feed.
        for day in 1..=28 {
            cal.apply_action(PlayerAction::Feed { grams: 0.05 })?;
            cal.step_hours(24)?;

            if day == 7 {
                cal.check_envelope(
                    "week_1",
                    &Envelope::default()
                        .ph(4.0, 10.0)
                        .tan_mg_n_per_l(0.0, 80.0)
                        .do_min(4.0),
                );
            }
            if day == 28 {
                cal.check_envelope(
                    "week_4",
                    &Envelope::default()
                        .ph(4.0, 10.0)
                        .tan_mg_n_per_l(0.0, 150.0)
                        .do_min(3.0)
                        .biofilter_maturity(0.0, 1.0),
                );
            }
        }

        rows.push(cal.finish());
    }

    let report = CalibrationReport::from_rows("default", rows);

    // All scenarios should produce a status.
    assert_eq!(report.summary.total, 3);
    assert_eq!(
        report.summary.passed + report.summary.marginal + report.summary.failed,
        3,
        "every scenario must have exactly one status"
    );

    // Each scenario has both checkpoints.
    for row in &report.scenarios {
        assert_eq!(
            row.checkpoints.len(),
            2,
            "expected 2 checkpoints for {}",
            row.scenario_id
        );
        for cp in &row.checkpoints {
            assert!(!cp.label.is_empty());
            assert!(cp.checkpoint_hours > 0);
            assert!(!cp.fields.is_empty());
        }
    }

    // Report should be valid JSON.
    let json = serde_json::to_string_pretty(&report)?;
    let parsed: CalibrationReport = serde_json::from_str(&json)?;
    assert_eq!(parsed.scenarios.len(), 3);

    // CI viability: < 60 seconds.
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 60,
        "calibration workflow took {elapsed:?}, expected < 60s"
    );

    Ok(())
}
