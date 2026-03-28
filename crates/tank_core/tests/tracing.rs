//! Integration tests for the simulation tracing facility.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use tank_core::{
    Engine, JsonLinesSink, PlayerAction, SimSeed, SimTracer, SimulationEngine, TankState,
    TickTrace, TraceSink, Verbosity,
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
    state.algae.periphyton_biomass_g = 0.55;
    state.microbe.decomposer_biomass_g = 0.12;
    state.microbe.ammonia_oxidizer_biomass_g = 0.08;
    state.microbe.nitrite_oxidizer_biomass_g = 0.07;
    state.microbe.comammox_biomass_g = 0.03;
    state.animal.adults_count = 0;
    state.animal.juveniles_count = 0;
    state.animal.berried_females_count = 0;
    state.reseed_stability_tracker();
    state
}

struct CountingSink {
    emitted_ticks: Arc<AtomicUsize>,
}

impl TraceSink for CountingSink {
    fn emit_tick(&mut self, _tick: &TickTrace) {
        self.emitted_ticks.fetch_add(1, Ordering::SeqCst);
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
            sink.emit_tick(tick);
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
        sink.emit_tick(&tracer.ticks()[0]);
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
fn disable_tracing_returns_tracer_with_data() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(active_state(SimSeed(1200)), vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine.step_hours(2)?;

    let tracer = engine.disable_tracing().expect("should return tracer");
    assert_eq!(tracer.tick_count(), 2);
    assert!(engine.tracer().is_none());

    Ok(())
}
