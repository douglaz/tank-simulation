//! Calibration report workflow for comparing simulated outputs to target envelopes.
//!
//! Builds on the shared regression harness ([`HarnessRun`]) to produce structured,
//! machine-readable reports with pass/marginal/fail classification at each checkpoint.
//!
//! # Marginal classification
//!
//! A value is **marginal** when it lies within the envelope but within 10% of the
//! range width from either boundary. This early warning identifies parameters that
//! are close to failing without blocking CI.
//!
//! # Example
//!
//! ```rust,ignore
//! use tank_core::SimSeed;
//! use tank_harness::{Envelope, HarnessRun};
//! use tank_harness::calibration::{CalibrationRun, CalibrationReport};
//!
//! let run = HarnessRun::new(SimSeed(42), "medium_planted")?;
//! let mut cal = CalibrationRun::new(run, "default_params");
//! cal.enable_instrumentation();
//! cal.step_hours(168)?;
//! cal.check_envelope("week_1", &Envelope::default().ph(6.5, 8.5));
//! let row = cal.finish();
//! let report = CalibrationReport::from_rows("default_params", vec![row]);
//! println!("{}", serde_json::to_string_pretty(&report).unwrap());
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Envelope, HarnessError, HarnessRun};
use tank_core::{PlayerAction, SimulationEngine, TankSnapshot};

/// Marginal threshold as a fraction of the envelope range width.
const MARGINAL_FRACTION: f64 = 0.10;

// ---------------------------------------------------------------------------
// CheckStatus
// ---------------------------------------------------------------------------

/// Classification for a field, checkpoint, or scenario.
///
/// Ordered by severity: Pass < Marginal < Fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Marginal,
    Fail,
}

impl CheckStatus {
    /// Return the more severe of two statuses.
    pub fn worst(self, other: Self) -> Self {
        self.max(other)
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "pass"),
            Self::Marginal => write!(f, "marginal"),
            Self::Fail => write!(f, "fail"),
        }
    }
}

/// Confidence tier for a validation scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationConfidence {
    High,
    Medium,
    Low,
}

impl fmt::Display for ValidationConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
        }
    }
}

/// Scientific maturity marker for a validation scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStatus {
    ValidatedDirectionally,
    StillHeuristic,
}

impl fmt::Display for ProvenanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidatedDirectionally => write!(f, "validated directionally"),
            Self::StillHeuristic => write!(f, "still heuristic"),
        }
    }
}

// ---------------------------------------------------------------------------
// FieldCheck
// ---------------------------------------------------------------------------

/// Result of checking a single field against its envelope bounds.
/// Result of checking a single field against its envelope bounds.
///
/// Bounds use `Option<f64>`: `None` means unbounded (e.g., `do_min` has no
/// upper bound). `observed` is `None` when the snapshot value was non-finite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCheck {
    pub field: String,
    /// Observed value from the snapshot. `None` if non-finite (NaN/Inf).
    pub observed: Option<f64>,
    /// Lower envelope bound. `None` if unbounded.
    pub envelope_min: Option<f64>,
    /// Upper envelope bound. `None` if unbounded.
    pub envelope_max: Option<f64>,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// CheckpointRow
// ---------------------------------------------------------------------------

/// Calibration result for a single checkpoint within a scenario run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRow {
    pub label: String,
    pub day: u32,
    pub hour: u8,
    pub checkpoint_hours: u32,
    pub status: CheckStatus,
    pub fields: Vec<FieldCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
}

/// Labeled artifact link for a scenario report row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioArtifact {
    pub label: String,
    pub path: String,
}

// ---------------------------------------------------------------------------
// ScenarioRow
// ---------------------------------------------------------------------------

/// Calibration result for a single scenario run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRow {
    pub scenario_id: String,
    pub scenario_name: String,
    pub seed: u64,
    pub parameter_variant: String,
    pub domain: String,
    pub confidence: ValidationConfidence,
    pub provenance_status: ProvenanceStatus,
    pub status: CheckStatus,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub observed_summary: String,
    pub checkpoints: Vec<CheckpointRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ScenarioArtifact>,
}

/// Paths written by [`CalibrationReport::write_bundle`].
#[derive(Debug, Clone)]
pub struct CalibrationReportArtifacts {
    pub report_json: PathBuf,
    pub summary_txt: PathBuf,
}

