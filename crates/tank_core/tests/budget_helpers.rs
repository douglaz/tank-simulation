//! Tests for the budget_helpers module: step_and_inspect, assertion API, debug output,
//! and per-system inspection.

use std::panic::catch_unwind;

use tank_core::{
    budget_helpers::{
        assert_budget_balanced, assert_c_conserved, assert_n_conserved, assert_o2_balanced,
        assert_per_tick_balanced, step_and_inspect, with_budget_debug_env, BudgetInspector,
        Element,
    },
    BudgetDelta, BudgetEntry, BudgetLedger, BudgetRecordingKind, BudgetTotals, ElementBudget,
    Engine, PlantGuildState, PlayerAction, ProcessParams, SimError, SimSeed, SimulationEngine,
    SourceWaterProfile, SubstrateLayerState, TankState, TickBudgetRecord,
};

// ---------------------------------------------------------------------------
// State fixtures (reused from budget_tracking.rs patterns)
// ---------------------------------------------------------------------------

fn close_budget_gas_exchange(state: &mut TankState) {
    // Keep the filter enabled so nitrification fixtures still behave like
    // filtered tanks, but zero all gas-transfer terms that would otherwise
    // make the carbon helper observe atmospheric exchange.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
}

fn active_budget_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    close_budget_gas_exchange(&mut state);
    state.water.ammonia_total_mg_n_total = 6.5;
    state.water.nitrite_mg_n_total = 1.5;
    state.water.nitrate_mg_n_total = 14.0;
    state.water.phosphate_mg_p_total = 3.5;
    state.water.dissolved_inorganic_carbon_mg_c_total = 300.0;
    state.water.dissolved_organic_carbon_mg_c_total = 32.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.5;
    state.detritus.particulate_organics_g_total = 0.7;
    state.detritus.fine_detritus_g_total = 0.45;
    state.detritus.dissolved_feed_residue_g_total = 0.2;
    state.algae.suspended_biomass_g = 0.35;
    state.algae.periphyton_biomass_g = 0.55;
    state.microbe.decomposer_biomass_g = 0.12;
    state.microbe.ammonia_oxidizer_biomass_g = 0.08;
    state.microbe.nitrite_oxidizer_biomass_g = 0.07;
    state.microbe.comammox_biomass_g = 0.03;
    state.microfauna.population_index = 0.4;
    state.microfauna.grazing_pressure_index = 0.35;
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.substrate_layers[0].nutrient_store_mg_n_total = 22.0;
    state.substrate_layers[0].nutrient_store_mg_p_total = 6.0;
    state.reseed_stability_tracker();
    state
}

fn quiescent_budget_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    close_budget_gas_exchange(&mut state);
    state.hardware.light.enabled = false;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.animal.adult.count = 0;
    state.animal.juvenile.count = 0;
    state.animal.berried_females_count = 0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;
    state.substrate_layers = vec![SubstrateLayerState {
        nutrient_store_mg_n_total: 18.0,
        nutrient_store_mg_p_total: 4.0,
        ..SubstrateLayerState::default()
    }];
    state.water.ammonia_total_mg_n_total = 3.0;
    state.water.nitrite_mg_n_total = 1.25;
    state.water.nitrate_mg_n_total = 9.5;
    state.water.dissolved_organic_nitrogen_mg_n_total = 2.75;
    state.water.dissolved_inorganic_carbon_mg_c_total = 210.0;
    state.water.dissolved_organic_carbon_mg_c_total = 16.0;
    state.reseed_stability_tracker();
    state
}

// ---------------------------------------------------------------------------
// Retrofit: closed-system conservation using new helpers
// (mirrors test_closed_system_n_conservation / test_closed_system_c_conservation)
// ---------------------------------------------------------------------------

#[test]
fn closed_system_n_and_c_conserved_via_helpers() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_001));
    let mut engine = Engine::from_parts(state, vec![]);

    let result = step_and_inspect(&mut engine, 24)?;

    // Single-call conservation checks replace multi-line manual assertions
    assert_n_conserved(&result.budget, 1e-6);
    assert_c_conserved(&result.budget, 1e-6);

    // Per-tick granularity: every individual tick stays balanced
    assert_per_tick_balanced(&result.budget, Element::Nitrogen, 1e-6);
    assert_per_tick_balanced(&result.budget, Element::Carbon, 1e-6);

    // Verify we got the expected number of ticks
    assert_eq!(result.budget.ticks().len(), 24);

    Ok(())
}

// ---------------------------------------------------------------------------
// Generic per-element assertion API
// ---------------------------------------------------------------------------

#[test]
fn assert_budget_balanced_covers_all_three_elements() -> Result<(), SimError> {
    let state = quiescent_budget_state(SimSeed(9_050));
    let mut engine = Engine::from_parts(state, vec![]);
    let result = step_and_inspect(&mut engine, 1)?;

    assert_budget_balanced(&result.budget, Element::Nitrogen, 1e-6);
    assert_budget_balanced(&result.budget, Element::Carbon, 1e-6);
    assert_budget_balanced(&result.budget, Element::Oxygen, 1e-6);

    Ok(())
}

