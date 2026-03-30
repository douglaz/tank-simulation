//! Structured simulation tracing with per-system deltas and configurable verbosity.
//!
//! Records per tick and per system: pool modifications, deltas, events generated,
//! and optional intermediate notes. Designed as a reusable substrate for scenario
//! probes, CI artifacts, and domain-specific diagnostics.
//!
//! # Verbosity levels
//!
//! - **Off**: no tracing (records nothing and emits nothing)
//! - **Summary**: per-tick system names and event counts
//! - **Detail**: per-system pool deltas
//! - **Trace**: per-system pool deltas plus intermediate notes
//!
//! # Output modes
//!
//! - **In-memory**: `SimTracer::new()` — for test assertions
//! - **JSON-lines file**: `SimTracer::with_sink()` + [`JsonLinesSink`] — for CI archival
//! - **Stderr**: `SimTracer::with_sink()` + [`StderrSink`] — for interactive debugging

use std::{error::Error, fmt, io::Write};

use serde::{Deserialize, Serialize};

use crate::types::{PlantGuild, TankState};

// ---------------------------------------------------------------------------
// Verbosity
// ---------------------------------------------------------------------------

/// Controls how much detail the tracer captures.
///
/// Levels are ordered: `Off` < `Summary` < `Detail` < `Trace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verbosity {
    /// No tracing. The engine skips tick builders, in-memory storage, and sink writes.
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

    /// Returns whether `pool` is part of the stable traced-pool schema.
    pub fn tracks_pool(pool: &str) -> bool {
        TRACKED_POOL_NAMES.contains(&pool)
    }

    /// Get the delta value for a tracked pool, or 0.0 if the pool wasn't modified.
    ///
    /// Panics if `pool` is not part of the traced-pool schema. Use
    /// [`SystemTraceEntry::tracks_pool`] or [`SystemTraceEntry::try_delta_for`]
    /// to probe support without panicking.
    pub fn delta_for(&self, pool: &str) -> f64 {
        self.try_delta_for(pool)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Get the delta value for a tracked pool, or an error when the tracer
    /// schema does not record that pool at detail verbosity.
    pub fn try_delta_for(&self, pool: &str) -> Result<f64, UntrackedPoolError> {
        if !Self::tracks_pool(pool) {
            return Err(UntrackedPoolError {
                pool: pool.to_owned(),
            });
        }

        Ok(self.pool_delta(pool).map(|d| d.delta).unwrap_or(0.0))
    }
}

// ---------------------------------------------------------------------------
// Output sinks
// ---------------------------------------------------------------------------

/// Trait for external trace output destinations.
pub trait TraceSink: Send {
    /// Write one tick's trace record.
    fn emit_tick(&mut self, tick: &TickTrace) -> Result<(), TraceSinkError>;
}

/// Error returned by an external trace sink.
#[derive(Debug)]
pub enum TraceSinkError {
    Serialization(serde_json::Error),
    Write(std::io::Error),
}

impl fmt::Display for TraceSinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(err) => write!(f, "trace serialization failed: {err}"),
            Self::Write(err) => write!(f, "trace write failed: {err}"),
        }
    }
}

impl Error for TraceSinkError {}

/// A sink failure captured by [`SimTracer`] while still retaining the in-memory tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSinkFailure {
    pub tick_index: usize,
    pub error: String,
}

/// Error returned when callers ask for a pool outside the stable traced schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntrackedPoolError {
    pub pool: String,
}

impl fmt::Display for UntrackedPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pool '{}' is not tracked by the simulation trace schema",
            self.pool
        )
    }
}

impl Error for UntrackedPoolError {}

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
    fn emit_tick(&mut self, tick: &TickTrace) -> Result<(), TraceSinkError> {
        let line = serde_json::to_string(tick).map_err(TraceSinkError::Serialization)?;
        writeln!(self.writer, "{line}").map_err(TraceSinkError::Write)
    }
}

/// Stderr output sink for interactive debugging.
pub struct StderrSink;