/// Paths written by [`ComparisonReport::write_bundle`].
#[derive(Debug, Clone)]
pub struct ComparisonReportArtifacts {
    pub comparison_json: PathBuf,
    pub comparison_txt: PathBuf,
}

// ---------------------------------------------------------------------------
// ReportSummary
// ---------------------------------------------------------------------------

/// Aggregate status counts for a calibration report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total: usize,
    pub passed: usize,
    pub marginal: usize,
    pub failed: usize,
}

// ---------------------------------------------------------------------------
// CalibrationReport
// ---------------------------------------------------------------------------

/// Full calibration report across multiple scenario runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationReport {
    pub generated_at: String,
    pub parameter_set: String,
    pub summary: ReportSummary,
    pub scenarios: Vec<ScenarioRow>,
}

impl CalibrationReport {
    /// Build a report from completed scenario rows.
    pub fn from_rows(parameter_set: impl Into<String>, scenarios: Vec<ScenarioRow>) -> Self {
        let summary = ReportSummary {
            total: scenarios.len(),
            passed: scenarios
                .iter()
                .filter(|r| r.status == CheckStatus::Pass)
                .count(),
            marginal: scenarios
                .iter()
                .filter(|r| r.status == CheckStatus::Marginal)
                .count(),
            failed: scenarios
                .iter()
                .filter(|r| r.status == CheckStatus::Fail)
                .count(),
        };
        Self {
            generated_at: now_iso8601(),
            parameter_set: parameter_set.into(),
            summary,
            scenarios,
        }
    }

    /// Compare this report to another, returning status changes per scenario.
    ///
    /// Matches scenarios by `scenario_id`. Only scenarios present in both reports
    /// and whose status changed are included in the diff.
    pub fn compare(&self, other: &CalibrationReport) -> ComparisonReport {
        let mut changes = Vec::new();
        for row_before in &self.scenarios {
            if let Some(row_after) = other
                .scenarios
                .iter()
                .find(|r| r.scenario_id == row_before.scenario_id)
            {
                if row_before.status != row_after.status {
                    changes.push(StatusChange {
                        scenario_id: row_before.scenario_id.clone(),
                        before: row_before.status,
                        after: row_after.status,
                    });
                }
            }
        }
        ComparisonReport {
            before_parameter_set: self.parameter_set.clone(),
            after_parameter_set: other.parameter_set.clone(),
            changes,
        }
    }
}

// ---------------------------------------------------------------------------
// ComparisonReport
// ---------------------------------------------------------------------------

/// Diff between two calibration reports highlighting status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub before_parameter_set: String,
    pub after_parameter_set: String,
    pub changes: Vec<StatusChange>,
}

/// A single scenario's status change between two calibration runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChange {
    pub scenario_id: String,
    pub before: CheckStatus,
    pub after: CheckStatus,
}

// ---------------------------------------------------------------------------
// CalibrationRun
// ---------------------------------------------------------------------------

/// Wraps a [`HarnessRun`] to produce calibration rows with marginal classification.
///
/// Uses the shared regression harness for scenario execution, checkpoint capture,
/// and artifact packaging. Adds per-field marginal detection (within 10% of
/// envelope boundary) as a calibration-specific overlay.
pub struct CalibrationRun {
    inner: HarnessRun,
    parameter_variant: String,
    checkpoint_rows: Vec<CheckpointRow>,
}

impl CalibrationRun {
    /// Wrap an existing harness run for calibration reporting.
    pub fn new(inner: HarnessRun, parameter_variant: impl Into<String>) -> Self {
        Self {
            inner,
            parameter_variant: parameter_variant.into(),
            checkpoint_rows: Vec::new(),
        }
    }

    /// Access the underlying harness run.
    pub fn inner(&self) -> &HarnessRun {
        &self.inner
    }

    /// Mutable access to the underlying harness run.
    pub fn inner_mut(&mut self) -> &mut HarnessRun {
        &mut self.inner
    }

    /// Enable budget tracking and tracing instrumentation.
    pub fn enable_instrumentation(&mut self) {
        self.inner.enable_instrumentation();
    }