// ---------------------------------------------------------------------------
// System-level delta inspection
// ---------------------------------------------------------------------------

#[test]
fn inspect_system_deltas_for_nitrogen() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_051));
    let mut engine = Engine::from_parts(state, vec![]);
    let result = step_and_inspect(&mut engine, 24)?;

    let deltas = result.budget.system_deltas(Element::Nitrogen);

    // The nitrogen_cycle system should appear in the deltas
    let n_cycle = deltas
        .iter()
        .find(|d| d.label == "system:nitrogen_cycle")
        .expect("nitrogen_cycle system should produce N deltas over 24 hours");
    // Nitrogen cycle moves nitrogen between pools (in and out)
    assert!(
        n_cycle.in_mg > 0.0 || n_cycle.out_mg > 0.0,
        "nitrogen_cycle should have non-zero in or out: {n_cycle:?}"
    );

    Ok(())
}

#[test]
fn inspect_specific_system_in_specific_tick() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_052));
    let mut engine = Engine::from_parts(state, vec![]);
    let result = step_and_inspect(&mut engine, 1)?;

    // Look up nitrogen_cycle in tick 0
    let delta = result
        .budget
        .system_delta_in_tick(0, "system:nitrogen_cycle", Element::Nitrogen)
        .expect("nitrogen_cycle should be in tick 0");
    // The net should be zero for a closed system within tolerance
    assert!(
        delta.net_mg.abs() <= 1e-6,
        "nitrogen_cycle net should be near-zero in closed system: {delta:?}"
    );

    Ok(())
}

#[test]
fn largest_mover_identifies_dominant_system() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_053));
    let mut engine = Engine::from_parts(state, vec![]);
    let result = step_and_inspect(&mut engine, 24)?;

    let mover = result.budget.largest_mover(Element::Nitrogen);
    // In an active state, some system should be the largest nitrogen mover
    assert!(mover.is_some(), "should identify a largest nitrogen mover");
    let mover = mover.unwrap();
    assert!(
        !mover.label.is_empty(),
        "largest mover should have a non-empty label"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Water change (open-system) inspection
// ---------------------------------------------------------------------------

#[test]
fn water_change_shows_n_export_via_system_deltas() -> Result<(), SimError> {
    let mut state = quiescent_budget_state(SimSeed(9_054));
    state.substrate_layers[0].nutrient_store_mg_n_total = 0.0;
    state
        .source_water_catalog
        .insert("ro_like".to_string(), SourceWaterProfile::zero());
    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 25.0,
        source_profile_id: "ro_like".to_string(),
    })?;
    let result = step_and_inspect(&mut engine, 1)?;

    // Water change should show up as a nitrogen exporter
    let deltas = result.budget.system_deltas(Element::Nitrogen);
    let wc = deltas
        .iter()
        .find(|d| d.label == "action:water_change")
        .expect("water_change action should appear in system deltas");
    assert!(
        wc.out_mg > 0.0,
        "water change with RO water should export nitrogen: {wc:?}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Trim plants: closed-system action inspection
// ---------------------------------------------------------------------------

#[test]
fn trim_plants_is_closed_system_via_helpers() -> Result<(), SimError> {
    let mut state = quiescent_budget_state(SimSeed(9_055));
    state.plant_guilds = vec![PlantGuildState::default(), PlantGuildState::default()];
    state.plant_guilds[0].biomass_g = 2.0;
    state.plant_guilds[1].biomass_g = 3.5;
    let mut engine = Engine::from_parts(state, vec![]);

    engine.apply_action(PlayerAction::TrimPlantsAndRemove { fraction: 0.25 })?;
    let result = step_and_inspect(&mut engine, 1)?;

    // Trim plants is closed-system: N and C should be conserved
    assert_n_conserved(&result.budget, 1e-6);
    assert_c_conserved(&result.budget, 1e-6);

    // Verify the action shows gross in/out (redistribution, not net change)
    let trim_delta = result
        .budget
        .system_delta_in_tick(0, "action:trim_plants", Element::Nitrogen)
        .expect("trim_plants should appear in budget");
    assert!(
        trim_delta.in_mg > 0.0 && trim_delta.out_mg > 0.0,
        "trim_plants should redistribute nitrogen: {trim_delta:?}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// step_and_inspect returns valid snapshot
// ---------------------------------------------------------------------------

#[test]
fn step_and_inspect_returns_snapshot() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_056));
    let mut engine = Engine::from_parts(state, vec![]);

    let result = step_and_inspect(&mut engine, 1)?;

    assert!(result.snapshot.ph.is_finite());
    assert!(result.snapshot.water_temp_c.is_finite());
    assert!(result.snapshot.tan_mg_n_per_l.is_finite());

    Ok(())
}

// ---------------------------------------------------------------------------
// Multiple step_and_inspect calls accumulate correctly
// ---------------------------------------------------------------------------

