//! End-to-end conservation scenario runner.
//!
//! Runs all 6 conservation scenarios in sequence and produces a formatted
//! summary report showing pass/fail status with N and C delta magnitudes.
//! Exits non-zero if any scenario fails.
//!
//! Each scenario replicates the conservation story from the corresponding
//! unit test in `conservation_regression.rs`, wrapped in error handling
//! to ensure all 6 scenarios run even if early ones fail.
//!
//! Run: `cargo test -p tank_harness --test e2e_conservation -- --nocapture`
//! Set `TANK_BUDGET_DEBUG=1` for per-tick budget summaries on stderr.

use tank_core::{
    budget_helpers::{step_and_inspect, Element},
    plant_carbon_mg, plant_nitrogen_mg,
    test_fixtures::{
        dump_trace_to_subdir, feeding_fixture, grazing_fixture, mortality_senescence_fixture,
        trim_fixture, water_change_fixture,
    },
    Engine, PlayerAction, SimTracer, SimulationEngine, Verbosity,
};

const TOL: f64 = 1e-6;
const TRACE_DIR: &str = "tank_conservation_e2e_traces";

// ---------------------------------------------------------------------------
// Outcome type
// ---------------------------------------------------------------------------

struct ScenarioOutcome {
    n_delta_mg: f64,
    c_delta_mg: f64,
}

fn dump_trace(label: &str, engine: &Engine) {
    let _ = dump_trace_to_subdir(label, engine, TRACE_DIR);
}

fn fail_scenario<T>(label: &str, engine: &Engine, message: String) -> Result<T, String> {
    dump_trace(label, engine);
    Err(message)
}

fn ensure<F>(label: &str, engine: &Engine, condition: bool, message: F) -> Result<(), String>
where
    F: FnOnce() -> String,
{
    if condition {
        Ok(())
    } else {
        fail_scenario(label, engine, message())
    }
}

fn ensure_close(
    label: &str,
    engine: &Engine,
    actual: f64,
    expected: f64,
    tolerance: f64,
    context: &str,
) -> Result<(), String> {
    ensure(
        label,
        engine,
        (actual - expected).abs() <= tolerance,
        || format!("{context}: expected {expected}, got {actual} (tolerance {tolerance})"),
    )
}

fn trace_has_pool_delta<F>(engine: &Engine, system: &str, pool: &str, predicate: F) -> bool
where
    F: Fn(f64) -> bool,
{
    let Some(tracer) = engine.tracer() else {
        return false;
    };
    tracer
        .ticks()
        .iter()
        .filter_map(|tick| tick.system(system))
        .any(|entry| predicate(entry.delta_for(pool)))
}

fn ensure_trace_delta<F>(
    label: &str,
    engine: &Engine,
    system: &str,
    pool: &str,
    predicate: F,
    description: &str,
) -> Result<(), String>
where
    F: Fn(f64) -> bool,
{
    ensure(
        label,
        engine,
        trace_has_pool_delta(engine, system, pool, predicate),
        || format!("missing trace evidence for {description}: expected {system} to move {pool}"),
    )
}

// ---------------------------------------------------------------------------
// Scenario runners — each returns Ok(outcome) or Err(message)
// ---------------------------------------------------------------------------

