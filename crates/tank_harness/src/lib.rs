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

use std::fmt;
use std::path::PathBuf;

use tank_core::{
    Engine, JsonLinesSink, PlayerAction, SimSeed, SimTracer, SimulationEngine, TankSnapshot,
    TraceSink, Verbosity,
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
    pub tan_mg_l_bounds: Option<(f64, f64)>,
    pub nitrite_mg_l_bounds: Option<(f64, f64)>,
    pub nitrate_mg_l_bounds: Option<(f64, f64)>,
    pub do_mg_l_bounds: Option<(f64, f64)>,
    pub shrimp_count_bounds: Option<(u32, u32)>,
    pub plant_biomass_g_bounds: Option<(f64, f64)>,
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

    pub fn tan_mg_l(mut self, min: f64, max: f64) -> Self {
        self.tan_mg_l_bounds = Some((min, max));
        self
    }

    pub fn nitrite_mg_l(mut self, min: f64, max: f64) -> Self {
        self.nitrite_mg_l_bounds = Some((min, max));
        self
    }

    pub fn nitrate_mg_l(mut self, min: f64, max: f64) -> Self {
        self.nitrate_mg_l_bounds = Some((min, max));
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

    pub fn plant_biomass_g(mut self, min: f64, max: f64) -> Self {
        self.plant_biomass_g_bounds = Some((min, max));
        self
    }

    /// Check a snapshot against this envelope, returning all violations.
    pub fn check(&self, snap: &TankSnapshot) -> Vec<String> {
        let mut violations = Vec::new();

        if let Some((min, max)) = self.ph_bounds {
            if snap.ph < min || snap.ph > max {
                violations.push(format!("pH {:.3} outside [{:.1}, {:.1}]", snap.ph, min, max));
            }
        }
        if let Some((min, max)) = self.temperature_c_bounds {
            if snap.water_temp_c < min || snap.water_temp_c > max {
                violations.push(format!(
                    "temp {:.2}C outside [{:.1}, {:.1}]",
                    snap.water_temp_c, min, max
                ));
            }
        }
        if let Some((min, max)) = self.tan_mg_l_bounds {
            if snap.tan_mg_l < min || snap.tan_mg_l > max {
                violations.push(format!(
                    "TAN {:.4} mg/L outside [{:.3}, {:.3}]",
                    snap.tan_mg_l, min, max
                ));
            }
        }
        if let Some((min, max)) = self.nitrite_mg_l_bounds {
            if snap.nitrite_mg_l < min || snap.nitrite_mg_l > max {
                violations.push(format!(
                    "NO2 {:.4} mg/L outside [{:.3}, {:.3}]",
                    snap.nitrite_mg_l, min, max
                ));
            }
        }
        if let Some((min, max)) = self.nitrate_mg_l_bounds {
            if snap.nitrate_mg_l < min || snap.nitrate_mg_l > max {
                violations.push(format!(
                    "NO3 {:.4} mg/L outside [{:.3}, {:.3}]",
                    snap.nitrate_mg_l, min, max
                ));
            }
        }
        if let Some((min, max)) = self.do_mg_l_bounds {
            if snap.do_mg_l < min || snap.do_mg_l > max {
                violations.push(format!(
                    "DO {:.3} mg/L outside [{:.1}, {:.1}]",
                    snap.do_mg_l, min, max
                ));
            }
        }
        if let Some((min, max)) = self.shrimp_count_bounds {
            let total = snap.adult_shrimp_count + snap.juveniles_count;
            if total < min || total > max {
                violations.push(format!("shrimp count {} outside [{}, {}]", total, min, max));
            }
        }
        if let Some((min, max)) = self.plant_biomass_g_bounds {
            if snap.total_plant_biomass_g < min || snap.total_plant_biomass_g > max {
                violations.push(format!(
                    "plant biomass {:.3}g outside [{:.2}, {:.2}]",
                    snap.total_plant_biomass_g, min, max
                ));
            }
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
        let state =
            tank_scenarios::seeded_state_with_full_overrides(seed, scenario_id, overrides)?;
        let verbose = is_verbose();
        Ok(Self {
            seed,
            scenario_id: scenario_id.to_owned(),
            engine: Engine::from_parts(state, vec![]),
            checkpoints: Vec::new(),
            failures: Vec::new(),
            verbose,
        })
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
        std::env::temp_dir()
            .join("tank_harness")
            .join(format!("{}_seed{}", self.scenario_id, self.seed.0))
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
        let mut msg = format!(
            "scenario '{}' seed={}: {} assertion failure(s)\n",
            self.scenario_id,
            self.seed.0,
            self.failures.len()
        );
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
                "--- TANK_E2E_VERBOSE trace for '{}' seed={} ({} ticks) ---",
                self.scenario_id,
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
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_verbose() -> bool {
    std::env::var("TANK_E2E_VERBOSE").is_ok_and(|v| v == "1")
}

fn write_json(path: &std::path::Path, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).expect("failed to serialize artifact");
    std::fs::write(path, json).expect("failed to write artifact");
}
