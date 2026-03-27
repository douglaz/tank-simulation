use std::collections::BTreeSet;

use serde_json::Value;
use tank_core::{
    carbon_budget_components, nitrogen_budget_components, Engine, PlantGuildState, PlayerAction,
    SimError, SimSeed, SimulationEngine, SourceWaterProfile, SubstrateKind, SubstrateLayerState,
    TankState,
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

fn active_budget_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
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
    state.animal.adults_count = 0;
    state.animal.juveniles_count = 0;
    state.animal.berried_females_count = 0;
    state.substrate_layers[0].nutrient_store_mg_n_total = 22.0;
    state.substrate_layers[0].nutrient_store_mg_p_total = 6.0;
    state.reseed_stability_tracker();
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

fn trim_plants_budget_state(seed: SimSeed) -> TankState {
    let mut state = quiescent_budget_state(seed);
    state.plant_guilds = vec![PlantGuildState::default(), PlantGuildState::default()];
    state.plant_guilds[0].biomass_g = 2.0;
    state.plant_guilds[1].biomass_g = 3.5;
    state
}

fn shrimp_mortality_budget_state(seed: SimSeed) -> TankState {
    let mut state = quiescent_budget_state(seed);
    state.animal.adults_count = 5;
    state.animal.condition_index = 0.0;
    state.animal.molt_stress_index = 1.0;
    state.process_params.shrimp_base_mortality_per_day = 0.5;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.shrimp_params.hatch_success_base = 0.0;
    state.reseed_stability_tracker();
    state
}

fn manual_organic_nitrogen_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    mass_g * 1000.0 * n_to_c_ratio / (1.0 + n_to_c_ratio)
}

fn manual_organic_carbon_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    mass_g * 1000.0 / (1.0 + n_to_c_ratio)
}

fn manual_live_biomass_nitrogen_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    manual_organic_nitrogen_mg(biomass_g * 0.20, n_to_c_ratio)
}

fn manual_live_biomass_carbon_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    manual_organic_carbon_mg(biomass_g * 0.20, n_to_c_ratio)
}

fn manual_shrimp_biomass_g(adults_count: u32, juveniles_count: u32) -> f64 {
    f64::from(adults_count) * 0.12 + f64::from(juveniles_count) * 0.05
}

fn collect_numeric_paths(value: &Value, prefix: &str, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                collect_numeric_paths(child, &next, paths);
            }
        }
        Value::Array(items) => {
            for child in items {
                let next = format!("{prefix}[*]");
                collect_numeric_paths(child, &next, paths);
            }
        }
        Value::Number(_) => {
            paths.insert(prefix.to_string());
        }
        _ => {}
    }
}

fn is_shared_budget_path(path: &str) -> bool {
    path.ends_with("biomass_g")
        || path.ends_with("particulate_organics_g_total")
        || path.ends_with("fine_detritus_g_total")
        || matches!(path, "animal.adults_count" | "animal.juveniles_count")
}

fn is_nitrogen_budget_path(path: &str) -> bool {
    is_shared_budget_path(path) || path.ends_with("_mg_n_total")
}

fn is_carbon_budget_path(path: &str) -> bool {
    is_shared_budget_path(path) || path.ends_with("_mg_c_total")
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
        + state.plant_guilds[0].biomass_g * 28.0
        + state.plant_guilds[1].biomass_g * 28.0
        + state.algae.suspended_biomass_g * 35.0
        + state.algae.periphyton_biomass_g * 35.0
        + manual_live_biomass_nitrogen_mg(microbe_biomass_g, ratio)
        + manual_live_biomass_nitrogen_mg(
            manual_shrimp_biomass_g(state.animal.adults_count, state.animal.juveniles_count),
            ratio,
        )
        + manual_organic_nitrogen_mg(
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
        + (state.plant_guilds[0].biomass_g * 28.0 / ratio)
        + (state.plant_guilds[1].biomass_g * 28.0 / ratio)
        + (state.algae.suspended_biomass_g * 35.0 / ratio)
        + (state.algae.periphyton_biomass_g * 35.0 / ratio)
        + manual_live_biomass_carbon_mg(microbe_biomass_g, ratio)
        + manual_live_biomass_carbon_mg(
            manual_shrimp_biomass_g(state.animal.adults_count, state.animal.juveniles_count),
            ratio,
        )
        + manual_organic_carbon_mg(
            state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total,
            ratio,
        );

    assert_close(state.total_carbon(), expected, 1e-9);
}

#[test]
fn test_budget_component_labels_cover_all_element_bearing_fields() {
    let state = known_budget_state();
    let mut numeric_paths = BTreeSet::new();
    collect_numeric_paths(
        &serde_json::to_value(&state).expect("state should serialize"),
        "",
        &mut numeric_paths,
    );

    let nitrogen_paths: BTreeSet<_> = numeric_paths
        .iter()
        .filter(|path| is_nitrogen_budget_path(path))
        .cloned()
        .collect();
    let carbon_paths: BTreeSet<_> = numeric_paths
        .iter()
        .filter(|path| is_carbon_budget_path(path))
        .cloned()
        .collect();
    let nitrogen_labels: BTreeSet<_> = nitrogen_budget_components(&state)
        .into_iter()
        .map(|component| component.label.to_string())
        .collect();
    let carbon_labels: BTreeSet<_> = carbon_budget_components(&state)
        .into_iter()
        .map(|component| component.label.to_string())
        .collect();

    assert_eq!(nitrogen_labels, nitrogen_paths);
    assert_eq!(carbon_labels, carbon_paths);
    assert!(
        !nitrogen_paths.contains("detritus.dissolved_feed_residue_g_total")
            && !carbon_paths.contains("detritus.dissolved_feed_residue_g_total"),
        "dissolved feed residue must stay excluded because it mirrors DOC/DON"
    );
}

#[test]
fn test_budget_path_rules_catch_convention_based_future_fields() {
    assert!(is_nitrogen_budget_path("water.future_pool_mg_n_total"));
    assert!(is_carbon_budget_path("water.future_pool_mg_c_total"));
    assert!(is_nitrogen_budget_path("microbe.future_biomass_g"));
    assert!(is_carbon_budget_path("microbe.future_biomass_g"));
}

#[test]
fn test_closed_system_n_conservation() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_001));
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
    assert!(ledger.ticks.iter().any(|tick| tick
        .entries
        .iter()
        .any(|entry| entry.label == "system:nitrogen_cycle")));
    assert!(ledger.ticks.iter().any(|tick| tick
        .entries
        .iter()
        .any(|entry| entry.label == "system:daily_plants")));

    Ok(())
}