impl TraceSink for StderrSink {
    fn emit_tick(&mut self, tick: &TickTrace) -> Result<(), TraceSinkError> {
        let stderr = std::io::stderr();
        let mut handle = stderr.lock();
        let line = serde_json::to_string(tick).map_err(TraceSinkError::Serialization)?;
        writeln!(&mut handle, "{line}").map_err(TraceSinkError::Write)
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
        let substrate_p_mg: f64 = state
            .substrate_layers
            .iter()
            .map(|l| l.nutrient_store_mg_p_total)
            .sum();
        let fast_stem_biomass_g =
            total_guild_metric(state, PlantGuild::FastStem, |plant| plant.biomass_g);
        let fast_stem_health =
            average_guild_metric(state, PlantGuild::FastStem, |plant| plant.health_index);
        let fast_stem_crowding =
            average_guild_metric(state, PlantGuild::FastStem, |plant| plant.crowding_index);
        let fast_stem_habitat =
            average_guild_metric(state, PlantGuild::FastStem, |plant| plant.habitat_index);
        let rosette_biomass_g =
            total_guild_metric(state, PlantGuild::RootFeedingRosette, |plant| {
                plant.biomass_g
            });
        let rosette_health = average_guild_metric(state, PlantGuild::RootFeedingRosette, |plant| {
            plant.health_index
        });
        let rosette_crowding =
            average_guild_metric(state, PlantGuild::RootFeedingRosette, |plant| {
                plant.crowding_index
            });
        let rosette_habitat =
            average_guild_metric(state, PlantGuild::RootFeedingRosette, |plant| {
                plant.habitat_index
            });
        let egg_count: u32 = state
            .animal
            .egg_cohorts
            .iter()
            .map(|cohort| cohort.count)
            .sum();
        let max_egg_progress_days = state
            .animal
            .egg_cohorts
            .iter()
            .map(|cohort| cohort.progress_days)
            .fold(0.0, f64::max);

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
                ("water.calcium_mg", state.water.calcium_mg_total),
                ("water.magnesium_mg", state.water.magnesium_mg_total),
                ("water.sodium_mg", state.water.sodium_mg_total),
                ("water.potassium_mg", state.water.potassium_mg_total),
                ("water.bicarbonate_mg", state.water.bicarbonate_mg_total),
                ("water.chloride_mg", state.water.chloride_mg_total),
                ("water.sulfate_mg", state.water.sulfate_mg_total),
                ("water.ph", state.water.ph),
                ("water.temperature_c", state.water.temperature_c),
                // Biology
                ("plants.biomass_g", plant_biomass_g),
                ("plants.fast_stem_biomass_g", fast_stem_biomass_g),
                ("plants.fast_stem_health", fast_stem_health),
                ("plants.fast_stem_crowding", fast_stem_crowding),
                ("plants.fast_stem_habitat", fast_stem_habitat),
                ("plants.root_feeding_rosette_biomass_g", rosette_biomass_g),
                ("plants.root_feeding_rosette_health", rosette_health),
                ("plants.root_feeding_rosette_crowding", rosette_crowding),
                ("plants.root_feeding_rosette_habitat", rosette_habitat),
                ("algae.suspended_g", state.algae.suspended_biomass_g),
                ("algae.periphyton_g", state.algae.periphyton_biomass_g),
                ("algae.nuisance_index", state.algae.nuisance_index),
                ("microbe.decomposer_g", state.microbe.decomposer_biomass_g),
                ("microbe.aob_g", state.microbe.ammonia_oxidizer_biomass_g),
                ("microbe.nob_g", state.microbe.nitrite_oxidizer_biomass_g),
                ("microbe.comammox_g", state.microbe.comammox_biomass_g),
                ("microbe.maturity_index", state.microbe.maturity_index),
                ("animal.adults", f64::from(state.animal.adult.count)),
                ("animal.sub_adults", f64::from(state.animal.sub_adult.count)),
                ("animal.juveniles", f64::from(state.animal.juvenile.count)),
                (
                    "animal.berried_females",
                    f64::from(state.animal.berried_females_count),
                ),
                (
                    "animal.condition",
                    state.animal.population_condition_index(),
                ),
                ("animal.molt_stress", state.animal.molt_stress_index),
                ("animal.molt_readiness", state.animal.molt_readiness),
                ("animal.failed_molt_accum", state.animal.failed_molt_accum),
                (
                    "animal.reproductive_readiness",
                    state.animal.reproductive_readiness_index,
                ),
                ("animal.egg_progress_days", state.animal.egg_progress_days),
                (
                    "animal.egg_cohort_count",
                    state.animal.egg_cohorts.len() as f64,
                ),
                ("animal.egg_count", f64::from(egg_count)),
                ("animal.max_egg_cohort_progress_days", max_egg_progress_days),
                (
                    "animal.nh3_stress_accum",
                    state.animal.hourly_nh3_stress_accum,
                ),
                (
                    "animal.nitrite_stress_accum",
                    state.animal.hourly_nitrite_stress_accum,
                ),
                (
                    "animal.low_do_stress_accum",
                    state.animal.hourly_low_do_stress_accum,
                ),
                (
                    "animal.heat_stress_accum",
                    state.animal.hourly_heat_stress_accum,
                ),
                (
                    "animal.instability_stress_accum",
                    state.animal.hourly_instability_stress_accum,
                ),
                (
                    "animal.daily_food_consumed_g",
                    state.animal.daily_food_consumed_g,
                ),
                (
                    "animal.maturation_accum",
                    state.animal.juvenile.maturation_accum,
                ),
                ("animal.reserve_g", state.animal.total_reserve_g()),
                ("microfauna.population", state.microfauna.population_index),
                (
                    "microfauna.grazing_pressure",
                    state.microfauna.grazing_pressure_index,
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
                ("substrate.p_mg", substrate_p_mg),
                (
                    "substrate.o2_penetration_depth_cm",
                    state.substrate_o2_penetration_depth_cm(),
                ),
                (
                    "substrate.root_oxygenation_bonus_cm",
                    crate::systems::substrate::root_oxygenation_bonus_cm(state),
                ),
                (
                    "filter.maturity",
                    state.filter_state.biofilter_maturity_index,
                ),
                ("filter.clogging", state.filter_state.clogging_index),
                (
                    "filter.seeded_biomass",
                    state.filter_state.seeded_biomass_index,
                ),
                (
                    "filter.cleanliness",
                    state.hardware.filter.cleanliness_index,
                ),
                ("heater.last_output_w", state.hardware.heater.last_output_w),
                ("stability.prev_temp_c", state.stability_tracker.prev_temp_c),
                ("stability.prev_ph", state.stability_tracker.prev_ph),
                ("stability.prev_gh_d", state.stability_tracker.prev_gh_d),
                (
                    "stability.prev_do_mg_l",
                    state.stability_tracker.prev_do_mg_l,
                ),
                (
                    "stability.last_temp_swing_c",
                    state.stability_tracker.last_temp_swing_c,
                ),
                (
                    "stability.instability_index",
                    state.stability_tracker.instability_index,
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
                if meaningful_delta(*before_val, *after_val) {
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
    #[allow(dead_code)]
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
    next_tick_index: usize,
    sink: Option<Box<dyn TraceSink>>,
    sink_failures: Vec<TraceSinkFailure>,
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
            next_tick_index: 0,
            sink: None,
            sink_failures: Vec::new(),
        }
    }

    /// Create a tracer that records in-memory and writes to an external sink.
    pub fn with_sink(verbosity: Verbosity, sink: Box<dyn TraceSink>) -> Self {
        Self {
            verbosity,
            ticks: Vec::new(),
            next_tick_index: 0,
            sink: Some(sink),
            sink_failures: Vec::new(),
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

    /// Number of ticks emitted so far, including ticks that have already been drained.
    pub fn total_tick_count(&self) -> usize {
        self.next_tick_index
    }

    /// Sink failures captured while writing external trace output.
    pub fn sink_failures(&self) -> &[TraceSinkFailure] {
        &self.sink_failures
    }

    /// Number of sink failures captured so far.
    pub fn sink_failure_count(&self) -> usize {
        self.sink_failures.len()
    }

    /// Drain all recorded ticks, freeing memory.
    pub fn drain_ticks(&mut self) -> Vec<TickTrace> {
        std::mem::take(&mut self.ticks)
    }

    /// Start building a trace for a new tick.
    pub(crate) fn begin_tick(&self, state: &TankState) -> TickTraceBuilder {
        TickTraceBuilder {
            verbosity: self.verbosity,
            tick_index: self.next_tick_index,
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            entries: Vec::new(),
        }
    }

    /// Finalize a tick's trace record: write to sink (if any) and store.
    pub(crate) fn finish_tick(&mut self, builder: TickTraceBuilder) {
        let tick = builder.into_tick_trace();
        if let Some(sink) = self.sink.as_mut() {
            if let Err(err) = sink.emit_tick(&tick) {
                self.sink_failures.push(TraceSinkFailure {
                    tick_index: tick.tick_index,
                    error: err.to_string(),
                });
            }
        }
        self.next_tick_index = tick.tick_index + 1;
        self.ticks.push(tick);
    }
}

const TRACKED_POOL_NAMES: &[&str] = &[
    "water.ammonia_mg_n",
    "water.nitrite_mg_n",
    "water.nitrate_mg_n",
    "water.don_mg_n",
    "water.dic_mg_c",
    "water.doc_mg_c",
    "water.do_mg",
    "water.alkalinity_meq",
    "water.phosphate_mg_p",
    "water.calcium_mg",
    "water.magnesium_mg",
    "water.sodium_mg",
    "water.potassium_mg",
    "water.bicarbonate_mg",
    "water.chloride_mg",
    "water.sulfate_mg",
    "water.ph",
    "water.temperature_c",
    "plants.biomass_g",
    "plants.fast_stem_biomass_g",
    "plants.fast_stem_health",
    "plants.fast_stem_crowding",
    "plants.fast_stem_habitat",
    "plants.root_feeding_rosette_biomass_g",
    "plants.root_feeding_rosette_health",
    "plants.root_feeding_rosette_crowding",
    "plants.root_feeding_rosette_habitat",
    "algae.suspended_g",
    "algae.periphyton_g",
    "algae.nuisance_index",
    "microbe.decomposer_g",
    "microbe.aob_g",
    "microbe.nob_g",
    "microbe.comammox_g",
    "microbe.maturity_index",
    "animal.adults",
    "animal.juveniles",
    "animal.berried_females",
    "animal.condition",
    "animal.molt_stress",
    "animal.molt_readiness",
    "animal.failed_molt_accum",
    "animal.reproductive_readiness",
    "animal.egg_progress_days",
    "animal.egg_cohort_count",
    "animal.egg_count",
    "animal.max_egg_cohort_progress_days",
    "animal.nh3_stress_accum",
    "animal.nitrite_stress_accum",
    "animal.low_do_stress_accum",
    "animal.heat_stress_accum",
    "animal.instability_stress_accum",
    "animal.daily_food_consumed_g",
    "animal.maturation_accum",
    "animal.reserve_g",
    "microfauna.population",
    "microfauna.grazing_pressure",
    "detritus.particulate_g",
    "detritus.fine_g",
    "detritus.feed_residue_g",
    "substrate.n_mg",
    "substrate.p_mg",
    "substrate.o2_penetration_depth_cm",
    "substrate.root_oxygenation_bonus_cm",
    "filter.maturity",
    "filter.clogging",
    "filter.seeded_biomass",
    "filter.cleanliness",
    "heater.last_output_w",
    "stability.prev_temp_c",
    "stability.prev_ph",
    "stability.prev_gh_d",
    "stability.prev_do_mg_l",
    "stability.last_temp_swing_c",
    "stability.instability_index",
];

const ABSOLUTE_DELTA_THRESHOLD: f64 = 1e-12;
const RELATIVE_DELTA_THRESHOLD: f64 = 1e-12;

fn meaningful_delta(before: f64, after: f64) -> bool {
    let delta = after - before;
    let scale = before.abs().max(after.abs()).max(1.0);
    delta.abs() > ABSOLUTE_DELTA_THRESHOLD.max(scale * RELATIVE_DELTA_THRESHOLD)
}

fn total_guild_metric<F>(state: &TankState, guild: PlantGuild, select: F) -> f64
where
    F: Fn(&crate::types::PlantGuildState) -> f64,
{
    state
        .plant_guilds
        .iter()
        .filter(|plant| plant.guild == guild)
        .map(select)
        .sum()
}

fn average_guild_metric<F>(state: &TankState, guild: PlantGuild, select: F) -> f64
where
    F: Fn(&crate::types::PlantGuildState) -> f64,
{
    let matching: Vec<_> = state
        .plant_guilds
        .iter()
        .filter(|plant| plant.guild == guild)
        .collect();
    if matching.is_empty() {
        0.0
    } else {
        matching.iter().map(|plant| select(plant)).sum::<f64>() / matching.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("expected write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

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
            })
            .expect("sink should serialize first tick");
            sink.emit_tick(&TickTrace {
                tick_index: 1,
                day: 0,
                hour: 1,
                systems: vec![],
            })
            .expect("sink should serialize second tick");
        }
        let output = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let _: TickTrace = serde_json::from_str(line).expect("valid JSON per line");
        }
    }

    #[test]
    fn json_lines_sink_reports_write_failures() {
        let mut sink = JsonLinesSink::new(FailingWriter);
        let err = sink
            .emit_tick(&TickTrace {
                tick_index: 0,
                day: 0,
                hour: 0,
                systems: vec![],
            })
            .expect_err("sink should report write failures");
        assert!(matches!(err, TraceSinkError::Write(_)));
    }

    #[test]
    fn pool_delta_suppresses_rounding_noise() {
        let before = PoolSnapshot {
            pools: vec![("a", 42.0)],
            event_count: 0,
        };
        let after = PoolSnapshot {
            pools: vec![("a", 42.0 + 5e-13)],
            event_count: 0,
        };

        assert!(before.deltas_to(&after).is_empty());
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
        assert!(SystemTraceEntry::tracks_pool("water.ph"));
    }

    #[test]
    fn try_delta_for_rejects_untracked_pools() {
        let entry = SystemTraceEntry {
            system: "test".to_owned(),
            pool_deltas: vec![],
            notes: vec![],
            events_generated: 0,
        };

        let err = entry
            .try_delta_for("nonexistent")
            .expect_err("untracked pools should be rejected explicitly");
        assert_eq!(err.pool, "nonexistent");
    }
}
