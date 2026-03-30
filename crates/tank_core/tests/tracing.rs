//! Integration tests for the simulation tracing facility.

use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tank_core::{
    Engine, JsonLinesSink, PlayerAction, SimSeed, SimTracer, SimulationEngine, SystemTraceEntry,
    TankState, TickTrace, TraceSink, TraceSinkError, Verbosity,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn active_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.water.ammonia_total_mg_n_total = 6.5;
    state.water.nitrite_mg_n_total = 1.5;
    state.water.nitrate_mg_n_total = 14.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 300.0;
    state.water.dissolved_organic_carbon_mg_c_total = 32.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.5;
    state.detritus.particulate_organics_g_total = 0.7;
    state.detritus.fine_detritus_g_total = 0.45;
    state.algae.suspended_biomass_g = 0.35;
    state.algae.set_periphyton_total(0.55);
    state.microbe.set_decomposer_total(0.12);
    state.microbe.ammonia_oxidizer_biomass_g = 0.08;
    state.microbe.nitrite_oxidizer_biomass_g = 0.07;
    state.microbe.comammox_biomass_g = 0.03;
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.reseed_stability_tracker();
    state
}

fn shrimp_stress_trace_state(seed: SimSeed) -> TankState {
    let mut state = active_state(seed);
    let volume_l = state.water_volume_l();
    state.water.nitrite_mg_n_total = 3.0 * volume_l;
    state.water.chloride_mg_total = 20.0 * volume_l;
    state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
    state.animal.adult.count = 12;
    state.animal.adult.condition_index = 0.8;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.reseed_stability_tracker();
    state
}

struct CountingSink {
    emitted_ticks: Arc<AtomicUsize>,
}

impl TraceSink for CountingSink {
    fn emit_tick(&mut self, _tick: &TickTrace) -> Result<(), TraceSinkError> {
        self.emitted_ticks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("expected tracing sink failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Core tracing tests
// ---------------------------------------------------------------------------

#[test]
fn tracing_off_by_default_no_tracer() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::new(SimSeed(1));
    engine.step_hours(5)?;
    assert!(engine.tracer().is_none());
    Ok(())
}

#[test]
fn tracing_records_one_tick_per_hour_at_detail() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(100)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(3)?;

    let tracer = engine.tracer().expect("tracer enabled");
    assert_eq!(tracer.tick_count(), 3, "one tick per hour");
    Ok(())
}

#[test]
fn detail_verbosity_captures_per_system_pool_deltas() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(200)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];

    // Should have multiple system entries
    assert!(
        tick.systems.len() >= 4,
        "expected multiple systems, got {}",
        tick.systems.len()
    );

    // Find the nitrogen cycle system - it should modify ammonia/nitrite/nitrate pools
    let nitrogen = tick.system("system:nitrogen_cycle");
    assert!(
        nitrogen.is_some(),
        "nitrogen_cycle system should appear in trace"
    );

    // At least some system should have pool deltas (the state has active chemistry)
    let total_deltas: usize = tick.systems.iter().map(|s| s.pool_deltas.len()).sum();
    assert!(
        total_deltas > 0,
        "detail verbosity should capture pool deltas for active state"
    );

    Ok(())
}

#[test]
fn summary_verbosity_records_systems_without_pool_deltas() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(300)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Summary));
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];

    // Systems should be recorded
    assert!(
        !tick.systems.is_empty(),
        "summary should record system names"
    );

    // But no pool deltas at Summary level
    for system in &tick.systems {
        assert!(
            system.pool_deltas.is_empty(),
            "summary verbosity should not capture pool deltas for {}",
            system.system
        );
        assert!(
            system.notes.is_empty(),
            "summary verbosity should not capture notes for {}",
            system.system
        );
    }

    Ok(())
}