#[test]
fn test_closed_system_c_conservation() -> Result<(), SimError> {
    let state = active_budget_state(SimSeed(9_002));
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
    assert!(ledger.ticks.iter().any(|tick| tick
        .entries
        .iter()
        .any(|entry| entry.label == "system:daily_algae")));
    assert!(ledger.ticks.iter().any(|tick| tick
        .entries
        .iter()
        .any(|entry| entry.label == "system:daily_microfauna")));

    Ok(())
}

#[test]
fn test_trim_plants_preserves_n_and_c_when_routed_to_detritus() -> Result<(), SimError> {
    let state = trim_plants_budget_state(SimSeed(9_010));
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.apply_action(PlayerAction::TrimPlants { fraction: 0.25 })?;

    engine.step_hours(1)?;

    assert_close(engine.full_state().total_nitrogen(), initial_total_n, 1e-6);
    assert_close(engine.full_state().total_carbon(), initial_total_c, 1e-6);

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let trim_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "action:trim_plants")
        .expect("trim plants entry should be recorded");
    assert!(trim_entry.delta.nitrogen.in_mg > 0.0);
    assert!(trim_entry.delta.nitrogen.out_mg > 0.0);
    assert!(trim_entry.delta.carbon.in_mg > 0.0);
    assert!(trim_entry.delta.carbon.out_mg > 0.0);
    assert_close(trim_entry.delta.nitrogen.net_mg(), 0.0, 1e-6);
    assert_close(trim_entry.delta.carbon.net_mg(), 0.0, 1e-6);

    Ok(())
}

#[test]
fn test_closed_system_shrimp_mortality_conserves_n_and_c() -> Result<(), SimError> {
    let state = shrimp_mortality_budget_state(SimSeed(9_011));
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(72)?;

    assert!(
        engine.full_state().animal.adults_count < 5,
        "expected deterministic shrimp mortality to occur"
    );
    assert_close(engine.full_state().total_nitrogen(), initial_total_n, 1e-6);
    assert_close(engine.full_state().total_carbon(), initial_total_c, 1e-6);
    assert!(engine
        .budget_ledger()
        .expect("budget tracking enabled")
        .ticks
        .iter()
        .any(|tick| tick
            .entries
            .iter()
            .any(|entry| entry.label == "system:daily_shrimp")));

    Ok(())
}

#[test]
fn test_water_change_n_export_tracked() -> Result<(), SimError> {
    let mut state = quiescent_budget_state(SimSeed(9_003));
    state.substrate_layers[0].nutrient_store_mg_n_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 1.6;
    state
        .source_water_catalog
        .insert("ro_like".to_string(), SourceWaterProfile::zero());
    let residue_before = state.detritus.dissolved_feed_residue_g_total;
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
    assert_close(
        engine.full_state().detritus.dissolved_feed_residue_g_total,
        residue_before * 0.75,
        1e-9,
    );

    Ok(())
}

#[test]
fn test_water_change_tracks_gross_n_import_and_export() -> Result<(), SimError> {
    let mut state = quiescent_budget_state(SimSeed(9_012));
    state.substrate_layers[0].nutrient_store_mg_n_total = 0.0;
    let exchanged_fraction = 0.25;
    let exchanged_l = state.water_volume_l() * exchanged_fraction;
    let water_n_before = state.water.ammonia_total_mg_n_total
        + state.water.nitrite_mg_n_total
        + state.water.nitrate_mg_n_total
        + state.water.dissolved_organic_nitrogen_mg_n_total;
    state.source_water_catalog.insert(
        "buffered".to_string(),
        SourceWaterProfile {
            ammonia_mg_n_per_l: 0.4,
            nitrite_mg_n_per_l: 0.2,
            nitrate_mg_n_per_l: 3.0,
            don_mg_n_per_l: 0.6,
            ..SourceWaterProfile::zero()
        },
    );
    let expected_export_mg = water_n_before * exchanged_fraction;
    let expected_import_mg = exchanged_l * (0.4 + 0.2 + 3.0 + 0.6);
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 25.0,
        source_profile_id: "buffered".to_string(),
    })?;
    engine.step_hours(1)?;

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
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
    assert_close(
        water_change_entry.delta.nitrogen.in_mg,
        expected_import_mg,
        1e-6,
    );

    Ok(())
}

#[test]
fn test_dissolved_oxygen_stage_tracks_gross_in_and_out() -> Result<(), SimError> {
    let mut state = active_budget_state(SimSeed(9_013));
    state.environment.hour_of_day = 12;
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.water.dissolved_oxygen_mg_total = 40.0;
    state.reseed_stability_tracker();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(1)?;

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let do_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "system:dissolved_oxygen")
        .expect("dissolved oxygen entry should be recorded");
    assert!(do_entry.delta.oxygen.in_mg > 0.0);
    assert!(do_entry.delta.oxygen.out_mg > 0.0);

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