    /// Step the simulation forward by the given number of hours.
    pub fn step_hours(&mut self, hours: u32) -> Result<(), HarnessError> {
        self.inner.step_hours(hours)
    }

    /// Apply a player action.
    pub fn apply_action(&mut self, action: PlayerAction) -> Result<(), HarnessError> {
        self.inner.apply_action(action)
    }

    /// Check the current snapshot against an envelope with marginal classification.
    ///
    /// Records a checkpoint in the underlying harness for artifact capture.
    /// Failed fields are also recorded as harness failures so they appear in
    /// the standard `assertion_summary.txt` artifact.
    pub fn check_envelope(&mut self, label: &str, envelope: &Envelope) {
        let snap = self.inner.snapshot();
        let state = self.inner.engine().full_state();
        let day = state.environment.day;
        let hour = state.environment.hour_of_day;

        let fields = check_classified(&snap, envelope);
        let status = fields
            .iter()
            .map(|f| f.status)
            .fold(CheckStatus::Pass, CheckStatus::worst);

        // Record checkpoint in harness for artifact capture.
        self.inner.checkpoint(label);

        // Record failures in harness for assertion_summary.txt.
        for field in &fields {
            if field.status == CheckStatus::Fail {
                if let Some(detail) = &field.detail {
                    self.inner.record_failure(label, detail);
                }
            }
        }

        let artifact_path = if status != CheckStatus::Pass {
            Some(self.inner.artifact_dir().display().to_string())
        } else {
            None
        };

        self.checkpoint_rows.push(CheckpointRow {
            label: label.to_owned(),
            day,
            hour,
            checkpoint_hours: day * 24 + u32::from(hour),
            status,
            fields,
            artifact_path,
        });
    }

    /// Finish the run and produce a [`ScenarioRow`].
    ///
    /// Persists artifacts for non-passing scenarios, then consumes the
    /// underlying harness run. Any failures accumulated directly on the shared
    /// [`HarnessRun`] override the checkpoint-derived scenario status.
    pub fn finish(self) -> ScenarioRow {
        let Self {
            inner,
            parameter_variant,
            checkpoint_rows,
        } = self;

        let scenario_id = inner.scenario_id().to_owned();
        let seed = inner.seed().0;
        let checkpoint_status = checkpoint_rows
            .iter()
            .map(|c| c.status)
            .fold(CheckStatus::Pass, CheckStatus::worst);
        let harness_has_failures = !inner.failures().is_empty();
        let artifact_path = if checkpoint_status != CheckStatus::Pass || harness_has_failures {
            Some(inner.artifact_dir().display().to_string())
        } else {
            None
        };

        if checkpoint_status == CheckStatus::Marginal && !harness_has_failures {
            inner.persist_artifacts();
        }

        // Consume the harness (verbose trace dump, cleanup) and escalate the
        // scenario if the shared harness accumulated failures outside envelope
        // checkpoints (for example via `inner_mut().assert_snapshot()`).
        let finish_result = inner.finish();
        let overall_status = if finish_result.is_err() {
            CheckStatus::Fail
        } else {
            checkpoint_status
        };

        ScenarioRow {
            scenario_id,
            seed,
            parameter_variant,
            status: overall_status,
            checkpoints: checkpoint_rows,
            artifact_path,
        }
    }
}

// ---------------------------------------------------------------------------
// Classified envelope check
// ---------------------------------------------------------------------------

