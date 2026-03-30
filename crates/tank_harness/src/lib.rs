//! Deterministic scientific-regression harness for seeded scenario runs and e2e checks.
//!
//! Provides [`HarnessRun`] for scenario execution with checkpoint capture, envelope
//! assertions, and failure artifact packaging. Reuses the budget/tracing surfaces
//! from `tank_core` (budget ledger + simulation tracing) rather than inventing
//! parallel inspection formats.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use tank_core::SimSeed;
//! use tank_harness::{HarnessRun, Envelope};
//!
//! let mut run = HarnessRun::new(SimSeed(42), "medium_planted")?;
//! run.enable_instrumentation();
//! run.step_hours(24 * 7)?;
//! run.assert_envelope("week_1", &Envelope::default().ph(6.0, 8.5).do_min(2.0));
//! run.finish().unwrap();
//! ```
//!
//! # Verbose mode
//!
//! Set `TANK_E2E_VERBOSE=1` to emit full trace output on successful runs
//! without changing assertions.
//!
//! # Failure artifacts
//!
//! On assertion failure, [`HarnessRun::finish`] writes a structured artifact
//! directory to a deterministic temp path containing:
//! - `metadata.json` — seed, scenario_id, simulated time, failure list
//! - `checkpoints.json` — all recorded checkpoint snapshots
//! - `assertion_summary.txt` — human-readable failure summary
//! - `budget_ledger.json` — budget ledger (if tracking was enabled)
//! - `trace.jsonl` — JSON-lines trace output (if tracing was enabled)

pub mod calibration;

use std::fmt;
use std::path::PathBuf;

use tank_core::{
    Engine, JsonLinesSink, PlayerAction, SimSeed, SimTracer, SimulationEngine, TankSnapshot,
    TankState, TraceSink, Verbosity,
};
use tank_scenarios::StartupOverrides;

// ---------------------------------------------------------------------------
// Checkpoint
// ---------------------------------------------------------------------------

/// A named snapshot of simulation state at a particular point in time.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Checkpoint {
    pub label: String,
    pub day: u32,
    pub hour: u8,
    pub snapshot: TankSnapshot,
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// Bounds on snapshot fields for regression assertions.
///
/// Each field is `Option<(min, max)>`. `None` means unconstrained.
/// Use the builder methods to set individual bounds.
#[derive(Debug, Clone, Default)]
pub struct Envelope {
    pub ph_bounds: Option<(f64, f64)>,
    pub temperature_c_bounds: Option<(f64, f64)>,
    pub tan_mg_n_per_l_bounds: Option<(f64, f64)>,
    pub nitrite_mg_n_per_l_bounds: Option<(f64, f64)>,
    pub nitrate_mg_n_per_l_bounds: Option<(f64, f64)>,
    pub do_mg_l_bounds: Option<(f64, f64)>,
    pub shrimp_count_bounds: Option<(u32, u32)>,
    pub juveniles_count_bounds: Option<(u32, u32)>,
    pub berried_females_count_bounds: Option<(u32, u32)>,
    pub shrimp_reproductive_readiness_bounds: Option<(f64, f64)>,
    pub plant_biomass_g_bounds: Option<(f64, f64)>,
    pub algae_nuisance_bounds: Option<(f64, f64)>,
    pub biofilter_maturity_bounds: Option<(f64, f64)>,
    pub fast_stem_biomass_g_bounds: Option<(f64, f64)>,
    pub periphyton_biomass_g_bounds: Option<(f64, f64)>,
    pub kh_d_bounds: Option<(f64, f64)>,
    pub dic_mg_c_per_l_bounds: Option<(f64, f64)>,
}

impl Envelope {
    pub fn ph(mut self, min: f64, max: f64) -> Self {
        self.ph_bounds = Some((min, max));
        self
    }

    pub fn temperature_c(mut self, min: f64, max: f64) -> Self {
        self.temperature_c_bounds = Some((min, max));
        self
    }

    pub fn tan_mg_n_per_l(mut self, min: f64, max: f64) -> Self {
        self.tan_mg_n_per_l_bounds = Some((min, max));
        self
    }

    pub fn nitrite_mg_n_per_l(mut self, min: f64, max: f64) -> Self {
        self.nitrite_mg_n_per_l_bounds = Some((min, max));
        self
    }