#[test]
fn trace_verbosity_records_intermediate_notes_and_post_stage_events(
) -> Result<(), tank_core::SimError> {
    let mut state = active_state(SimSeed(350));
    state.environment.hour_of_day = 23;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Trace));
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];
    let biofilter = tick
        .system("system:daily_biofilter_maturity")
        .expect("daily biofilter maturity stage should be traced");

    assert_eq!(
        biofilter.events_generated, 2,
        "daily biofilter maturity should attribute both maturity events to the traced stage"
    );
    assert!(
        biofilter
            .notes
            .iter()
            .any(|note| note.starts_with("biofilter_maturity.delta=")),
        "trace verbosity should expose deterministic intermediate notes"
    );
    assert!(
        biofilter
            .notes
            .iter()
            .any(|note| note == "biofilter_maturity.emitted.biofilm_maturity_increase=true"),
        "trace notes should record whether the biofilm maturity event fired"
    );
    assert!(
        biofilter
            .notes
            .iter()
            .any(|note| note == "biofilter_maturity.emitted.cycle_progressing=true"),
        "trace notes should record whether the cycle-progressing event fired"
    );

    Ok(())
}

#[test]
fn trace_verbosity_reports_chloride_adjusted_nitrite_stress_diagnostics(
) -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(shrimp_stress_trace_state(SimSeed(360)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Trace));
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];
    let shrimp_stress = tick
        .system("system:shrimp_stress")
        .expect("shrimp stress stage should be traced");

    for prefix in [
        "shrimp_stress.nitrite_mg_n_per_l=",
        "shrimp_stress.chloride_mg_per_l=",
        "shrimp_stress.effective_nitrite_hazard_mg_n_per_l=",
        "shrimp_stress.nitrite_stress_increment=",
        "shrimp_stress.nitrite_stress_accum.after=",
    ] {
        assert!(
            shrimp_stress
                .notes
                .iter()
                .any(|note| note.starts_with(prefix)),
            "trace notes should include {prefix:?}. Notes: {:?}",
            shrimp_stress.notes
        );
    }

    Ok(())
}

#[test]
fn trace_verbosity_reports_tick_snapshot_for_shrimp_probe_debugging(
) -> Result<(), tank_core::SimError> {
    let mut state = shrimp_stress_trace_state(SimSeed(365));
    state.environment.hour_of_day = 23;
    state.animal.sub_adult.count = 4;
    state.animal.juvenile.count = 6;
    state.animal.adult.reserve_g = 1.2;
    state.animal.sub_adult.reserve_g = 0.4;
    state.animal.juvenile.reserve_g = 0.2;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Trace));
    engine.apply_action(PlayerAction::Feed { grams: 0.25 })?;
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];
    let snapshot = tick
        .system("system:tick_snapshot")
        .expect("tick snapshot stage should be traced");

    for prefix in [
        "tick_snapshot.shrimp.adult.count=",
        "tick_snapshot.shrimp.adult.reserve_g=",
        "tick_snapshot.shrimp.sub_adult.reserve_g=",
        "tick_snapshot.shrimp.juvenile.reserve_g=",
        "tick_snapshot.shrimp.failed_molt_accum=",
        "tick_snapshot.water.gh_d=",
        "tick_snapshot.water.ph=",
        "tick_snapshot.water.nitrite_mg_n_per_l=",
        "tick_snapshot.water.chloride_mg_per_l=",
    ] {
        assert!(
            snapshot.notes.iter().any(|note| note.starts_with(prefix)),
            "tick snapshot notes should include {prefix:?}. Notes: {:?}",
            snapshot.notes
        );
    }

    Ok(())
}

#[test]
fn feed_action_shows_detritus_increase_in_trace() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(400)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.apply_action(PlayerAction::Feed { grams: 1.0 })?;
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];

    // Find the feed action trace
    let feed = tick
        .system("action:feed")
        .expect("feed action should appear in trace");

    // Feed adds to particulate detritus
    let detritus_delta = feed.delta_for("detritus.particulate_g");
    assert!(
        detritus_delta > 0.0,
        "feeding should increase particulate detritus, got delta={detritus_delta}"
    );

    Ok(())
}