fn run_grazing() -> Result<ScenarioOutcome, String> {
    let label = "grazing";
    let state = grazing_fixture();
    let initial_periphyton = state.algae.periphyton_biomass_g;
    let initial_tan = state.water.ammonia_total_mg_n_total;
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 48).map_err(|e| e.to_string())?;
    let after = engine.full_state();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    ensure(
        label,
        &engine,
        after.algae.periphyton_biomass_g < initial_periphyton,
        || {
            format!(
                "periphyton should decrease from grazing: before={initial_periphyton}, after={}",
                after.algae.periphyton_biomass_g
            )
        },
    )?;
    ensure(
        label,
        &engine,
        after.water.ammonia_total_mg_n_total > initial_tan,
        || {
            format!(
                "TAN should increase from excretion: before={initial_tan}, after={}",
                after.water.ammonia_total_mg_n_total
            )
        },
    )?;
    ensure(
        label,
        &engine,
        after.detritus.fine_detritus_g_total > initial_fine_detritus,
        || {
            format!(
                "fine detritus should increase from feces: before={initial_fine_detritus}, after={}",
                after.detritus.fine_detritus_g_total
            )
        },
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_shrimp",
        "algae.periphyton_g",
        |delta| delta < 0.0,
        "shrimp grazing periphyton",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_shrimp",
        "water.ammonia_mg_n",
        |delta| delta > 0.0,
        "shrimp excretion raising TAN",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_shrimp",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "shrimp feces raising fine detritus",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_feeding() -> Result<ScenarioOutcome, String> {
    let label = "feeding";
    let state = feeding_fixture();
    let initial_particulate = state.detritus.particulate_organics_g_total;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 120).map_err(|e| e.to_string())?;
    let after = engine.full_state();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    ensure(
        label,
        &engine,
        after.detritus.particulate_organics_g_total < initial_particulate,
        || {
            format!(
                "feed should leave particulate organics: before={initial_particulate}, after={}",
                after.detritus.particulate_organics_g_total
            )
        },
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "detritus.particulate_g",
        |delta| delta < 0.0,
        "feed leaching out of particulate organics",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "detritus.feed_residue_g",
        |delta| delta > 0.0,
        "feed dissolution into dissolved residue",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "water.dic_mg_c",
        |delta| delta > 0.0,
        "decomposer mineralization producing DIC",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:nitrogen_cycle",
        "water.nitrate_mg_n",
        |delta| delta > 0.0,
        "nitrification producing nitrate",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_microfauna",
        "algae.periphyton_g",
        |delta| delta < 0.0,
        "microfauna grazing periphyton",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_mortality_senescence() -> Result<ScenarioOutcome, String> {
    let label = "mortality_senescence";
    let state = mortality_senescence_fixture();
    let initial_shrimp = state.animal.adult.count;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    let result = step_and_inspect(&mut engine, 120).map_err(|e| e.to_string())?;
    let after = engine.full_state();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, &engine);
        return Err(format!("conservation: dN={n:+.9} dC={c:+.9}"));
    }
    ensure(
        label,
        &engine,
        after.animal.adult.count < initial_shrimp,
        || {
            format!(
                "some shrimp should have died: before={initial_shrimp}, after={}",
                after.animal.adult.count
            )
        },
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_shrimp",
        "animal.adults",
        |delta| delta < 0.0,
        "shrimp mortality reducing adult shrimp count",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_shrimp",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "shrimp death routing biomass to fine detritus",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_plants",
        "plants.biomass_g",
        |delta| delta < 0.0,
        "plant senescence reducing plant biomass",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_plants",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "plant senescence routing biomass to fine detritus",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_algae",
        "algae.suspended_g",
        |delta| delta < 0.0,
        "algae senescence reducing suspended algae biomass",
    )?;
    ensure_trace_delta(
        label,
        &engine,
        "system:daily_algae",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "algae senescence routing biomass to fine detritus",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_trim_and_remove() -> Result<ScenarioOutcome, String> {
    let label = "trim_and_remove";
    let state = trim_fixture();
    let fraction = 0.25;
    let trimmed_biomass_g: f64 = state
        .plant_guilds
        .iter()
        .map(|p| p.biomass_g * fraction)
        .sum();
    let expected_n_export = plant_nitrogen_mg(trimmed_biomass_g);
    let expected_c_export =
        plant_carbon_mg(trimmed_biomass_g, state.process_params.feed_n_to_c_ratio);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::TrimPlantsAndRemove { fraction })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let actual_export = initial_total_n - final_total_n;
    ensure_close(
        label,
        &engine,
        actual_export,
        expected_n_export,
        TOL,
        "trim-and-remove nitrogen export",
    )?;
    ensure_close(
        label,
        &engine,
        initial_total_c - final_total_c,
        expected_c_export,
        TOL,
        "trim-and-remove carbon export",
    )?;
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_trim_and_leave() -> Result<ScenarioOutcome, String> {
    let label = "trim_and_leave";
    let state = trim_fixture();
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let fraction = 0.25;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::TrimPlantsAndLeaveCuttings { fraction })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    ensure_close(
        label,
        &engine,
        final_total_n,
        initial_total_n,
        TOL,
        "trim-and-leave total nitrogen",
    )?;
    ensure_close(
        label,
        &engine,
        final_total_c,
        initial_total_c,
        TOL,
        "trim-and-leave total carbon",
    )?;
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

fn run_water_change() -> Result<ScenarioOutcome, String> {
    let label = "water_change";
    let state = water_change_fixture();
    let percent = 25.0;
    let fraction = percent / 100.0;
    let vol = state.water_volume_l();
    let exchanged_l = vol * fraction;
    let expected_n_export = fraction
        * (state.water.ammonia_total_mg_n_total
            + state.water.nitrite_mg_n_total
            + state.water.nitrate_mg_n_total
            + state.water.dissolved_organic_nitrogen_mg_n_total);
    let source = state.source_water_catalog.get("test_source").unwrap();
    let expected_n_import = exchanged_l
        * (source.ammonia_mg_n_per_l
            + source.nitrite_mg_n_per_l
            + source.nitrate_mg_n_per_l
            + source.don_mg_n_per_l);
    let expected_c_export = fraction
        * (state.water.dissolved_inorganic_carbon_mg_c_total
            + state.water.dissolved_organic_carbon_mg_c_total);
    let expected_c_import = exchanged_l * (source.dic_mg_c_per_l + source.doc_mg_c_per_l);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
    engine
        .apply_action(PlayerAction::WaterChangePercent {
            percent,
            source_profile_id: "test_source".to_string(),
        })
        .map_err(|e| e.to_string())?;
    let result = step_and_inspect(&mut engine, 1).map_err(|e| e.to_string())?;
    let final_total_n = engine.full_state().total_nitrogen();
    let final_total_c = engine.full_state().total_carbon();
    let actual_change = final_total_n - initial_total_n;
    let expected_change = expected_n_import - expected_n_export;
    ensure_close(
        label,
        &engine,
        actual_change,
        expected_change,
        TOL,
        "water-change total nitrogen delta",
    )?;
    ensure_close(
        label,
        &engine,
        final_total_c - initial_total_c,
        expected_c_import - expected_c_export,
        TOL,
        "water-change total carbon delta",
    )?;
    let n = result.budget.net_delta(Element::Nitrogen);
    let c = result.budget.net_delta(Element::Carbon);
    Ok(ScenarioOutcome {
        n_delta_mg: n,
        c_delta_mg: c,
    })
}

// ---------------------------------------------------------------------------
// E2E runner
// ---------------------------------------------------------------------------

/// Runs all 6 conservation scenarios, produces a summary report, exits
/// non-zero on any failure.
#[test]
fn conservation_e2e_all_scenarios() {
    type ScenarioFn = fn() -> Result<ScenarioOutcome, String>;
    let scenarios: &[(&str, ScenarioFn)] = &[
        ("grazing", run_grazing),
        ("feeding", run_feeding),
        ("mortality_senescence", run_mortality_senescence),
        ("trim_and_remove", run_trim_and_remove),
        ("trim_and_leave", run_trim_and_leave),
        ("water_change", run_water_change),
    ];

    let mut passed = 0usize;
    let mut failed_count = 0usize;
    let mut rows: Vec<(&str, &str, f64, f64, String)> = Vec::new();

    for &(name, run_fn) in scenarios {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_fn)) {
            Ok(Ok(outcome)) => {
                rows.push((
                    name,
                    "PASS",
                    outcome.n_delta_mg,
                    outcome.c_delta_mg,
                    String::new(),
                ));
                passed += 1;
            }
            Ok(Err(msg)) => {
                rows.push((name, "FAIL", f64::NAN, f64::NAN, msg));
                failed_count += 1;
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown panic".into());
                // Truncate long panic messages for the summary
                let short = if msg.len() > 200 {
                    format!("{}...", &msg[..200])
                } else {
                    msg
                };
                rows.push((name, "PANIC", f64::NAN, f64::NAN, short));
                failed_count += 1;
            }
        }
    }

    // Print summary report
    eprintln!();
    eprintln!("{:=<76}", "");
    eprintln!(
        "  Conservation E2E Summary -- {} passed, {} failed",
        passed, failed_count
    );
    eprintln!("{:=<76}", "");
    eprintln!(
        "{:<28} {:>6} {:>16} {:>16}",
        "Scenario", "Result", "dN (mg)", "dC (mg)"
    );
    eprintln!("{:-<76}", "");
    for (name, result, n, c, msg) in &rows {
        if n.is_nan() {
            eprintln!("{:<28} {:>6} {:>16} {:>16}", name, result, "---", "---");
            if !msg.is_empty() {
                // Print first line of error only for readability
                let first_line = msg.lines().next().unwrap_or(msg);
                eprintln!("  {first_line}");
            }
        } else {
            eprintln!("{:<28} {:>6} {:>+16.9} {:>+16.9}", name, result, n, c);
        }
    }
    eprintln!("{:=<76}", "");
    eprintln!();

    assert_eq!(
        failed_count, 0,
        "{failed_count} conservation scenario(s) failed"
    );
}