    pub fn nitrate_mg_n_per_l(mut self, min: f64, max: f64) -> Self {
        self.nitrate_mg_n_per_l_bounds = Some((min, max));
        self
    }

    pub fn do_min(mut self, min: f64) -> Self {
        self.do_mg_l_bounds = Some((min, f64::INFINITY));
        self
    }

    pub fn do_mg_l(mut self, min: f64, max: f64) -> Self {
        self.do_mg_l_bounds = Some((min, max));
        self
    }

    pub fn shrimp_count(mut self, min: u32, max: u32) -> Self {
        self.shrimp_count_bounds = Some((min, max));
        self
    }

    pub fn juveniles_count(mut self, min: u32, max: u32) -> Self {
        self.juveniles_count_bounds = Some((min, max));
        self
    }

    pub fn berried_females_count(mut self, min: u32, max: u32) -> Self {
        self.berried_females_count_bounds = Some((min, max));
        self
    }

    pub fn shrimp_reproductive_readiness(mut self, min: f64, max: f64) -> Self {
        self.shrimp_reproductive_readiness_bounds = Some((min, max));
        self
    }

    pub fn plant_biomass_g(mut self, min: f64, max: f64) -> Self {
        self.plant_biomass_g_bounds = Some((min, max));
        self
    }

    pub fn algae_nuisance(mut self, min: f64, max: f64) -> Self {
        self.algae_nuisance_bounds = Some((min, max));
        self
    }

    pub fn biofilter_maturity(mut self, min: f64, max: f64) -> Self {
        self.biofilter_maturity_bounds = Some((min, max));
        self
    }

    pub fn fast_stem_biomass_g(mut self, min: f64, max: f64) -> Self {
        self.fast_stem_biomass_g_bounds = Some((min, max));
        self
    }

    pub fn periphyton_biomass_g(mut self, min: f64, max: f64) -> Self {
        self.periphyton_biomass_g_bounds = Some((min, max));
        self
    }

    pub fn kh_d(mut self, min: f64, max: f64) -> Self {
        self.kh_d_bounds = Some((min, max));
        self
    }

    pub fn dic_mg_c_per_l(mut self, min: f64, max: f64) -> Self {
        self.dic_mg_c_per_l_bounds = Some((min, max));
        self
    }