#[test]
fn multiple_inspections_are_independent() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_057));
    let mut engine = Engine::from_parts(state, vec![]);

    let r1 = step_and_inspect(&mut engine, 12)?;
    let r2 = step_and_inspect(&mut engine, 12)?;

    assert_eq!(r1.budget.ticks().len(), 12);
    assert_eq!(r2.budget.ticks().len(), 12);

    // Both periods should individually conserve N
    assert_n_conserved(&r1.budget, 1e-6);
    assert_n_conserved(&r2.budget, 1e-6);

    Ok(())
}

// ---------------------------------------------------------------------------
// O2 demand balance (aerated system)
// ---------------------------------------------------------------------------

#[test]
fn o2_balance_check_with_aeration() -> Result<(), SimError> {
    let mut state = active_budget_state(SimSeed(9_058));
    let defaults = ProcessParams::default();
    state.environment.hour_of_day = 12;
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.hardware.filter.flow_lph = 200.0;
    state.process_params.reaeration_kla_base = defaults.reaeration_kla_base;
    state.process_params.aeration_kla_boost = defaults.aeration_kla_boost;
    state.water.dissolved_oxygen_mg_total = 40.0;
    state.reseed_stability_tracker();
    let mut engine = Engine::from_parts(state, vec![]);

    let result = step_and_inspect(&mut engine, 1)?;

    assert_o2_balanced(&result.budget, 1e-6);

    let do_delta =
        result
            .budget
            .system_delta_in_tick(0, "system:dissolved_oxygen", Element::Oxygen);
    assert!(
        do_delta.is_some(),
        "dissolved_oxygen system should appear in budget"
    );
    let do_delta = do_delta.unwrap();
    assert!(
        do_delta.in_mg > 0.0 && do_delta.out_mg > 0.0,
        "dissolved_oxygen should preserve gross O2 bookkeeping in this fixture: {do_delta:?}"
    );
    let do_entry = result.budget.ticks()[0]
        .entries
        .iter()
        .find(|entry| entry.label == "system:dissolved_oxygen")
        .expect("dissolved_oxygen entry should be present in tick ledger");
    assert_eq!(
        do_entry.recording_kind,
        BudgetRecordingKind::Explicit,
        "dissolved_oxygen should not fall back to snapshot-only oxygen accounting"
    );

    Ok(())
}

#[test]
fn o2_balance_check_rejects_snapshot_only_bidirectional_stage_accounting() {
    let budget = BudgetInspector {
        ledger: BudgetLedger {
            ticks: vec![TickBudgetRecord {
                tick_index: 0,
                day: 0,
                hour: 12,
                before: BudgetTotals {
                    oxygen_mg: 40.0,
                    ..BudgetTotals::default()
                },
                after: BudgetTotals {
                    oxygen_mg: 45.0,
                    ..BudgetTotals::default()
                },
                net_delta: BudgetDelta {
                    oxygen: ElementBudget {
                        in_mg: 5.0,
                        out_mg: 0.0,
                    },
                    ..BudgetDelta::default()
                },
                entries: vec![BudgetEntry {
                    label: "system:dissolved_oxygen".to_string(),
                    delta: BudgetDelta {
                        oxygen: ElementBudget {
                            in_mg: 5.0,
                            out_mg: 0.0,
                        },
                        ..BudgetDelta::default()
                    },
                    recording_kind: BudgetRecordingKind::Snapshot,
                }],
            }],
        },
        before_totals: BudgetTotals {
            oxygen_mg: 40.0,
            ..BudgetTotals::default()
        },
        after_totals: BudgetTotals {
            oxygen_mg: 45.0,
            ..BudgetTotals::default()
        },
    };

    let panic = catch_unwind(|| assert_o2_balanced(&budget, 1e-6))
        .expect_err("snapshot-only dissolved_oxygen accounting should be rejected");
    let message = if let Some(message) = panic.downcast_ref::<String>() {
        message.as_str()
    } else if let Some(message) = panic.downcast_ref::<&str>() {
        message
    } else {
        panic!("unexpected panic payload type");
    };
    assert!(
        message.contains("missing explicit gross O2 accounting"),
        "panic should explain the explicit O2 bookkeeping requirement: {message}"
    );
}

// ---------------------------------------------------------------------------
// Debug output (TANK_BUDGET_DEBUG) smoke test
// ---------------------------------------------------------------------------

#[test]
fn debug_output_env_path_does_not_panic() -> Result<(), SimError> {
    with_budget_debug_env(true, || -> Result<(), SimError> {
        let state = quiescent_budget_state(SimSeed(9_059));
        let mut engine = Engine::from_parts(state, vec![]);

        let result = step_and_inspect(&mut engine, 1)?;
        assert_eq!(result.budget.ticks().len(), 1);

        Ok(())
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// net_delta accessor
// ---------------------------------------------------------------------------

#[test]
fn net_delta_accessor_matches_before_after() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_060));
    let mut engine = Engine::from_parts(state, vec![]);
    let result = step_and_inspect(&mut engine, 24)?;

    let n_delta = result.budget.net_delta(Element::Nitrogen);
    let expected = result.budget.after_totals.nitrogen_mg - result.budget.before_totals.nitrogen_mg;
    assert!(
        (n_delta - expected).abs() < 1e-12,
        "net_delta should match before/after difference"
    );

    Ok(())
}
