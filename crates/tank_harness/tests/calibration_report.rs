//! Calibration report tests (tanksim-6e5.7.4).
//!
//! Validates the calibration report workflow: structured output, marginal
//! classification, artifact path inclusion, cross-run comparison, and
//! integration with the shared regression harness.
//!
//! Run all:    `cargo test --test calibration_report`
//! Verbose:    `TANK_E2E_VERBOSE=1 cargo test --test calibration_report -- --nocapture`

use std::time::{SystemTime, UNIX_EPOCH};

use tank_core::{SimSeed, TankSnapshot, TankState};
use tank_harness::calibration::{
    check_classified, CalibrationReport, CalibrationRun, CheckStatus, ComparisonReport,
    ProvenanceStatus, ScenarioRow, ValidationConfidence,
};
use tank_harness::validation_suite::{run_calibration_suite, validation_scenarios};
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

    let bundle_dir = std::env::temp_dir().join(format!(
        "tank_harness_report_bundle_{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let artifacts = report.write_bundle(&bundle_dir)?;
    assert!(artifacts.report_json.exists());
    assert!(artifacts.summary_txt.exists());
    let summary = std::fs::read_to_string(&artifacts.summary_txt)?;
    assert!(summary.contains("CALIBRATION REPORT"));
    let _ = std::fs::remove_dir_all(bundle_dir);

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
    assert_eq!(row.artifacts.len(), 1);

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
// 4. test_finish_propagates_shared_harness_failures
// ---------------------------------------------------------------------------

/// Calibration rows fail if the shared harness records assertions outside the
/// checkpoint envelope workflow.
#[test]
fn test_finish_propagates_shared_harness_failures() -> Result<(), Box<dyn std::error::Error>> {
    let run = HarnessRun::new(SimSeed(9004), "medium_planted")?
        .with_artifact_label("shared_harness_failures");
    let artifact_dir = run.artifact_dir();
    let _ = std::fs::remove_dir_all(&artifact_dir);

    let mut cal = CalibrationRun::new(run, "test_params");
    cal.step_hours(24)?;
    cal.check_envelope("day_1", &Envelope::default().ph(4.0, 10.0));
    cal.inner_mut().assert_snapshot("custom_assertion", |_| {
        Err("synthetic shared-harness failure".to_owned())
    });

    let row = cal.finish();
    let artifact_dir_string = artifact_dir.display().to_string();

    assert_eq!(
        row.status,
        CheckStatus::Fail,
        "shared harness failures must escalate the scenario row"
    );
    assert_eq!(
        row.checkpoints[0].status,
        CheckStatus::Pass,
        "checkpoint classification should remain independent from scenario-level harness failures"
    );
    assert_eq!(
        row.artifact_path.as_deref(),
        Some(artifact_dir_string.as_str()),
        "scenario row should expose the harness artifact path"
    );
    assert!(
        artifact_dir.exists(),
        "artifact directory should exist at {}",
        artifact_dir.display()
    );

    let _ = std::fs::remove_dir_all(artifact_dir);
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. test_report_comparison_across_runs
// ---------------------------------------------------------------------------

/// Two reports from different parameter sets can be compared: same scenario_id
/// rows are aligned, changes in pass/fail status are highlighted.
#[test]
fn test_report_comparison_across_runs() -> Result<(), Box<dyn std::error::Error>> {
    fn make_row(scenario_id: &str, status: CheckStatus, variant: &str) -> ScenarioRow {
        ScenarioRow {
            scenario_id: scenario_id.to_owned(),
            scenario_name: scenario_id.to_owned(),
            seed: 42,
            parameter_variant: variant.to_owned(),
            domain: String::new(),
            confidence: ValidationConfidence::High,
            provenance_status: ProvenanceStatus::ValidatedDirectionally,
            status,
            observed_summary: String::new(),
            checkpoints: vec![],
            artifact_path: None,
            artifacts: vec![],
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
// 6. test_marginal_classification
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
// 7. test_report_uses_shared_harness
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
// 8. test_full_calibration_workflow (integration)
// ---------------------------------------------------------------------------

/// Run calibration report on all shipped scenarios with current parameters.
/// Verify all scenarios produce a status (pass/marginal/fail). The report
/// should complete in < 60 seconds for CI viability.
#[test]
fn test_full_calibration_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let report = run_calibration_suite("default")?;

    // All scenarios should produce a status.
    assert_eq!(report.summary.total, validation_scenarios().len());
    assert_eq!(
        report.summary.passed + report.summary.marginal + report.summary.failed,
        validation_scenarios().len(),
        "every scenario must have exactly one status"
    );

    let expected_ids: Vec<&str> = validation_scenarios()
        .iter()
        .map(|scenario| scenario.id)
        .collect();
    let actual_ids: Vec<&str> = report
        .scenarios
        .iter()
        .map(|row| row.scenario_id.as_str())
        .collect();
    assert_eq!(
        actual_ids, expected_ids,
        "workflow should cover the shipped validation suite"
    );

    for (row, definition) in report.scenarios.iter().zip(validation_scenarios().iter()) {
        assert_eq!(row.scenario_name, definition.title);
        assert_eq!(row.domain, definition.domain);
        assert_eq!(row.confidence, definition.confidence);
        assert_eq!(row.provenance_status, definition.provenance_status);
        assert!(!row.observed_summary.is_empty());
        assert!(
            !row.checkpoints.is_empty(),
            "expected at least one checkpoint for {}",
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
    assert_eq!(parsed.scenarios.len(), validation_scenarios().len());

    let bundle_dir = std::env::temp_dir().join(format!(
        "tank_harness_calibration_workflow_{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let artifacts = report.write_bundle(&bundle_dir)?;
    assert!(artifacts.report_json.exists());
    assert!(artifacts.summary_txt.exists());
    let _ = std::fs::remove_dir_all(bundle_dir);

    // CI viability: < 60 seconds.
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 60,
        "calibration workflow took {elapsed:?}, expected < 60s"
    );

    Ok(())
}
