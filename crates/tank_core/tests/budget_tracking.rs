use tank_core::{
    algae_carbon_mg, algae_nitrogen_mg, detritus_carbon_mg, detritus_nitrogen_mg,
    live_biomass_carbon_mg, live_biomass_nitrogen_mg, plant_carbon_mg, plant_nitrogen_mg,
    shrimp_biomass_g, Engine, PlayerAction, SimError, SimSeed, SimulationEngine,
    SourceWaterProfile, SubstrateKind, SubstrateLayerState, TankState,
};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

fn known_budget_state() -> TankState {
    let mut state = TankState::new(SimSeed(4_242));
    state.process_params.feed_n_to_c_ratio = 0.16;
    state.water.ammonia_total_mg_n_total = 11.0;
    state.water.nitrite_mg_n_total = 7.0;
    state.water.nitrate_mg_n_total = 19.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 13.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 240.0;
    state.water.dissolved_organic_carbon_mg_c_total = 42.0;
    state.substrate_layers = vec![
        SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            nutrient_store_mg_n_total: 31.0,
            nutrient_store_mg_p_total: 2.0,
            ..SubstrateLayerState::default()
        },
        SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            nutrient_store_mg_n_total: 9.0,
            nutrient_store_mg_p_total: 1.0,
            ..SubstrateLayerState::default()
        },
    ];
    state.plant_guilds[0].biomass_g = 2.5;
    state.plant_guilds[1].biomass_g = 3.25;
    state.algae.suspended_biomass_g = 0.8;
    state.algae.periphyton_biomass_g = 1.4;
    state.microbe.decomposer_biomass_g = 0.6;
    state.microbe.ammonia_oxidizer_biomass_g = 0.2;
    state.microbe.nitrite_oxidizer_biomass_g = 0.15;
    state.microbe.comammox_biomass_g = 0.05;
    state.animal.adults_count = 10;
    state.animal.juveniles_count = 14;
    state.detritus.particulate_organics_g_total = 1.8;
    state.detritus.fine_detritus_g_total = 0.9;
    state.detritus.dissolved_feed_residue_g_total = 1.1;
    state
}

fn quiescent_budget_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
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
    state.animal.adults_count = 0;
    state.animal.juveniles_count = 0;
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

#[test]
fn test_total_n_helper_sums_all_pools() {
    let state = known_budget_state();
    let ratio = state.process_params.feed_n_to_c_ratio;
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let substrate_n_mg: f64 = state
        .substrate_layers
        .iter()
        .map(|layer| layer.nutrient_store_mg_n_total)
        .sum();
    let expected = state.water.ammonia_total_mg_n_total
        + state.water.nitrite_mg_n_total
        + state.water.nitrate_mg_n_total
        + state.water.dissolved_organic_nitrogen_mg_n_total
        + substrate_n_mg
        + plant_nitrogen_mg(state.plant_guilds[0].biomass_g)
        + plant_nitrogen_mg(state.plant_guilds[1].biomass_g)
        + algae_nitrogen_mg(state.algae.suspended_biomass_g)
        + algae_nitrogen_mg(state.algae.periphyton_biomass_g)
        + live_biomass_nitrogen_mg(microbe_biomass_g, ratio)
        + live_biomass_nitrogen_mg(
            shrimp_biomass_g(state.animal.adults_count, state.animal.juveniles_count),
            ratio,
        )
        + detritus_nitrogen_mg(
            state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total,
            ratio,
        );

    assert_close(state.total_nitrogen(), expected, 1e-9);
}