    /// Check a snapshot against this envelope, returning all violations.
    pub fn check(&self, snap: &TankSnapshot) -> Vec<String> {
        let mut violations = Vec::new();

        if let Some((min, max)) = self.ph_bounds {
            check_f64_bounds(
                &mut violations,
                "pH",
                snap.ph,
                (min, max),
                |value, min, max| format!("pH {value:.3} outside [{min:.1}, {max:.1}]"),
            );
        }
        if let Some((min, max)) = self.temperature_c_bounds {
            check_f64_bounds(
                &mut violations,
                "temperature",
                snap.water_temp_c,
                (min, max),
                |value, min, max| format!("temp {value:.2}C outside [{min:.1}, {max:.1}]"),
            );
        }
        if let Some((min, max)) = self.tan_mg_n_per_l_bounds {
            check_f64_bounds(
                &mut violations,
                "TAN",
                snap.tan_mg_n_per_l,
                (min, max),
                |value, min, max| format!("TAN {value:.4} mg N/L outside [{min:.3}, {max:.3}]"),
            );
        }
        if let Some((min, max)) = self.nitrite_mg_n_per_l_bounds {
            check_f64_bounds(
                &mut violations,
                "NO2",
                snap.nitrite_mg_n_per_l,
                (min, max),
                |value, min, max| format!("NO2 {value:.4} mg N/L outside [{min:.3}, {max:.3}]"),
            );
        }
        if let Some((min, max)) = self.nitrate_mg_n_per_l_bounds {
            check_f64_bounds(
                &mut violations,
                "NO3",
                snap.nitrate_mg_n_per_l,
                (min, max),
                |value, min, max| format!("NO3 {value:.4} mg N/L outside [{min:.3}, {max:.3}]"),
            );
        }
        if let Some((min, max)) = self.do_mg_l_bounds {
            check_f64_bounds(
                &mut violations,
                "DO",
                snap.do_mg_l,
                (min, max),
                |value, min, max| format!("DO {value:.3} mg/L outside [{min:.1}, {max:.1}]"),
            );
        }
        if let Some((min, max)) = self.shrimp_count_bounds {
            let total = snap.total_shrimp_count;
            if total < min || total > max {
                violations.push(format!("shrimp count {} outside [{}, {}]", total, min, max));
            }
        }
        if let Some((min, max)) = self.juveniles_count_bounds {
            if snap.juveniles_count < min || snap.juveniles_count > max {
                violations.push(format!(
                    "juveniles {} outside [{}, {}]",
                    snap.juveniles_count, min, max
                ));
            }
        }
        if let Some((min, max)) = self.berried_females_count_bounds {
            if snap.berried_females_count < min || snap.berried_females_count > max {
                violations.push(format!(
                    "berried females {} outside [{}, {}]",
                    snap.berried_females_count, min, max
                ));
            }
        }
        if let Some((min, max)) = self.shrimp_reproductive_readiness_bounds {
            check_f64_bounds(
                &mut violations,
                "shrimp reproductive readiness",
                snap.shrimp_reproductive_readiness,
                (min, max),
                |value, min, max| {
                    format!("shrimp reproductive readiness {value:.3} outside [{min:.2}, {max:.2}]")
                },
            );
        }
        if let Some((min, max)) = self.plant_biomass_g_bounds {
            check_f64_bounds(
                &mut violations,
                "plant biomass",
                snap.total_plant_biomass_g,
                (min, max),
                |value, min, max| format!("plant biomass {value:.3}g outside [{min:.2}, {max:.2}]"),
            );
        }
        if let Some((min, max)) = self.algae_nuisance_bounds {
            check_f64_bounds(
                &mut violations,
                "algae nuisance",
                snap.algae_nuisance_index,
                (min, max),
                |value, min, max| format!("algae nuisance {value:.3} outside [{min:.2}, {max:.2}]"),
            );
        }
        if let Some((min, max)) = self.biofilter_maturity_bounds {
            check_f64_bounds(
                &mut violations,
                "biofilter maturity",
                snap.biofilter_maturity_index,
                (min, max),
                |value, min, max| {
                    format!("biofilter maturity {value:.3} outside [{min:.2}, {max:.2}]")
                },
            );
        }
        if let Some((min, max)) = self.fast_stem_biomass_g_bounds {
            check_f64_bounds(
                &mut violations,
                "fast stem biomass",
                snap.fast_stem_biomass_g,
                (min, max),
                |value, min, max| {
                    format!("fast stem biomass {value:.3}g outside [{min:.2}, {max:.2}]")
                },
            );
        }
        if let Some((min, max)) = self.periphyton_biomass_g_bounds {
            check_f64_bounds(
                &mut violations,
                "periphyton biomass",
                snap.periphyton_biomass_g,
                (min, max),
                |value, min, max| {
                    format!("periphyton biomass {value:.4}g outside [{min:.3}, {max:.3}]")
                },
            );
        }
        if let Some((min, max)) = self.kh_d_bounds {
            check_f64_bounds(
                &mut violations,
                "KH",
                snap.kh_d,
                (min, max),
                |value, min, max| format!("KH {value:.2} dKH outside [{min:.1}, {max:.1}]"),
            );
        }
        if let Some((min, max)) = self.dic_mg_c_per_l_bounds {
            check_f64_bounds(
                &mut violations,
                "DIC",
                snap.dissolved_inorganic_carbon_mg_c_per_l,
                (min, max),
                |value, min, max| format!("DIC {value:.2} mg C/L outside [{min:.1}, {max:.1}]"),
            );
        }

        violations
    }
}

// ---------------------------------------------------------------------------
// Assertion failure record
// ---------------------------------------------------------------------------

/// A single assertion failure with context about when and where it occurred.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssertionFailure {
    pub checkpoint_label: String,
    pub day: u32,
    pub hour: u8,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Artifact metadata
// ---------------------------------------------------------------------------

/// Metadata written to `metadata.json` in the artifact directory.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArtifactMetadata {
    pub seed: u64,
    pub scenario_id: String,
    pub artifact_label: String,
    pub simulated_day: u32,
    pub simulated_hour: u8,
    pub checkpoint_count: usize,
    pub assertion_failures: Vec<AssertionFailure>,
}

// ---------------------------------------------------------------------------
// HarnessError
// ---------------------------------------------------------------------------