/// Check a snapshot against an envelope with per-field classification.
///
/// Returns a [`FieldCheck`] for each constrained field in the envelope.
/// Pass: comfortably within bounds. Marginal: within 10% of boundary.
/// Fail: outside bounds or non-finite.
pub fn check_classified(snap: &TankSnapshot, envelope: &Envelope) -> Vec<FieldCheck> {
    let mut checks = Vec::new();

    if let Some((min, max)) = envelope.ph_bounds {
        checks.push(classify_f64("ph", snap.ph, min, max));
    }
    if let Some((min, max)) = envelope.temperature_c_bounds {
        checks.push(classify_f64("temperature_c", snap.water_temp_c, min, max));
    }
    if let Some((min, max)) = envelope.tan_mg_n_per_l_bounds {
        checks.push(classify_f64(
            "tan_mg_n_per_l",
            snap.tan_mg_n_per_l,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.nitrite_mg_n_per_l_bounds {
        checks.push(classify_f64(
            "nitrite_mg_n_per_l",
            snap.nitrite_mg_n_per_l,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.nitrate_mg_n_per_l_bounds {
        checks.push(classify_f64(
            "nitrate_mg_n_per_l",
            snap.nitrate_mg_n_per_l,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.do_mg_l_bounds {
        checks.push(classify_f64("do_mg_l", snap.do_mg_l, min, max));
    }
    if let Some((min, max)) = envelope.shrimp_count_bounds {
        checks.push(classify_u32(
            "shrimp_count",
            snap.total_shrimp_count,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.juveniles_count_bounds {
        checks.push(classify_u32(
            "juveniles_count",
            snap.juveniles_count,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.berried_females_count_bounds {
        checks.push(classify_u32(
            "berried_females_count",
            snap.berried_females_count,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.shrimp_reproductive_readiness_bounds {
        checks.push(classify_f64(
            "shrimp_reproductive_readiness",
            snap.shrimp_reproductive_readiness,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.plant_biomass_g_bounds {
        checks.push(classify_f64(
            "plant_biomass_g",
            snap.total_plant_biomass_g,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.algae_nuisance_bounds {
        checks.push(classify_f64(
            "algae_nuisance",
            snap.algae_nuisance_index,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.biofilter_maturity_bounds {
        checks.push(classify_f64(
            "biofilter_maturity",
            snap.biofilter_maturity_index,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.fast_stem_biomass_g_bounds {
        checks.push(classify_f64(
            "fast_stem_biomass_g",
            snap.fast_stem_biomass_g,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.periphyton_biomass_g_bounds {
        checks.push(classify_f64(
            "periphyton_biomass_g",
            snap.periphyton_biomass_g,
            min,
            max,
        ));
    }
    if let Some((min, max)) = envelope.kh_d_bounds {
        checks.push(classify_f64("kh_d", snap.kh_d, min, max));
    }
    if let Some((min, max)) = envelope.dic_mg_c_per_l_bounds {
        checks.push(classify_f64(
            "dic_mg_c_per_l",
            snap.dissolved_inorganic_carbon_mg_c_per_l,
            min,
            max,
        ));
    }

    checks
}

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

fn finite_or_none(v: f64) -> Option<f64> {
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn classify_f64(field: &str, value: f64, min: f64, max: f64) -> FieldCheck {
    let envelope_min = finite_or_none(min);
    let envelope_max = finite_or_none(max);

    if !value.is_finite() {
        return FieldCheck {
            field: field.to_owned(),
            observed: None,
            envelope_min,
            envelope_max,
            status: CheckStatus::Fail,
            detail: Some(format!("{field} is non-finite")),
        };
    }

    if value < min || value > max {
        return FieldCheck {
            field: field.to_owned(),
            observed: Some(value),
            envelope_min,
            envelope_max,
            status: CheckStatus::Fail,
            detail: Some(format!("{field} {value:.4} outside [{min:.4}, {max:.4}]")),
        };
    }

    let margin = marginal_width(min, max);
    let near_lower = value - min < margin;
    let near_upper = max - value < margin;

    if near_lower || near_upper {
        FieldCheck {
            field: field.to_owned(),
            observed: Some(value),
            envelope_min,
            envelope_max,
            status: CheckStatus::Marginal,
            detail: Some(format!(
                "{field} {value:.4} within 10% of envelope boundary"
            )),
        }
    } else {
        FieldCheck {
            field: field.to_owned(),
            observed: Some(value),
            envelope_min,
            envelope_max,
            status: CheckStatus::Pass,
            detail: None,
        }
    }
}

fn classify_u32(field: &str, value: u32, min: u32, max: u32) -> FieldCheck {
    classify_f64(field, f64::from(value), f64::from(min), f64::from(max))
}

/// Compute the marginal-zone width for a given envelope range.
///
/// Finite ranges: 10% of (max - min).
/// One-sided (infinite max): 10% of |min|, floor 0.1.
/// One-sided (infinite min): 10% of |max|, floor 0.1.
fn marginal_width(min: f64, max: f64) -> f64 {
    if min.is_finite() && max.is_finite() {
        MARGINAL_FRACTION * (max - min)
    } else if max.is_infinite() && min.is_finite() {
        (MARGINAL_FRACTION * min.abs()).max(0.1)
    } else if min.is_infinite() && max.is_finite() {
        (MARGINAL_FRACTION * max.abs()).max(0.1)
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Timestamp helper
// ---------------------------------------------------------------------------

fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_epoch_utc(secs)
}

fn format_epoch_utc(epoch_secs: u64) -> String {
    let total_days = (epoch_secs / 86400) as i64;
    let time_of_day = epoch_secs % 86400;
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;

    // Howard Hinnant's civil_from_days algorithm.
    let z = total_days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_pass_within_bounds() {
        let check = classify_f64("temp", 25.0, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.detail.is_none());
    }

    #[test]
    fn classify_fail_below_bounds() {
        let check = classify_f64("temp", 19.0, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.detail.is_some());
    }

    #[test]
    fn classify_fail_above_bounds() {
        let check = classify_f64("temp", 31.0, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Fail);
    }

    #[test]
    fn classify_fail_non_finite() {
        let check = classify_f64("temp", f64::NAN, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.detail.as_ref().unwrap().contains("non-finite"));
    }

    #[test]
    fn classify_marginal_near_lower_bound() {
        // Range [20, 30], width 10, margin 1.0.
        // Value 20.5 is within margin of lower bound.
        let check = classify_f64("temp", 20.5, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Marginal);
    }

    #[test]
    fn classify_marginal_near_upper_bound() {
        // Range [20, 30], width 10, margin 1.0.
        // Value 29.5 is within margin of upper bound.
        let check = classify_f64("temp", 29.5, 20.0, 30.0);
        assert_eq!(check.status, CheckStatus::Marginal);
    }

    #[test]
    fn classify_marginal_one_sided_lower() {
        // do_min(6.0) => (6.0, inf), margin = 0.6.
        // Value 6.3 is within margin of lower bound.
        let check = classify_f64("do", 6.3, 6.0, f64::INFINITY);
        assert_eq!(check.status, CheckStatus::Marginal);
    }

    #[test]
    fn classify_pass_one_sided_comfortably_above() {
        // do_min(6.0) => (6.0, inf), margin = 0.6.
        // Value 8.0 is well above margin.
        let check = classify_f64("do", 8.0, 6.0, f64::INFINITY);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn classify_u32_delegates_correctly() {
        // Range [8, 10], width 2, margin 0.2.
        // Value 8 => 8-8=0 < 0.2 => marginal.
        let check = classify_u32("shrimp", 8, 8, 10);
        assert_eq!(check.status, CheckStatus::Marginal);
    }

    #[test]
    fn marginal_width_finite_range() {
        assert!((marginal_width(20.0, 30.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn marginal_width_infinite_upper() {
        assert!((marginal_width(6.0, f64::INFINITY) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn marginal_width_zero_range() {
        assert!((marginal_width(5.0, 5.0) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn check_status_worst_ordering() {
        assert_eq!(
            CheckStatus::Pass.worst(CheckStatus::Pass),
            CheckStatus::Pass
        );
        assert_eq!(
            CheckStatus::Pass.worst(CheckStatus::Marginal),
            CheckStatus::Marginal
        );
        assert_eq!(
            CheckStatus::Pass.worst(CheckStatus::Fail),
            CheckStatus::Fail
        );
        assert_eq!(
            CheckStatus::Marginal.worst(CheckStatus::Fail),
            CheckStatus::Fail
        );
    }

    #[test]
    fn format_epoch_utc_unix_epoch() {
        assert_eq!(format_epoch_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_epoch_utc_known_date() {
        // 2000-01-01T00:00:00Z = 946684800
        assert_eq!(format_epoch_utc(946_684_800), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn check_status_serializes_lowercase() -> Result<(), serde_json::Error> {
        assert_eq!(serde_json::to_string(&CheckStatus::Pass)?, "\"pass\"");
        assert_eq!(
            serde_json::to_string(&CheckStatus::Marginal)?,
            "\"marginal\""
        );
        assert_eq!(serde_json::to_string(&CheckStatus::Fail)?, "\"fail\"");
        Ok(())
    }
}
