//! Structured simulation tracing with per-system deltas and configurable verbosity.
//!
//! Records per tick and per system: pool modifications, deltas, events generated,
//! and optional intermediate notes. Designed as a reusable substrate for scenario
//! probes, CI artifacts, and domain-specific diagnostics.
//!
//! # Verbosity levels
//!
//! - **Off**: no tracing (zero overhead via `Option<SimTracer>`)
//! - **Summary**: per-tick system names and event counts
//! - **Detail**: per-system pool deltas
//! - **Trace**: per-system pool deltas plus intermediate notes
//!
//! # Output modes
//!
//! - **In-memory**: `SimTracer::new()` — for test assertions
//! - **JSON-lines file**: `SimTracer::with_sink()` + [`JsonLinesSink`] — for CI archival
//! - **Stderr**: `SimTracer::with_sink()` + [`StderrSink`] — for interactive debugging

use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::types::TankState;

// ---------------------------------------------------------------------------
// Verbosity
// ---------------------------------------------------------------------------

/// Controls how much detail the tracer captures.
///
/// Levels are ordered: `Off` < `Summary` < `Detail` < `Trace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    /// No tracing. Real "off" is `Option::<SimTracer>::None` on the engine.
    Off,
    /// Per-tick system names and event counts only.
    Summary,
    /// Per-system pool deltas.
    Detail,
    /// Per-system pool deltas plus intermediate notes.
    Trace,
}

// ---------------------------------------------------------------------------
// Trace record types — stable schema for downstream consumers
// ---------------------------------------------------------------------------

/// A single pool's change during one system's execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolDelta {
    pub pool: String,
    pub before: f64,
    pub after: f64,
    pub delta: f64,
}

/// Trace entry for one system's execution within a tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemTraceEntry {
    pub system: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pool_deltas: Vec<PoolDelta>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub events_generated: usize,
}

/// Trace record for one complete simulation tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickTrace {
    pub tick_index: usize,
    pub day: u32,
    pub hour: u8,
    pub systems: Vec<SystemTraceEntry>,
}

impl TickTrace {
    /// Find the trace entry for a specific system by name.
    pub fn system(&self, name: &str) -> Option<&SystemTraceEntry> {
        self.systems.iter().find(|s| s.system == name)
    }
}

impl SystemTraceEntry {
    /// Find the delta for a specific pool by name.
    pub fn pool_delta(&self, pool: &str) -> Option<&PoolDelta> {
        self.pool_deltas.iter().find(|d| d.pool == pool)
    }

