//! Conservation regression tests for grazing, feeding, mortality/senescence,
//! trimming, and water change pathways.
//!
//! These 6 tests provide repeatable proof that matter is neither silently
//! created nor destroyed during normal simulation. Closed-loop processes
//! conserve total N and C within < 1e-6 mg per tick. Export actions
//! (trim-and-remove, water change) are tracked as explicit budget entries,
//! not as unexplained mass loss.
//!
//! Tracing is enabled at Detail verbosity. On assertion failure, the full
//! simulation trace is dumped to `/tmp/tank_conservation_traces/<scenario>.jsonl`.
//! Run with `TANK_BUDGET_DEBUG=1` for per-tick budget summaries on stderr.

use tank_core::{
    budget_helpers::{
        assert_c_conserved, assert_n_conserved, assert_per_tick_balanced, step_and_inspect,
        BudgetInspector, Element, InspectionResult,
    },
    plant_carbon_mg, plant_nitrogen_mg,
    test_fixtures::{
        dump_trace_to_subdir, feeding_fixture, grazing_fixture, mortality_senescence_fixture,
        trim_fixture, water_change_fixture,
    },
    Engine, PlayerAction, SimError, SimTracer, SimulationEngine, Verbosity,
};

/// Tolerance for deterministic floating-point conservation: < 1e-6 mg per tick.
const TOL: f64 = 1e-6;
const TRACE_DIR: &str = "tank_conservation_traces";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn panic_with_trace(label: &str, engine: &Engine, message: String) -> ! {
    dump_trace(label, engine);
    panic!("{message}");
}

fn assert_or_dump<F>(label: &str, engine: &Engine, condition: bool, message: F)
where
    F: FnOnce() -> String,
{
    if !condition {
        panic_with_trace(label, engine, message());
    }
}

fn assert_close_or_dump(
    label: &str,
    engine: &Engine,
    actual: f64,
    expected: f64,
    tolerance: f64,
    context: &str,
) {
    if (actual - expected).abs() > tolerance {
        panic_with_trace(
            label,
            engine,
            format!("{context}: expected {expected}, got {actual} (tolerance {tolerance})"),
        );
    }
}