/// Error type for harness operations.
#[derive(Debug)]
pub enum HarnessError {
    Sim(tank_core::SimError),
    Setup(String),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarnessError::Sim(e) => write!(f, "simulation error: {e}"),
            HarnessError::Setup(msg) => write!(f, "harness setup: {msg}"),
        }
    }
}

impl std::error::Error for HarnessError {}

impl From<tank_core::SimError> for HarnessError {
    fn from(e: tank_core::SimError) -> Self {
        HarnessError::Sim(e)
    }
}

impl From<tank_data::PresetError> for HarnessError {
    fn from(e: tank_data::PresetError) -> Self {
        HarnessError::Setup(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// HarnessRun
// ---------------------------------------------------------------------------

/// A deterministic scenario run with checkpoint capture and failure artifact packaging.
///
/// Orchestrates seeded scenario execution, optional budget/tracing instrumentation,
/// checkpoint recording, envelope assertions, and structured artifact output on failure.
pub struct HarnessRun {
    seed: SimSeed,
    scenario_id: String,
    artifact_label: Option<String>,
    engine: Engine,
    checkpoints: Vec<Checkpoint>,
    failures: Vec<AssertionFailure>,
    verbose: bool,
}

impl HarnessRun {
    /// Create a new harness run with default scenario settings.
    ///
    /// Reads `TANK_E2E_VERBOSE` to determine verbose trace mode.
    pub fn new(seed: SimSeed, scenario_id: &str) -> Result<Self, HarnessError> {
        let state = tank_scenarios::seeded_state(seed, scenario_id)?;
        let verbose = is_verbose();
        Ok(Self {
            seed,
            scenario_id: scenario_id.to_owned(),
            artifact_label: None,
            engine: Engine::from_parts(state, vec![]),
            checkpoints: Vec::new(),
            failures: Vec::new(),
            verbose,
        })
    }

    /// Create a new harness run with full startup overrides.
    pub fn with_overrides(
        seed: SimSeed,
        scenario_id: &str,
        overrides: StartupOverrides,
    ) -> Result<Self, HarnessError> {
        let state = tank_scenarios::seeded_state_with_full_overrides(seed, scenario_id, overrides)?;
        let verbose = is_verbose();
        Ok(Self {
            seed,
            scenario_id: scenario_id.to_owned(),
            artifact_label: None,
            engine: Engine::from_parts(state, vec![]),
            checkpoints: Vec::new(),
            failures: Vec::new(),
            verbose,
        })
    }

    /// Create a new harness run from a caller-prepared state.
    pub fn from_state(seed: SimSeed, scenario_id: &str, state: TankState) -> Self {
        let verbose = is_verbose();
        Self {
            seed,
            scenario_id: scenario_id.to_owned(),
            artifact_label: None,
            engine: Engine::from_parts(state, vec![]),
            checkpoints: Vec::new(),
            failures: Vec::new(),
            verbose,
        }
    }

    /// Override the artifact label used in failure metadata and temp paths.
    ///
    /// This is useful when multiple tests run the same scenario with the same
    /// seed but need distinct failure artifacts.
    pub fn with_artifact_label(mut self, label: impl Into<String>) -> Self {
        self.artifact_label = Some(label.into());
        self
    }

    /// Enable budget tracking and tracing on the engine.
    ///
    /// Uses `Verbosity::Trace` in verbose mode, `Verbosity::Detail` otherwise.
    /// Budget tracking and tracing surfaces come from tank_core's existing
    /// budget ledger and simulation tracer.
    pub fn enable_instrumentation(&mut self) {
        self.engine.enable_budget_tracking();
        let verbosity = if self.verbose {
            Verbosity::Trace
        } else {
            Verbosity::Detail
        };
        self.engine.enable_tracing(SimTracer::new(verbosity));
    }

    /// Step the simulation forward by the given number of hours.
    pub fn step_hours(&mut self, hours: u32) -> Result<(), HarnessError> {
        self.engine.step_hours(hours)?;
        Ok(())
    }

    /// Apply a player action to the engine.
    pub fn apply_action(&mut self, action: PlayerAction) -> Result<(), HarnessError> {
        self.engine.apply_action(action)?;
        Ok(())
    }

    /// Take a snapshot of the current simulation state.
    pub fn snapshot(&self) -> TankSnapshot {
        self.engine.snapshot()
    }

    /// Access the underlying engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Mutable access to the underlying engine.
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// The seed used for this run.
    pub fn seed(&self) -> SimSeed {
        self.seed
    }

    /// The scenario id for this run.
    pub fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    /// Record a named checkpoint at the current simulation time.
    pub fn checkpoint(&mut self, label: &str) -> &Checkpoint {
        let snap = self.engine.snapshot();
        let state = self.engine.full_state();
        let cp = Checkpoint {
            label: label.to_owned(),
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            snapshot: snap,
        };
        self.checkpoints.push(cp);
        self.checkpoints.last().unwrap()
    }

    /// Record a checkpoint and check it against an envelope.
    ///
    /// Violations are accumulated as assertion failures; call [`finish`] to
    /// trigger artifact capture and panic if any failures exist.
    pub fn assert_envelope(&mut self, label: &str, envelope: &Envelope) {
        let snap = self.engine.snapshot();
        let state = self.engine.full_state();
        let day = state.environment.day;
        let hour = state.environment.hour_of_day;

        let violations = envelope.check(&snap);
        for v in &violations {
            self.failures.push(AssertionFailure {
                checkpoint_label: label.to_owned(),
                day,
                hour,
                message: v.clone(),
            });
        }

        self.checkpoints.push(Checkpoint {
            label: label.to_owned(),
            day,
            hour,
            snapshot: snap,
        });
    }

    /// Assert a custom predicate on the current snapshot.
    ///
    /// Return `Ok(())` from the closure if the assertion passes, or
    /// `Err(message)` to record a failure.
    pub fn assert_snapshot(
        &mut self,
        label: &str,
        pred: impl FnOnce(&TankSnapshot) -> Result<(), String>,
    ) {
        let snap = self.engine.snapshot();
        let state = self.engine.full_state();
        if let Err(msg) = pred(&snap) {
            self.failures.push(AssertionFailure {
                checkpoint_label: label.to_owned(),
                day: state.environment.day,
                hour: state.environment.hour_of_day,
                message: msg,
            });
        }
    }

    /// Record a custom failure against the run at the current simulation time.
    ///
    /// Use this for multi-run or stateful assertions that are not naturally
    /// expressed against a single snapshot. [`finish`] will still emit the
    /// standard artifact bundle before returning an error.
    pub fn record_failure(&mut self, label: &str, message: impl Into<String>) {
        let state = self.engine.full_state();
        self.failures.push(AssertionFailure {
            checkpoint_label: label.to_owned(),
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            message: message.into(),
        });
    }

    /// Return the current list of assertion failures.
    pub fn failures(&self) -> &[AssertionFailure] {
        &self.failures
    }

    /// Return the current list of checkpoints.
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Deterministic artifact directory path for this run.
    ///
    /// Path is derived solely from scenario_id and seed, so the same inputs
    /// always produce the same artifact path.
    pub fn artifact_dir(&self) -> PathBuf {
        let scenario_component = sanitize_artifact_component(&self.scenario_id);
        let dir_name = if self.artifact_label() == self.scenario_id {
            format!("{}_seed{}", scenario_component, self.seed.0)
        } else {
            format!(
                "{}_{}_seed{}",
                scenario_component,
                sanitize_artifact_component(self.artifact_label()),
                self.seed.0
            )
        };
        std::env::temp_dir().join("tank_harness").join(dir_name)
    }

    /// Persist the current artifact bundle even when the run has not failed.
    ///
    /// Summary runners can use this to archive checkpoints, budget ledgers,
    /// and `trace.jsonl` for successful executions before calling [`finish`].
    pub fn persist_artifacts(&self) -> PathBuf {
        self.write_artifacts()
    }

    /// Finalize the run.
    ///
    /// - If `TANK_E2E_VERBOSE=1` is set, dumps full trace to stderr even on success.
    /// - If any assertion failures were recorded, writes structured artifacts and
    ///   returns `Err` with a summary including the artifact path.
    /// - Returns `Ok(())` if all assertions passed.
    pub fn finish(self) -> Result<(), String> {
        if self.verbose {
            self.dump_verbose_trace();
        }

        if self.failures.is_empty() {
            return Ok(());
        }

        let artifact_path = self.write_artifacts();
        let mut msg = if self.artifact_label() == self.scenario_id {
            format!(
                "scenario '{}' seed={}: {} assertion failure(s)\n",
                self.scenario_id,
                self.seed.0,
                self.failures.len()
            )
        } else {
            format!(
                "scenario '{}' [{}] seed={}: {} assertion failure(s)\n",
                self.scenario_id,
                self.artifact_label(),
                self.seed.0,
                self.failures.len()
            )
        };
        for f in &self.failures {
            msg.push_str(&format!(
                "  [{} d{}h{}] {}\n",
                f.checkpoint_label, f.day, f.hour, f.message
            ));
        }
        msg.push_str(&format!("artifacts: {}\n", artifact_path.display()));
        Err(msg)
    }

    // -----------------------------------------------------------------------
    // Artifact writing
    // -----------------------------------------------------------------------

    fn write_artifacts(&self) -> PathBuf {
        let dir = self.artifact_dir();
        std::fs::create_dir_all(&dir).expect("failed to create artifact dir");

        let state = self.engine.full_state();
        let meta = ArtifactMetadata {
            seed: self.seed.0,
            scenario_id: self.scenario_id.clone(),
            artifact_label: self.artifact_label().to_owned(),
            simulated_day: state.environment.day,
            simulated_hour: state.environment.hour_of_day,
            checkpoint_count: self.checkpoints.len(),
            assertion_failures: self.failures.clone(),
        };
        write_json(&dir.join("metadata.json"), &meta);
        write_json(&dir.join("checkpoints.json"), &self.checkpoints);

        let mut summary = String::new();
        for f in &self.failures {
            summary.push_str(&format!(
                "[{} d{}h{}] {}\n",
                f.checkpoint_label, f.day, f.hour, f.message
            ));
        }
        let _ = std::fs::write(dir.join("assertion_summary.txt"), &summary);

        // Budget ledger reuses tank_core's BudgetLedger (Serialize)
        if let Some(ledger) = self.engine.budget_ledger() {
            write_json(&dir.join("budget_ledger.json"), ledger);
        }

        // Trace reuses tank_core's TickTrace + JsonLinesSink
        if let Some(tracer) = self.engine.tracer() {
            let mut buf = Vec::new();
            {
                let mut sink = JsonLinesSink::new(&mut buf);
                for tick in tracer.ticks() {
                    let _ = sink.emit_tick(tick);
                }
            }
            let _ = std::fs::write(dir.join("trace.jsonl"), &buf);
        }

        dir
    }

    fn dump_verbose_trace(&self) {
        if let Some(tracer) = self.engine.tracer() {
            eprintln!(
                "--- TANK_E2E_VERBOSE trace for '{}' [{}] seed={} ({} ticks) ---",
                self.scenario_id,
                self.artifact_label(),
                self.seed.0,
                tracer.tick_count()
            );
            let mut sink = JsonLinesSink::new(std::io::stderr());
            for tick in tracer.ticks() {
                let _ = sink.emit_tick(tick);
            }
            eprintln!("--- end trace ---");
        }
    }

    fn artifact_label(&self) -> &str {
        self.artifact_label
            .as_deref()
            .unwrap_or(self.scenario_id.as_str())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_verbose() -> bool {
    std::env::var("TANK_E2E_VERBOSE").is_ok_and(|v| v == "1")
}

fn sanitize_artifact_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "run".to_string()
    } else {
        trimmed.to_string()
    }
}

fn non_finite_label(value: f64) -> &'static str {
    if value.is_nan() {
        "NaN"
    } else if value.is_sign_negative() {
        "-inf"
    } else {
        "inf"
    }
}

fn check_f64_bounds(
    violations: &mut Vec<String>,
    label: &str,
    value: f64,
    bounds: (f64, f64),
    on_out_of_range: impl FnOnce(f64, f64, f64) -> String,
) {
    let (min, max) = bounds;
    if !value.is_finite() {
        violations.push(format!(
            "{label} is {} (expected [{min:.3}, {max:.3}])",
            non_finite_label(value)
        ));
        return;
    }
    if value < min || value > max {
        violations.push(on_out_of_range(value, min, max));
    }
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).expect("failed to serialize artifact");
    std::fs::write(path, json).expect("failed to write artifact");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{Envelope, HarnessRun};
    use tank_core::{SimSeed, TankSnapshot, TankState};

    #[test]
    fn shrimp_envelope_uses_total_population_count() {
        let mut state = TankState::new(SimSeed(8_001));
        state.animal.adult.count = 2;
        state.animal.sub_adult.count = 3;
        state.animal.juvenile.count = 1;
        let snapshot = TankSnapshot::from_state(&state);

        let violations = Envelope::default().shrimp_count(6, 6).check(&snapshot);

        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
    }

    #[test]
    fn reproduction_envelope_checks_snapshot_fields() {
        let mut state = TankState::new(SimSeed(8_002));
        state.animal.juvenile.count = 12;
        state.animal.berried_females_count = 2;
        state.animal.reproductive_readiness_index = 0.6;
        let snapshot = TankSnapshot::from_state(&state);

        let violations = Envelope::default()
            .juveniles_count(10, 15)
            .berried_females_count(1, 3)
            .shrimp_reproductive_readiness(0.5, 0.7)
            .check(&snapshot);

        assert!(
            violations.is_empty(),
            "unexpected violations: {violations:?}"
        );
    }

    #[test]
    fn envelope_reports_non_finite_values() {
        let mut state = TankState::new(SimSeed(8_003));
        state.water.temperature_c = f64::NAN;
        let snapshot = TankSnapshot::from_state(&state);

        let violations = Envelope::default()
            .temperature_c(20.0, 30.0)
            .check(&snapshot);

        assert_eq!(
            violations,
            vec!["temperature is NaN (expected [20.000, 30.000])"]
        );
    }

    #[test]
    fn explicit_artifact_label_changes_artifact_dir() {
        let state = TankState::new(SimSeed(8_004));
        let unlabeled = HarnessRun::from_state(SimSeed(8_004), "medium_planted", state.clone());
        let labeled = HarnessRun::from_state(SimSeed(8_004), "medium_planted", state)
            .with_artifact_label("do-night-cycle");

        assert_ne!(unlabeled.artifact_dir(), labeled.artifact_dir());
        assert!(labeled
            .artifact_dir()
            .ends_with("medium_planted_do_night_cycle_seed8004"));
    }

    #[test]
    fn recorded_failures_surface_in_finish_output() {
        let state = TankState::new(SimSeed(8_005));
        let mut run = HarnessRun::from_state(SimSeed(8_005), "medium_planted", state)
            .with_artifact_label("recorded_failure");
        let artifact_dir = run.artifact_dir();
        let _ = std::fs::remove_dir_all(&artifact_dir);

        run.record_failure("capacity_check", "expected higher nitrifier capacity");

        let err = run
            .finish()
            .expect_err("recorded failure should fail finish");
        assert!(err.contains("capacity_check"));
        assert!(err.contains("expected higher nitrifier capacity"));
        assert!(artifact_dir.join("metadata.json").exists());
        assert!(artifact_dir.join("assertion_summary.txt").exists());

        let _ = std::fs::remove_dir_all(artifact_dir);
    }

    #[test]
    fn successful_runs_can_persist_artifacts_for_summary_reporting() {
        let state = TankState::new(SimSeed(8_006));
        let mut run = HarnessRun::from_state(SimSeed(8_006), "medium_planted", state)
            .with_artifact_label("summary_capture");
        let artifact_dir = run.artifact_dir();
        let _ = std::fs::remove_dir_all(&artifact_dir);

        run.enable_instrumentation();
        run.checkpoint("initial");
        run.step_hours(1).expect("summary capture run should step");
        run.checkpoint("final");

        let persisted_dir = run.persist_artifacts();
        assert_eq!(persisted_dir, artifact_dir);
        assert!(artifact_dir.join("metadata.json").exists());
        assert!(artifact_dir.join("checkpoints.json").exists());
        assert!(artifact_dir.join("budget_ledger.json").exists());
        assert!(artifact_dir.join("trace.jsonl").exists());

        let meta_raw = std::fs::read_to_string(artifact_dir.join("metadata.json"))
            .expect("metadata should be readable");
        let meta: serde_json::Value =
            serde_json::from_str(&meta_raw).expect("metadata should parse as json");
        assert_eq!(
            meta["assertion_failures"]
                .as_array()
                .expect("assertion_failures should be an array")
                .len(),
            0
        );

        run.finish()
            .expect("persisting successful artifacts should not fail finish");

        let _ = std::fs::remove_dir_all(artifact_dir);
    }
}