    /// Get the delta value for a pool, or 0.0 if the pool wasn't modified.
    pub fn delta_for(&self, pool: &str) -> f64 {
        self.pool_delta(pool).map(|d| d.delta).unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// Output sinks
// ---------------------------------------------------------------------------

/// Trait for external trace output destinations.
pub trait TraceSink: Send {
    /// Write one tick's trace record.
    fn emit_tick(&mut self, tick: &TickTrace);
}

/// JSON-lines output sink. Each tick becomes one line of JSON.
pub struct JsonLinesSink<W: Write> {
    writer: W,
}

impl<W: Write> JsonLinesSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write + Send> TraceSink for JsonLinesSink<W> {
    fn emit_tick(&mut self, tick: &TickTrace) {
        if let Ok(line) = serde_json::to_string(tick) {
            let _ = writeln!(self.writer, "{line}");
        }
    }
}

/// Stderr output sink for interactive debugging.
pub struct StderrSink;

impl TraceSink for StderrSink {
    fn emit_tick(&mut self, tick: &TickTrace) {
        if let Ok(line) = serde_json::to_string(tick) {
            eprintln!("{line}");
        }
    }
}

// ---------------------------------------------------------------------------
// Pool snapshot — internal helper for computing deltas
// ---------------------------------------------------------------------------

/// Snapshot of key simulation pool values for computing deltas.
pub(crate) struct PoolSnapshot {
    pools: Vec<(&'static str, f64)>,
    event_count: usize,
}

impl PoolSnapshot {
    /// Capture current values of all tracked pools from state.
    pub(crate) fn capture(state: &TankState) -> Self {
        let plant_biomass_g: f64 = state.plant_guilds.iter().map(|p| p.biomass_g).sum();
        let substrate_n_mg: f64 = state
            .substrate_layers
            .iter()
            .map(|l| l.nutrient_store_mg_n_total)
            .sum();

        Self {
            pools: vec![
                // Water chemistry
                ("water.ammonia_mg_n", state.water.ammonia_total_mg_n_total),
                ("water.nitrite_mg_n", state.water.nitrite_mg_n_total),
                ("water.nitrate_mg_n", state.water.nitrate_mg_n_total),
                (
                    "water.don_mg_n",
                    state.water.dissolved_organic_nitrogen_mg_n_total,
                ),
                (
                    "water.dic_mg_c",
                    state.water.dissolved_inorganic_carbon_mg_c_total,
                ),
                (
                    "water.doc_mg_c",
                    state.water.dissolved_organic_carbon_mg_c_total,
                ),
                ("water.do_mg", state.water.dissolved_oxygen_mg_total),
                ("water.alkalinity_meq", state.water.alkalinity_meq_total),
                ("water.phosphate_mg_p", state.water.phosphate_mg_p_total),
                ("water.ph", state.water.ph),
                ("water.temperature_c", state.water.temperature_c),
                // Biology
                ("plants.biomass_g", plant_biomass_g),
                ("algae.suspended_g", state.algae.suspended_biomass_g),
                ("algae.periphyton_g", state.algae.periphyton_biomass_g),
                ("microbe.decomposer_g", state.microbe.decomposer_biomass_g),
                (
                    "microbe.aob_g",
                    state.microbe.ammonia_oxidizer_biomass_g,
                ),
                (
                    "microbe.nob_g",
                    state.microbe.nitrite_oxidizer_biomass_g,
                ),
                ("microbe.comammox_g", state.microbe.comammox_biomass_g),
                ("animal.adults", f64::from(state.animal.adults_count)),
                ("animal.juveniles", f64::from(state.animal.juveniles_count)),
                ("animal.condition", state.animal.condition_index),
                ("animal.reserve_g", state.animal.reserve_g),
                (
                    "microfauna.population",
                    state.microfauna.population_index,
                ),
                // Detritus
                (
                    "detritus.particulate_g",
                    state.detritus.particulate_organics_g_total,
                ),
                ("detritus.fine_g", state.detritus.fine_detritus_g_total),
                (
                    "detritus.feed_residue_g",
                    state.detritus.dissolved_feed_residue_g_total,
                ),
                // Substrate & filter
                ("substrate.n_mg", substrate_n_mg),
                (
                    "filter.maturity",
                    state.filter_state.biofilter_maturity_index,
                ),
                ("filter.clogging", state.filter_state.clogging_index),
                (
                    "filter.cleanliness",
                    state.hardware.filter.cleanliness_index,
                ),
            ],
            event_count: state.event_log.len(),
        }
    }

    /// Compute pool deltas between this (before) and `after` snapshot.
    /// Only includes pools that actually changed.
    pub(crate) fn deltas_to(&self, after: &PoolSnapshot) -> Vec<PoolDelta> {
        self.pools
            .iter()
            .zip(after.pools.iter())
            .filter_map(|((name, before_val), (_, after_val))| {
                let delta = after_val - before_val;
                if delta.abs() > f64::EPSILON {
                    Some(PoolDelta {
                        pool: (*name).to_owned(),
                        before: *before_val,
                        after: *after_val,
                        delta,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Count events added between this (before) and `after` snapshot.
    pub(crate) fn events_since(&self, after: &PoolSnapshot) -> usize {
        after.event_count.saturating_sub(self.event_count)
    }
}

// ---------------------------------------------------------------------------
// Per-tick trace builder — used during engine tick execution
// ---------------------------------------------------------------------------

/// Working state for building a trace record during a single tick.
pub(crate) struct TickTraceBuilder {
    pub(crate) verbosity: Verbosity,
    tick_index: usize,
    day: u32,
    hour: u8,
    pub(crate) entries: Vec<SystemTraceEntry>,
}

impl TickTraceBuilder {
    /// Convert to a finalized trace record.
    pub(crate) fn into_tick_trace(self) -> TickTrace {
        TickTrace {
            tick_index: self.tick_index,
            day: self.day,
            hour: self.hour,
            systems: self.entries,
        }
    }
}

// ---------------------------------------------------------------------------
// SimTracer — the main tracer stored on Engine
// ---------------------------------------------------------------------------

/// Structured simulation tracer.
///
/// Records per-tick, per-system trace entries with configurable verbosity.
/// Stored as `Option<SimTracer>` on the engine for zero overhead when off.
pub struct SimTracer {
    verbosity: Verbosity,
    ticks: Vec<TickTrace>,
    sink: Option<Box<dyn TraceSink>>,
}

impl std::fmt::Debug for SimTracer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimTracer")
            .field("verbosity", &self.verbosity)
            .field("tick_count", &self.ticks.len())
            .finish()
    }
}

/// Create a tracer that records in-memory only.
impl SimTracer {
    pub fn new(verbosity: Verbosity) -> Self {
        Self {
            verbosity,
            ticks: Vec::new(),
            sink: None,
        }
    }

    /// Create a tracer that records in-memory and writes to an external sink.
    pub fn with_sink(verbosity: Verbosity, sink: Box<dyn TraceSink>) -> Self {
        Self {
            verbosity,
            ticks: Vec::new(),
            sink: Some(sink),
        }
    }

    /// Current verbosity level.
    pub fn verbosity(&self) -> Verbosity {
        self.verbosity
    }

    /// Access all recorded tick traces.
    pub fn ticks(&self) -> &[TickTrace] {
        &self.ticks
    }

    /// Number of ticks recorded so far.
    pub fn tick_count(&self) -> usize {
        self.ticks.len()
    }

    /// Drain all recorded ticks, freeing memory.
    pub fn drain_ticks(&mut self) -> Vec<TickTrace> {
        std::mem::take(&mut self.ticks)
    }

    /// Start building a trace for a new tick.
    pub(crate) fn begin_tick(&self, state: &TankState) -> TickTraceBuilder {
        TickTraceBuilder {
            verbosity: self.verbosity,
            tick_index: self.ticks.len(),
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            entries: Vec::new(),
        }
    }

    /// Finalize a tick's trace record: write to sink (if any) and store.
    pub(crate) fn finish_tick(&mut self, builder: TickTraceBuilder) {
        let tick = builder.into_tick_trace();
        if let Some(sink) = self.sink.as_mut() {
            sink.emit_tick(&tick);
        }
        self.ticks.push(tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_ordering() {
        assert!(Verbosity::Off < Verbosity::Summary);
        assert!(Verbosity::Summary < Verbosity::Detail);
        assert!(Verbosity::Detail < Verbosity::Trace);
    }

    #[test]
    fn pool_delta_only_includes_changes() {
        let before = PoolSnapshot {
            pools: vec![("a", 1.0), ("b", 2.0), ("c", 3.0)],
            event_count: 0,
        };
        let after = PoolSnapshot {
            pools: vec![("a", 1.0), ("b", 5.0), ("c", 3.0)],
            event_count: 1,
        };

        let deltas = before.deltas_to(&after);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].pool, "b");
        assert!((deltas[0].delta - 3.0).abs() < f64::EPSILON);
        assert_eq!(before.events_since(&after), 1);
    }

    #[test]
    fn tick_trace_json_roundtrip() {
        let tick = TickTrace {
            tick_index: 0,
            day: 1,
            hour: 12,
            systems: vec![SystemTraceEntry {
                system: "system:temperature".to_owned(),
                pool_deltas: vec![PoolDelta {
                    pool: "water.temperature_c".to_owned(),
                    before: 24.0,
                    after: 24.5,
                    delta: 0.5,
                }],
                notes: vec![],
                events_generated: 0,
            }],
        };

        let json = serde_json::to_string(&tick).expect("serialize");
        let parsed: TickTrace = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tick, parsed);
    }

    #[test]
    fn json_lines_sink_produces_valid_output() {
        let mut buf = Vec::new();
        {
            let mut sink = JsonLinesSink::new(&mut buf);
            sink.emit_tick(&TickTrace {
                tick_index: 0,
                day: 0,
                hour: 0,
                systems: vec![],
            });
            sink.emit_tick(&TickTrace {
                tick_index: 1,
                day: 0,
                hour: 1,
                systems: vec![],
            });
        }
        let output = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let _: TickTrace = serde_json::from_str(line).expect("valid JSON per line");
        }
    }

    #[test]
    fn system_trace_entry_delta_helpers() {
        let entry = SystemTraceEntry {
            system: "test".to_owned(),
            pool_deltas: vec![PoolDelta {
                pool: "water.ph".to_owned(),
                before: 7.0,
                after: 6.8,
                delta: -0.2,
            }],
            notes: vec![],
            events_generated: 0,
        };
        assert!((entry.delta_for("water.ph") - (-0.2)).abs() < f64::EPSILON);
        assert!((entry.delta_for("nonexistent") - 0.0).abs() < f64::EPSILON);
    }
}