fn step_and_inspect_or_dump(
    label: &str,
    engine: &mut Engine,
    hours: u32,
) -> Result<InspectionResult, SimError> {
    match step_and_inspect(engine, hours) {
        Ok(result) => Ok(result),
        Err(err) => {
            dump_trace(label, engine);
            Err(err)
        }
    }
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

fn assert_trace_delta<F>(
    label: &str,
    engine: &Engine,
    system: &str,
    pool: &str,
    predicate: F,
    description: &str,
) where
    F: Fn(f64) -> bool,
{
    assert_or_dump(
        label,
        engine,
        trace_has_pool_delta(engine, system, pool, predicate),
        || format!("missing trace evidence for {description}: expected {system} to move {pool}"),
    );
}

/// Enable tracing at Detail verbosity on the engine.
fn enable_tracing(engine: &mut Engine) {
    engine.enable_tracing(SimTracer::new(Verbosity::Detail));
}

/// Dump trace to temp file for post-mortem debugging.
fn dump_trace(label: &str, engine: &Engine) {
    if let Some(path) = dump_trace_to_subdir(label, engine, TRACE_DIR) {
        eprintln!("[conservation] trace for '{label}': {}", path.display());
    }
}

/// Assert N and C conservation, dumping trace before panic on failure.
fn assert_conserved(label: &str, budget: &BudgetInspector, engine: &Engine) {
    let n = budget.net_delta(Element::Nitrogen);
    let c = budget.net_delta(Element::Carbon);
    if n.abs() > TOL || c.abs() > TOL {
        dump_trace(label, engine);
    }
    assert_n_conserved(budget, TOL);
    assert_c_conserved(budget, TOL);
}

// ---------------------------------------------------------------------------
// Test 1: Closed-system grazing
// ---------------------------------------------------------------------------

/// Closed-system grazing: shrimp eat periphyton, total N and C conserved.
///
/// When shrimp graze periphyton in a sealed system, the consumer routing
/// contract splits consumed biomass into feces (→ fine detritus), excretion
/// (→ TAN, DOC), respiration (→ DIC, O₂ demand), and growth (→ reserve).
/// Every milligram of N and C removed from periphyton must appear in exactly
/// one of these sinks. No external imports or exports exist, so total N
/// and C must remain flat.
#[test]
fn closed_system_grazing_conserves_n_and_c() -> Result<(), SimError> {
    let state = grazing_fixture();
    let initial_periphyton = state.algae.periphyton_biomass_g;
    let initial_tan = state.water.ammonia_total_mg_n_total;
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;

    let mut engine = Engine::from_parts(state, vec![]);
    enable_tracing(&mut engine);
    let result = step_and_inspect_or_dump("grazing", &mut engine, 48)?;
    let after = engine.full_state();

    // Verify the grazing pathway was exercised.
    assert_or_dump(
        "grazing",
        &engine,
        after.algae.periphyton_biomass_g < initial_periphyton,
        || {
            format!(
                "periphyton should decrease from grazing: before={initial_periphyton}, after={}",
                after.algae.periphyton_biomass_g
            )
        },
    );
    assert_or_dump(
        "grazing",
        &engine,
        after.water.ammonia_total_mg_n_total > initial_tan,
        || {
            format!(
                "TAN should increase from excretion: before={initial_tan}, after={}",
                after.water.ammonia_total_mg_n_total
            )
        },
    );
    assert_or_dump(
        "grazing",
        &engine,
        after.detritus.fine_detritus_g_total > initial_fine_detritus,
        || {
            format!(
                "fine detritus should increase from feces: before={initial_fine_detritus}, after={}",
                after.detritus.fine_detritus_g_total
            )
        },
    );

    // Conservation: N and C flat within tolerance.
    assert_conserved("grazing", &result.budget, &engine);
    assert_per_tick_balanced(&result.budget, Element::Nitrogen, TOL);
    assert_per_tick_balanced(&result.budget, Element::Carbon, TOL);

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 2: Closed-system feeding
// ---------------------------------------------------------------------------

/// Closed-system feeding: feed enters, total N and C conserved through the
/// full decomposition chain.
///
/// A feed pulse pre-loaded as particulate organics passes through:
///   particulate → fine detritus (leaching)
///   → DOC/DON (dissolution)
///   → DIC/TAN (decomposer mineralization)
///   → NO₂/NO₃ (nitrification)
///
/// Shrimp simultaneously graze periphyton and fine detritus. All pathways
/// route mass through explicit elemental bookkeeping. With gas exchange
/// disabled, total N and C must stay flat over 120 hours.
#[test]
fn closed_system_feeding_conserves_n_and_c() -> Result<(), SimError> {
    let state = feeding_fixture();
    let initial_particulate = state.detritus.particulate_organics_g_total;
    let mut engine = Engine::from_parts(state, vec![]);
    enable_tracing(&mut engine);
    let result = step_and_inspect_or_dump("feeding", &mut engine, 120)?;
    let after = engine.full_state();

    assert_or_dump(
        "feeding",
        &engine,
        after.detritus.particulate_organics_g_total < initial_particulate,
        || {
            format!(
                "feed should leave particulate organics: before={initial_particulate}, after={}",
                after.detritus.particulate_organics_g_total
            )
        },
    );
    assert_trace_delta(
        "feeding",
        &engine,
        "system:nitrogen_cycle",
        "detritus.particulate_g",
        |delta| delta < 0.0,
        "feed leaching out of particulate organics",
    );
    assert_trace_delta(
        "feeding",
        &engine,
        "system:nitrogen_cycle",
        "detritus.feed_residue_g",
        |delta| delta > 0.0,
        "feed dissolution into dissolved residue",
    );
    assert_trace_delta(
        "feeding",
        &engine,
        "system:nitrogen_cycle",
        "water.dic_mg_c",
        |delta| delta > 0.0,
        "decomposer mineralization producing DIC",
    );
    assert_trace_delta(
        "feeding",
        &engine,
        "system:nitrogen_cycle",
        "water.nitrate_mg_n",
        |delta| delta > 0.0,
        "nitrification producing nitrate",
    );
    assert_trace_delta(
        "feeding",
        &engine,
        "system:daily_microfauna",
        "algae.periphyton_g",
        |delta| delta < 0.0,
        "microfauna grazing periphyton",
    );

    assert_conserved("feeding", &result.budget, &engine);
    assert_per_tick_balanced(&result.budget, Element::Nitrogen, TOL);
    assert_per_tick_balanced(&result.budget, Element::Carbon, TOL);

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 3: Closed-system mortality / senescence
// ---------------------------------------------------------------------------

/// Closed-system mortality and senescence: shrimp death and plant senescence
/// route biomass to detritus, total N and C stay conserved.
///
/// When shrimp die, their carcass biomass is converted to fine detritus at
/// `death_biomass_to_detritus_fraction = 1.0`. When plants senesce, the
/// lost tissue routes to fine detritus via the plant senescence pathway.
/// Both paths are internal routing — no biomass leaves the system unless
/// a named export action explicitly removes it.
///
/// This test verifies that death-derived and senescence-derived detritus
/// enter the same downstream bookkeeping as feed-derived detritus without
/// double-counting or silent leaks.
#[test]
fn closed_system_mortality_senescence_conserves_n_and_c() -> Result<(), SimError> {
    let state = mortality_senescence_fixture();
    let initial_shrimp = state.animal.adult.count;

    let mut engine = Engine::from_parts(state, vec![]);
    enable_tracing(&mut engine);
    let result = step_and_inspect_or_dump("mortality_senescence", &mut engine, 120)?;
    let after = engine.full_state();

    // Verify that some shrimp actually died.
    assert_or_dump(
        "mortality_senescence",
        &engine,
        after.animal.adult.count < initial_shrimp,
        || {
            format!(
                "some shrimp should have died: before={initial_shrimp}, after={}",
                after.animal.adult.count
            )
        },
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_shrimp",
        "animal.adults",
        |delta| delta < 0.0,
        "shrimp mortality reducing adult shrimp count",
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_shrimp",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "shrimp death routing biomass to fine detritus",
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_plants",
        "plants.biomass_g",
        |delta| delta < 0.0,
        "plant senescence reducing plant biomass",
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_plants",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "plant senescence routing biomass to fine detritus",
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_algae",
        "algae.suspended_g",
        |delta| delta < 0.0,
        "algae senescence reducing suspended algae biomass",
    );
    assert_trace_delta(
        "mortality_senescence",
        &engine,
        "system:daily_algae",
        "detritus.fine_g",
        |delta| delta > 0.0,
        "algae senescence routing biomass to fine detritus",
    );

    // Conservation: all dead biomass routes to detritus → DOC → mineralization.
    assert_conserved("mortality_senescence", &result.budget, &engine);
    assert_per_tick_balanced(&result.budget, Element::Nitrogen, TOL);
    assert_per_tick_balanced(&result.budget, Element::Carbon, TOL);

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 4: Trim-and-remove (open-system export)
// ---------------------------------------------------------------------------

/// Trim-and-remove: trimmed plant biomass is exported from the system.
///
/// When a player trims plants and removes the cuttings, the trimmed biomass
/// exits the simulation entirely. Total N and C should decrease by exactly
/// the exported amount. The budget entry for `action:trim_plants_and_remove`
/// must show a net negative (more N/C leaving than entering). This is an
/// explicit export, not unexplained mass loss.
#[test]
fn trim_and_remove_exports_exact_amount() -> Result<(), SimError> {
    let label = "trim_and_remove";
    let state = trim_fixture();
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let fraction = 0.25;
    let trimmed_biomass_g: f64 = state
        .plant_guilds
        .iter()
        .map(|p| p.biomass_g * fraction)
        .sum();
    let ratio = state.process_params.feed_n_to_c_ratio;
    let expected_n_export = plant_nitrogen_mg(trimmed_biomass_g);
    let expected_c_export = plant_carbon_mg(trimmed_biomass_g, ratio);

    let mut engine = Engine::from_parts(state, vec![]);
    enable_tracing(&mut engine);
    engine.apply_action(PlayerAction::TrimPlantsAndRemove { fraction })?;
    let _result = step_and_inspect_or_dump(label, &mut engine, 1)?;

    // System total N/C should decrease by the exported amount.
    assert_close_or_dump(
        label,
        &engine,
        engine.full_state().total_nitrogen(),
        initial_total_n - expected_n_export,
        TOL,
        "trim-and-remove total nitrogen",
    );
    assert_close_or_dump(
        label,
        &engine,
        engine.full_state().total_carbon(),
        initial_total_c - expected_c_export,
        TOL,
        "trim-and-remove total carbon",
    );

    // Budget entry should show the export as net negative.
    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let trim_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|e| e.label == "action:trim_plants_and_remove")
        .unwrap_or_else(|| {
            panic_with_trace(
                label,
                &engine,
                "trim_plants_and_remove entry should be recorded".to_string(),
            )
        });
    assert_or_dump(
        label,
        &engine,
        trim_entry.delta.nitrogen.net_mg() < 0.0,
        || {
            format!(
                "nitrogen should show net export: net={}",
                trim_entry.delta.nitrogen.net_mg()
            )
        },
    );
    assert_or_dump(
        label,
        &engine,
        trim_entry.delta.carbon.net_mg() < 0.0,
        || {
            format!(
                "carbon should show net export: net={}",
                trim_entry.delta.carbon.net_mg()
            )
        },
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 5: Trim-and-leave (closed-system transfer)
// ---------------------------------------------------------------------------

/// Trim-and-leave: trimmed biomass transfers to detritus, total N/C conserved.
///
/// When a player trims plants and leaves the cuttings in the tank, the trimmed
/// tissue converts to fine detritus at the correct N:C ratio. No mass leaves
/// the system — this is a biomass → detritus internal transfer. Total N and C
/// must remain flat because the cuttings stay in the tank.
#[test]
fn trim_and_leave_conserves_n_and_c() -> Result<(), SimError> {
    let label = "trim_and_leave";
    let state = trim_fixture();
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;
    let fraction = 0.25;

    let mut engine = Engine::from_parts(state, vec![]);
    enable_tracing(&mut engine);
    engine.apply_action(PlayerAction::TrimPlantsAndLeaveCuttings { fraction })?;
    let _result = step_and_inspect_or_dump(label, &mut engine, 1)?;

    // Total N and C must stay flat — cuttings stay in the system.
    assert_close_or_dump(
        label,
        &engine,
        engine.full_state().total_nitrogen(),
        initial_total_n,
        TOL,
        "trim-and-leave total nitrogen",
    );
    assert_close_or_dump(
        label,
        &engine,
        engine.full_state().total_carbon(),
        initial_total_c,
        TOL,
        "trim-and-leave total carbon",
    );

    // Fine detritus should increase from the cuttings.
    assert_or_dump(
        label,
        &engine,
        engine.full_state().detritus.fine_detritus_g_total > initial_fine_detritus,
        || {
            format!(
                "fine detritus should increase from cuttings: before={initial_fine_detritus}, after={}",
                engine.full_state().detritus.fine_detritus_g_total
            )
        },
    );

    // Budget entry should show zero net N/C (internal transfer).
    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let trim_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|e| e.label == "action:trim_plants_and_leave_cuttings")
        .unwrap_or_else(|| {
            panic_with_trace(
                label,
                &engine,
                "trim_plants_and_leave_cuttings entry should be recorded".to_string(),
            )
        });
    assert_close_or_dump(
        label,
        &engine,
        trim_entry.delta.nitrogen.net_mg(),
        0.0,
        TOL,
        "trim-and-leave budget nitrogen net",
    );
    assert_close_or_dump(
        label,
        &engine,
        trim_entry.delta.carbon.net_mg(),
        0.0,
        TOL,
        "trim-and-leave budget carbon net",
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 6: Water change (open-system dilution + replacement)
// ---------------------------------------------------------------------------

/// Water change: dilution + replacement chemistry is mass-balanced.
///
/// A 25% water change with a non-zero source profile simultaneously:
/// - Removes 25% of each dissolved species from the tank (export)
/// - Adds replacement mass from the source at the exchanged volume (import)
///
/// The budget entry for `action:water_change` must track both the nitrogen
/// exported (removed from tank) and imported (from source water). The system's
/// total N change must equal import minus export, matching the budget entry's
/// gross flows to within tolerance.
#[test]
fn water_change_mass_balanced() -> Result<(), SimError> {
    let label = "water_change";
    let state = water_change_fixture();
    let percent = 25.0;
    let fraction = percent / 100.0;
    let vol = state.water_volume_l();
    let exchanged_l = vol * fraction;

    // Expected N export: fraction of dissolved N in tank
    let expected_n_export = fraction
        * (state.water.ammonia_total_mg_n_total
            + state.water.nitrite_mg_n_total
            + state.water.nitrate_mg_n_total
            + state.water.dissolved_organic_nitrogen_mg_n_total);

    // Expected N import: source concentrations × exchanged volume
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
    enable_tracing(&mut engine);
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent,
        source_profile_id: "test_source".to_string(),
    })?;
    let _result = step_and_inspect_or_dump(label, &mut engine, 1)?;

    // Budget entry should track gross N and C import and export.
    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let wc_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|e| e.label == "action:water_change")
        .unwrap_or_else(|| {
            panic_with_trace(
                label,
                &engine,
                "water change entry should be recorded".to_string(),
            )
        });
    assert_close_or_dump(
        label,
        &engine,
        wc_entry.delta.nitrogen.out_mg,
        expected_n_export,
        TOL,
        "water-change nitrogen export",
    );
    assert_close_or_dump(
        label,
        &engine,
        wc_entry.delta.nitrogen.in_mg,
        expected_n_import,
        TOL,
        "water-change nitrogen import",
    );
    assert_close_or_dump(
        label,
        &engine,
        wc_entry.delta.carbon.out_mg,
        expected_c_export,
        TOL,
        "water-change carbon export",
    );
    assert_close_or_dump(
        label,
        &engine,
        wc_entry.delta.carbon.in_mg,
        expected_c_import,
        TOL,
        "water-change carbon import",
    );

    // System total N change should match import − export.
    let final_total_n = engine.full_state().total_nitrogen();
    let actual_change = final_total_n - initial_total_n;
    let expected_change = expected_n_import - expected_n_export;
    assert_close_or_dump(
        label,
        &engine,
        actual_change,
        expected_change,
        TOL,
        "water-change total nitrogen delta",
    );

    // System total C change should match import − export.
    let final_total_c = engine.full_state().total_carbon();
    let actual_c_change = final_total_c - initial_total_c;
    let expected_c_change = expected_c_import - expected_c_export;
    assert_close_or_dump(
        label,
        &engine,
        actual_c_change,
        expected_c_change,
        TOL,
        "water-change total carbon delta",
    );

    Ok(())
}