#[test]
fn inspect_per_system_deltas_for_specific_pool() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(500)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));

    // Run multiple ticks
    engine.step_hours(5)?;

    let tracer = engine.tracer().unwrap();
    assert_eq!(tracer.tick_count(), 5);

    // For each tick, we can inspect the dissolved_oxygen system's effect on water.do_mg
    for tick in tracer.ticks() {
        if let Some(do_system) = tick.system("system:dissolved_oxygen") {
            // The DO system should modify water.do_mg (reaeration, respiration, etc.)
            // It may or may not have a delta depending on equilibrium, but the system
            // entry should exist
            let _ = do_system.delta_for("water.do_mg");
        }
    }

    Ok(())
}

#[test]
fn daily_systems_appear_on_day_boundary_ticks() -> Result<(), tank_core::SimError> {
    let mut state = active_state(SimSeed(600));
    // Set hour to 23 so the first tick triggers the daily pipeline
    state.environment.hour_of_day = 23;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let tick = &tracer.ticks()[0];

    // Daily systems should appear in this tick's trace
    let daily_systems: Vec<&str> = tick
        .systems
        .iter()
        .filter(|s| s.system.starts_with("system:daily_"))
        .map(|s| s.system.as_str())
        .collect();

    assert!(
        daily_systems.contains(&"system:daily_plants"),
        "daily_plants should run on day boundary"
    );
    assert!(
        daily_systems.contains(&"system:daily_algae"),
        "daily_algae should run on day boundary"
    );
    assert!(
        daily_systems.contains(&"system:daily_shrimp"),
        "daily_shrimp should run on day boundary"
    );

    Ok(())
}

#[test]
fn detail_tracing_surfaces_daily_shrimp_internal_state_deltas() -> Result<(), tank_core::SimError> {
    let mut state = active_state(SimSeed(650));
    state.environment.hour_of_day = 23;
    state.animal.adult.count = 12;
    state.animal.adult.condition_index = 0.55;
    state.animal.reproductive_readiness_index = 0.2;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(1)?;

    let tick = &engine.tracer().unwrap().ticks()[0];
    let daily_shrimp = tick
        .system("system:daily_shrimp")
        .expect("daily shrimp stage should be traced");

    assert!(SystemTraceEntry::tracks_pool("animal.molt_stress"));
    assert!(
        daily_shrimp
            .pool_deltas
            .iter()
            .any(|delta| delta.pool == "animal.molt_stress"),
        "daily shrimp tracing should surface internal shrimp-state deltas explicitly"
    );

    Ok(())
}

#[test]
fn detail_tracing_surfaces_failed_molt_accumulation_state() -> Result<(), tank_core::SimError> {
    let mut state = active_state(SimSeed(651));
    state.environment.hour_of_day = 23;
    state.animal.adult.count = 12;
    state.animal.adult.condition_index = 0.95;
    state.animal.adult.reserve_g = 5.0;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days - 1.0;
    let volume_l = state.water_volume_l();
    state.water.calcium_mg_total = volume_l;
    state.water.magnesium_mg_total = 0.1 * volume_l;
    state.reseed_stability_tracker();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(1)?;

    let tick = &engine.tracer().unwrap().ticks()[0];
    let daily_shrimp = tick
        .system("system:daily_shrimp")
        .expect("daily shrimp stage should be traced");

    assert!(SystemTraceEntry::tracks_pool("animal.molt_readiness"));
    assert!(SystemTraceEntry::tracks_pool("animal.failed_molt_accum"));

    let molt_readiness = daily_shrimp
        .pool_delta("animal.molt_readiness")
        .expect("daily shrimp tracing should surface molt_readiness");
    assert!(
        molt_readiness.after >= 1.0 - 1e-9,
        "ready-to-molt stage should drive traced molt_readiness to 1.0, got {}",
        molt_readiness.after
    );

    let failed_molt_accum = daily_shrimp
        .pool_delta("animal.failed_molt_accum")
        .expect("daily shrimp tracing should surface failed_molt_accum");
    assert!(
        failed_molt_accum.delta > 0.0,
        "failed molt should increase traced failed_molt_accum, got delta={}",
        failed_molt_accum.delta
    );

    Ok(())
}