#[test]
fn test_total_c_helper_sums_all_pools() {
    let state = known_budget_state();
    let ratio = state.process_params.feed_n_to_c_ratio;
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let expected = state.water.dissolved_inorganic_carbon_mg_c_total
        + state.water.dissolved_organic_carbon_mg_c_total
        + plant_carbon_mg(state.plant_guilds[0].biomass_g, ratio)
        + plant_carbon_mg(state.plant_guilds[1].biomass_g, ratio)
        + algae_carbon_mg(state.algae.suspended_biomass_g, ratio)
        + algae_carbon_mg(state.algae.periphyton_biomass_g, ratio)
        + live_biomass_carbon_mg(microbe_biomass_g, ratio)
        + live_biomass_carbon_mg(
            shrimp_biomass_g(state.animal.adults_count, state.animal.juveniles_count),
            ratio,
        )
        + detritus_carbon_mg(
            state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total,
            ratio,
        );

    assert_close(state.total_carbon(), expected, 1e-9);
}

#[test]
fn test_closed_system_n_conservation() -> Result<(), SimError> {
    let state = quiescent_budget_state(SimSeed(9_001));
    let initial_total_n = state.total_nitrogen();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(24)?;

    let final_total_n = engine.full_state().total_nitrogen();
    assert_close(final_total_n, initial_total_n, 1e-6);

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    assert_eq!(ledger.ticks.len(), 24);
    assert!(
        ledger
            .ticks
            .iter()
            .all(|tick| tick.net_delta.nitrogen.net_mg().abs() <= 1e-6),
        "expected per-tick nitrogen deltas to remain near zero: {ledger:#?}"
    );
    assert!(ledger.ticks[0]
        .entries
        .iter()
        .any(|entry| entry.label == "system:nitrogen_cycle"));

    Ok(())
}

#[test]
fn test_closed_system_c_conservation() -> Result<(), SimError> {
    let state = quiescent_budget_state(SimSeed(9_002));
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(24)?;

    let final_total_c = engine.full_state().total_carbon();
    assert_close(final_total_c, initial_total_c, 1e-6);

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    assert_eq!(ledger.ticks.len(), 24);
    assert!(
        ledger
            .ticks
            .iter()
            .all(|tick| tick.net_delta.carbon.net_mg().abs() <= 1e-6),
        "expected per-tick carbon deltas to remain near zero: {ledger:#?}"
    );

    Ok(())
}

#[test]
fn test_water_change_n_export_tracked() -> Result<(), SimError> {
    let mut state = quiescent_budget_state(SimSeed(9_003));
    state.substrate_layers[0].nutrient_store_mg_n_total = 0.0;
    state
        .source_water_catalog
        .insert("ro_like".to_string(), SourceWaterProfile::zero());
    let water_n_before = state.water.ammonia_total_mg_n_total
        + state.water.nitrite_mg_n_total
        + state.water.nitrate_mg_n_total
        + state.water.dissolved_organic_nitrogen_mg_n_total;
    let initial_total_n = state.total_nitrogen();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 25.0,
        source_profile_id: "ro_like".to_string(),
    })?;
    engine.step_hours(1)?;

    let expected_export_mg = water_n_before * 0.25;
    let final_total_n = engine.full_state().total_nitrogen();
    assert_close(final_total_n, initial_total_n - expected_export_mg, 1e-6);

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    assert_eq!(ledger.ticks.len(), 1);
    let water_change_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "action:water_change")
        .expect("water change entry should be recorded");
    assert_close(
        water_change_entry.delta.nitrogen.out_mg,
        expected_export_mg,
        1e-6,
    );
    assert_close(water_change_entry.delta.nitrogen.in_mg, 0.0, 1e-9);

    Ok(())
}

#[test]
fn test_budget_ledger_zero_cost_when_disabled() -> Result<(), SimError> {
    let mut engine = Engine::from_parts(quiescent_budget_state(SimSeed(9_004)), vec![]);
    assert!(engine.budget_ledger().is_none());

    engine.step_hours(2)?;
    assert!(
        engine.budget_ledger().is_none(),
        "disabled tracking should not allocate a ledger"
    );

    engine.enable_budget_tracking();
    engine.step_hours(1)?;
    assert_eq!(
        engine
            .budget_ledger()
            .expect("tracking enabled")
            .ticks
            .len(),
        1
    );

    Ok(())
}