#[test]
fn detail_tracing_surfaces_stability_tracker_baseline_deltas() -> Result<(), tank_core::SimError> {
    let mut state = active_state(SimSeed(660));
    state.environment.hour_of_day = 23;
    state.stability_tracker.prev_ph = state.water.ph + 0.35;
    state.stability_tracker.prev_temp_c = state.water.temperature_c - 1.0;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(1)?;

    let tick = &engine.tracer().unwrap().ticks()[0];
    let stability = tick
        .system("system:stability_tracker")
        .expect("stability tracker stage should be traced");

    assert!(SystemTraceEntry::tracks_pool("stability.prev_ph"));
    assert!(
        stability
            .pool_deltas
            .iter()
            .any(|delta| delta.pool == "stability.prev_ph"),
        "stability tracker tracing should surface baseline updates explicitly"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// JSON-lines output
// ---------------------------------------------------------------------------

#[test]
fn json_lines_output_produces_valid_parseable_records() -> Result<(), tank_core::SimError> {
    // Run engine with in-memory tracing, then serialize to JSON-lines via sink.
    let mut engine = Engine::from_parts(active_state(SimSeed(700)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(3)?;

    let tracer = engine.tracer().unwrap();
    let mut buf = Vec::new();
    {
        let mut sink = JsonLinesSink::new(&mut buf);
        for tick in tracer.ticks() {
            sink.emit_tick(tick)
                .expect("json lines sink should serialize tick");
        }
    }

    let output = String::from_utf8(buf).expect("utf8");
    let lines: Vec<&str> = output.trim().split('\n').collect();
    assert_eq!(lines.len(), 3, "one JSON line per tick");

    for (i, line) in lines.iter().enumerate() {
        let tick: TickTrace =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {i} invalid JSON: {e}"));
        assert_eq!(tick.tick_index, i, "tick_index should match line index");
    }

    Ok(())
}

#[test]
fn json_lines_output_is_jq_compatible() -> Result<(), tank_core::SimError> {
    // Run engine with in-memory tracing, then verify JSON schema via sink output.
    let mut engine = Engine::from_parts(active_state(SimSeed(800)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.apply_action(PlayerAction::Feed { grams: 0.5 })?;
    engine.step_hours(1)?;

    let tracer = engine.tracer().unwrap();
    let mut buf = Vec::new();
    {
        let mut sink = JsonLinesSink::new(&mut buf);
        sink.emit_tick(&tracer.ticks()[0])
            .expect("json lines sink should serialize tick");
    }

    let output = String::from_utf8(buf).expect("utf8");
    let tick: TickTrace =
        serde_json::from_str(output.trim()).expect("single-line JSON should parse");

    // Verify the structure has all expected fields
    assert!(tick.tick_index == 0);
    assert!(!tick.systems.is_empty());
    // Verify PoolDelta has the expected fields when serialized
    let json_val: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    let systems = json_val["systems"].as_array().unwrap();
    for system in systems {
        if let Some(deltas) = system["pool_deltas"].as_array() {
            for delta in deltas {
                assert!(delta["pool"].is_string());
                assert!(delta["before"].is_number());
                assert!(delta["after"].is_number());
                assert!(delta["delta"].is_number());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Zero overhead when off
// ---------------------------------------------------------------------------

#[test]
fn tracing_does_not_alter_simulation_results() -> Result<(), tank_core::SimError> {
    let state = active_state(SimSeed(900));

    // Run without tracing
    let mut engine_no_trace = Engine::from_parts(state.clone(), vec![]);
    engine_no_trace.apply_action(PlayerAction::Feed { grams: 0.5 })?;
    engine_no_trace.step_hours(24)?;
    let snapshot_no_trace = engine_no_trace.snapshot();

    // Run with tracing at Detail
    let mut engine_traced = Engine::from_parts(state, vec![]);
    engine_traced.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine_traced.apply_action(PlayerAction::Feed { grams: 0.5 })?;
    engine_traced.step_hours(24)?;
    let snapshot_traced = engine_traced.snapshot();

    // Results should be identical
    assert_eq!(
        snapshot_no_trace, snapshot_traced,
        "tracing should not alter simulation results"
    );

    Ok(())
}

#[test]
fn verbosity_off_records_no_ticks_and_emits_no_sink_output() -> Result<(), tank_core::SimError> {
    let emitted_ticks = Arc::new(AtomicUsize::new(0));
    let sink = CountingSink {
        emitted_ticks: Arc::clone(&emitted_ticks),
    };
    let mut engine = Engine::from_parts(active_state(SimSeed(950)), vec![]);
    engine.enable_tracing(SimTracer::with_sink(Verbosity::Off, Box::new(sink)));
    engine.step_hours(3)?;

    let tracer = engine.tracer().expect("tracer should stay attached");
    assert_eq!(
        tracer.tick_count(),
        0,
        "off verbosity should not retain ticks"
    );
    assert!(
        tracer.ticks().is_empty(),
        "off verbosity should leave the in-memory trace buffer empty"
    );
    assert_eq!(
        emitted_ticks.load(Ordering::SeqCst),
        0,
        "off verbosity should not emit sink output"
    );

    Ok(())
}

#[test]
fn tracing_compatible_with_budget_tracking() -> Result<(), tank_core::SimError> {
    let state = active_state(SimSeed(1000));

    // Run with both budget and tracing enabled
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(3)?;

    let budget = engine.budget_ledger().expect("budget should be active");
    let tracer = engine.tracer().expect("tracer should be active");

    assert_eq!(budget.ticks.len(), 3);
    assert_eq!(tracer.tick_count(), 3);

    Ok(())
}

#[test]
fn tracer_records_sink_failures_without_losing_ticks() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(1050)), vec![]);
    engine.enable_tracing(SimTracer::with_sink(
        Verbosity::Detail,
        Box::new(JsonLinesSink::new(FailingWriter)),
    ));
    engine.step_hours(1)?;

    let tracer = engine.tracer().expect("tracer should stay attached");
    assert_eq!(tracer.tick_count(), 1);
    assert_eq!(tracer.sink_failure_count(), 1);
    assert_eq!(tracer.sink_failures()[0].tick_index, 0);
    assert!(
        tracer.sink_failures()[0]
            .error
            .contains("trace write failed"),
        "sink failures should be retained for callers to inspect"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Drain and disable
// ---------------------------------------------------------------------------

#[test]
fn drain_ticks_frees_memory() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(1100)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Summary));
    engine.step_hours(5)?;

    let tracer = engine.tracer_mut().unwrap();
    assert_eq!(tracer.tick_count(), 5);

    let drained = tracer.drain_ticks();
    assert_eq!(drained.len(), 5);
    assert_eq!(tracer.tick_count(), 0);

    Ok(())
}

#[test]
fn drain_ticks_preserves_monotonic_tick_indices() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(1150)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Summary));
    engine.step_hours(5)?;

    {
        let tracer = engine.tracer_mut().unwrap();
        let drained = tracer.drain_ticks();
        assert_eq!(drained.len(), 5);
        assert_eq!(drained[0].tick_index, 0);
        assert_eq!(drained[4].tick_index, 4);
        assert_eq!(tracer.total_tick_count(), 5);
    }

    engine.step_hours(2)?;

    let tracer = engine.tracer().unwrap();
    assert_eq!(tracer.total_tick_count(), 7);
    assert_eq!(tracer.tick_count(), 2);
    assert_eq!(tracer.ticks()[0].tick_index, 5);
    assert_eq!(tracer.ticks()[1].tick_index, 6);

    Ok(())
}

#[test]
fn disable_tracing_returns_tracer_with_data() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(1200)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(2)?;

    let tracer = engine.disable_tracing().expect("should return tracer");
    assert_eq!(tracer.tick_count(), 2);
    assert!(engine.tracer().is_none());

    Ok(())
}

#[test]
fn cloned_engine_requires_tracing_to_be_reenabled() {
    let mut engine = Engine::from_parts(active_state(SimSeed(1250)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));

    let cloned = engine.clone();
    assert!(cloned.tracer().is_none());
}
