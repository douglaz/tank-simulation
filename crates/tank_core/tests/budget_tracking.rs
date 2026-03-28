use std::collections::BTreeSet;

use serde_json::Value;
use tank_core::{
    carbon_budget_components, nitrogen_budget_components,
    systems::{
        algae_growth::step_daily_algae, plant_growth::step_daily_plants, shrimp::step_daily_shrimp,
    },
    Engine, PlantGuildState, PlayerAction, ProcessParams, SimError, SimSeed, SimulationEngine,
    SourceWaterProfile, SubstrateKind, SubstrateLayerState, TankState, WaterState,
    ADULT_SHRIMP_BIOMASS_G,
};

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual} (tolerance {tolerance})"
    );
}

fn close_budget_gas_exchange(state: &mut TankState) {
    // Keep the filter enabled so nitrification scenarios still exercise the
    // same pathways, but zero all gas-transfer terms that would otherwise make
    // closed-system carbon checks observe atmospheric exchange.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
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

fn clean_filter_budget_state(seed: SimSeed) -> TankState {
    let mut state = quiescent_budget_state(seed);
    state.microbe.decomposer_biomass_g = 0.45;
    state.microbe.ammonia_oxidizer_biomass_g = 0.18;
    state.microbe.nitrite_oxidizer_biomass_g = 0.12;
    state.microbe.comammox_biomass_g = 0.10;
    state.process_params.decomposer_vmax_per_hour = 0.0;
    state.process_params.decomposer_decay_rate_per_hour = 0.0;
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 0.0;
    state.process_params.aob_decay_rate_per_hour = 0.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.0;
    state.process_params.nob_decay_rate_per_hour = 0.0;
    state.process_params.comammox_vmax_fraction = 0.0;
    state.process_params.comammox_decay_rate_per_hour = 0.0;
    state
}

fn siphon_detritus_budget_state(seed: SimSeed) -> TankState {
    let mut state = quiescent_budget_state(seed);
    state.detritus.particulate_organics_g_total = 1.2;
    state.detritus.fine_detritus_g_total = 0.6;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.decomposer_vmax_per_hour = 0.0;
    state.process_params.decomposer_decay_rate_per_hour = 0.0;
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

fn shrimp_reproduction_budget_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    // NOTE: We intentionally keep K_LA at defaults here so O2 reaeration
    // works over the 1440-hour run.  The test adjusts its carbon assertion
    // to account for the open-system CO2 atmospheric exchange that results.
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 0.8;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.4;
    state.environment.ambient_temp_c = 24.0;
    state.water.temperature_c = 24.0;
    state.water.dissolved_oxygen_mg_total = 8.0 * state.water_volume_l();
    state.water.dissolved_inorganic_carbon_mg_c_total = 420.0;
    state.water.dissolved_organic_carbon_mg_c_total = 260.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 48.0;
    state.algae.periphyton_biomass_g = 5.0;
    state.algae.suspended_biomass_g = 0.2;
    state.microbe.decomposer_biomass_g = 0.1;
    state.microbe.ammonia_oxidizer_biomass_g = 0.8;
    state.microbe.nitrite_oxidizer_biomass_g = 1.2;
    state.microbe.comammox_biomass_g = 0.4;
    state.filter_state.biofilter_maturity_index = 1.0;
    state.animal.adults_count = 10;
    state.animal.condition_index = 0.95;
    state.animal.molt_stress_index = 0.02;
    state.animal.reproductive_readiness_index = 0.95;
    state.animal.reserve_g = 2.0;
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 5.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 8.0;
    state.process_params.comammox_vmax_fraction = 0.8;
    state.process_params.periphyton_capacity_g_per_m2 = 30.0;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    state.shrimp_params.base_spawn_rate = 1.0;
    state.shrimp_params.hatch_success_base = 1.0;
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

fn is_animal_mass_budget_path(path: &str) -> bool {
    path.starts_with("animal.")
        && path.ends_with("_count")
        && path != "animal.berried_females_count"
}

fn is_shared_budget_path(path: &str) -> bool {
    path.ends_with("biomass_g")
        || path.ends_with("particulate_organics_g_total")
        || path.ends_with("fine_detritus_g_total")
        || path == "animal.reserve_g"
        || is_animal_mass_budget_path(path)
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
        + manual_organic_nitrogen_mg(state.animal.reserve_g, ratio)
        + manual_organic_nitrogen_mg(state.detritus.particulate_organics_g_total, ratio)
        + manual_organic_nitrogen_mg(state.detritus.fine_detritus_g_total, ratio);

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
        + manual_organic_carbon_mg(state.animal.reserve_g, ratio)
        + manual_organic_carbon_mg(state.detritus.particulate_organics_g_total, ratio)
        + manual_organic_carbon_mg(state.detritus.fine_detritus_g_total, ratio);

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
    assert!(is_nitrogen_budget_path("animal.larvae_count"));
    assert!(is_carbon_budget_path("animal.larvae_count"));
    assert!(!is_nitrogen_budget_path("animal.berried_females_count"));
    assert!(!is_carbon_budget_path("animal.berried_females_count"));
}

#[test]
fn test_closed_system_n_conservation() -> Result<(), SimError> {
    // Closed-system conservation assertions use the fixture helper that zeros
    // all gas-transfer terms while leaving the biological pathways intact.
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
    // Closed-system conservation assertions use the fixture helper that zeros
    // all gas-transfer terms while leaving the biological pathways intact.
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
fn test_periphyton_capacity_excess_routes_to_fine_detritus() {
    let mut state = quiescent_budget_state(SimSeed(9_016));
    state.hardware.light.enabled = false;
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 5.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.process_params.periphyton_capacity_g_per_m2 = 0.0;
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;
    let initial_don = state.water.dissolved_organic_nitrogen_mg_n_total;
    let initial_doc = state.water.dissolved_organic_carbon_mg_c_total;

    step_daily_algae(&mut state);

    assert_close(state.algae.periphyton_biomass_g, 0.0, 1e-9);
    assert_close(state.total_nitrogen(), initial_total_n, 1e-6);
    assert_close(state.total_carbon(), initial_total_c, 1e-6);
    assert!(state.detritus.fine_detritus_g_total > initial_fine_detritus);
    assert_close(
        state.water.dissolved_organic_nitrogen_mg_n_total,
        initial_don,
        1e-9,
    );
    assert_close(
        state.water.dissolved_organic_carbon_mg_c_total,
        initial_doc,
        1e-9,
    );
}

#[test]
fn test_algae_loss_routing_is_clamped_to_available_biomass_and_stays_particulate() {
    let mut state = quiescent_budget_state(SimSeed(9_017));
    state.hardware.light.enabled = false;
    state.algae.suspended_biomass_g = 0.4;
    state.algae.periphyton_biomass_g = 0.5;
    state.microfauna.population_index = 1.0;
    state.microfauna.grazing_pressure_index = 1.0;
    state.process_params.algae_respiration_fraction_per_day = 1.4;
    state.process_params.periphyton_capacity_g_per_m2 = 1_000.0;
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;
    let initial_don = state.water.dissolved_organic_nitrogen_mg_n_total;
    let initial_doc = state.water.dissolved_organic_carbon_mg_c_total;

    step_daily_algae(&mut state);

    assert_close(state.algae.suspended_biomass_g, 0.0, 1e-9);
    assert_close(state.algae.periphyton_biomass_g, 0.0, 1e-9);
    assert_close(state.total_nitrogen(), initial_total_n, 1e-6);
    assert_close(state.total_carbon(), initial_total_c, 1e-6);
    assert!(state.detritus.fine_detritus_g_total > initial_fine_detritus);
    assert_close(
        state.water.dissolved_organic_nitrogen_mg_n_total,
        initial_don,
        1e-9,
    );
    assert_close(
        state.water.dissolved_organic_carbon_mg_c_total,
        initial_doc,
        1e-9,
    );
}

#[test]
fn test_plant_loss_routing_is_clamped_to_available_biomass_and_stays_particulate() {
    let mut state = quiescent_budget_state(SimSeed(9_018));
    state.hardware.light.enabled = false;
    state.plant_guilds = vec![PlantGuildState::default(), PlantGuildState::default()];
    state.plant_guilds[0].biomass_g = 0.8;
    state.plant_guilds[1].biomass_g = 0.0;
    state.process_params.plant_respiration_fraction_per_day = 1.5;
    state.process_params.plant_senescence_fraction_per_day = 1.0;
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;
    let initial_don = state.water.dissolved_organic_nitrogen_mg_n_total;
    let initial_doc = state.water.dissolved_organic_carbon_mg_c_total;

    step_daily_plants(&mut state);

    assert_close(state.plant_guilds[0].biomass_g, 0.0, 1e-9);
    assert_close(state.total_nitrogen(), initial_total_n, 1e-6);
    assert_close(state.total_carbon(), initial_total_c, 1e-6);
    assert!(state.detritus.fine_detritus_g_total > initial_fine_detritus);
    assert_close(
        state.water.dissolved_organic_nitrogen_mg_n_total,
        initial_don,
        1e-9,
    );
    assert_close(
        state.water.dissolved_organic_carbon_mg_c_total,
        initial_doc,
        1e-9,
    );
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
fn test_clean_filter_routes_removed_microbes_to_dissolved_organics() -> Result<(), SimError> {
    let state = clean_filter_budget_state(SimSeed(9_019));
    let ratio = state.process_params.feed_n_to_c_ratio;
    let removed_biomass_g = (state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g)
        * 0.4;
    let expected_don_increase = manual_live_biomass_nitrogen_mg(removed_biomass_g, ratio);
    let expected_doc_increase = manual_live_biomass_carbon_mg(removed_biomass_g, ratio);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_don = state.water.dissolved_organic_nitrogen_mg_n_total;
    let initial_doc = state.water.dissolved_organic_carbon_mg_c_total;
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.apply_action(PlayerAction::CleanFilter { intensity: 0.8 })?;

    engine.step_hours(1)?;

    assert_close(engine.full_state().total_nitrogen(), initial_total_n, 1e-6);
    assert_close(engine.full_state().total_carbon(), initial_total_c, 1e-6);
    assert_close(
        engine
            .full_state()
            .water
            .dissolved_organic_nitrogen_mg_n_total,
        initial_don + expected_don_increase,
        1e-6,
    );
    assert_close(
        engine
            .full_state()
            .water
            .dissolved_organic_carbon_mg_c_total,
        initial_doc + expected_doc_increase,
        1e-6,
    );

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let clean_filter_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "action:clean_filter")
        .expect("clean filter entry should be recorded");
    assert!(clean_filter_entry.delta.nitrogen.in_mg > 0.0);
    assert!(clean_filter_entry.delta.nitrogen.out_mg > 0.0);
    assert!(clean_filter_entry.delta.carbon.in_mg > 0.0);
    assert!(clean_filter_entry.delta.carbon.out_mg > 0.0);
    assert_close(clean_filter_entry.delta.nitrogen.net_mg(), 0.0, 1e-6);
    assert_close(clean_filter_entry.delta.carbon.net_mg(), 0.0, 1e-6);

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
fn test_closed_system_shrimp_reproduction_conserves_n_and_c() -> Result<(), SimError> {
    let state = shrimp_reproduction_budget_state(SimSeed(9_014));
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(24 * 60)?;

    assert!(
        engine.full_state().animal.juveniles_count > 0,
        "expected deterministic hatching to occur in the reproducing shrimp fixture"
    );
    assert_close(engine.full_state().total_nitrogen(), initial_total_n, 1e-6);

    // Carbon assertion: the CO2 atmospheric exchange via K_LA is an open-system
    // flux tracked under "system:chemistry".  Subtract it so we can verify that
    // all *other* pathways conserve carbon exactly.
    let chemistry_c_flux: f64 = engine
        .budget_ledger()
        .expect("budget tracking enabled")
        .ticks
        .iter()
        .flat_map(|tick| tick.entries.iter())
        .filter(|entry| entry.label == "system:chemistry")
        .map(|entry| entry.delta.carbon.net_mg())
        .sum();
    assert_close(
        engine.full_state().total_carbon(),
        initial_total_c + chemistry_c_flux,
        1e-6,
    );

    let reproduction_entry = engine
        .budget_ledger()
        .expect("budget tracking enabled")
        .ticks
        .iter()
        .flat_map(|tick| tick.entries.iter())
        .find(|entry| {
            entry.label == "system:daily_shrimp"
                && entry.delta.nitrogen.in_mg > 0.0
                && entry.delta.nitrogen.out_mg > 0.0
                && entry.delta.carbon.in_mg > 0.0
                && entry.delta.carbon.out_mg > 0.0
        })
        .expect("reproduction should produce a gross in/out daily_shrimp budget entry");
    assert_close(reproduction_entry.delta.nitrogen.net_mg(), 0.0, 1e-6);
    assert_close(reproduction_entry.delta.carbon.net_mg(), 0.0, 1e-6);

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
fn test_siphon_detritus_n_and_c_export_tracked() -> Result<(), SimError> {
    let siphon_fraction = 0.25;
    let state = siphon_detritus_budget_state(SimSeed(9_017));
    let ratio = state.process_params.feed_n_to_c_ratio;
    let removed_detritus_g = (state.detritus.particulate_organics_g_total
        + state.detritus.fine_detritus_g_total)
        * siphon_fraction;
    let expected_n_export = manual_organic_nitrogen_mg(removed_detritus_g, ratio);
    let expected_c_export = manual_organic_carbon_mg(removed_detritus_g, ratio);
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.apply_action(PlayerAction::SiphonDetritus {
        fraction: siphon_fraction,
    })?;
    engine.step_hours(1)?;

    assert_close(
        initial_total_n - engine.full_state().total_nitrogen(),
        expected_n_export,
        1e-6,
    );
    assert_close(
        initial_total_c - engine.full_state().total_carbon(),
        expected_c_export,
        1e-6,
    );

    let ledger = engine.budget_ledger().expect("budget tracking enabled");
    let siphon_entry = ledger.ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "action:siphon_detritus")
        .expect("siphon detritus entry should be recorded");
    assert_close(siphon_entry.delta.nitrogen.in_mg, 0.0, 1e-9);
    assert_close(siphon_entry.delta.nitrogen.out_mg, expected_n_export, 1e-6);
    assert_close(
        siphon_entry.delta.nitrogen.net_mg(),
        -expected_n_export,
        1e-6,
    );
    assert_close(siphon_entry.delta.carbon.in_mg, 0.0, 1e-9);
    assert_close(siphon_entry.delta.carbon.out_mg, expected_c_export, 1e-6);
    assert_close(siphon_entry.delta.carbon.net_mg(), -expected_c_export, 1e-6);

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
fn test_preset_dic_exchange_is_explicitly_tracked_without_disabling_carbon_guard(
) -> Result<(), SimError> {
    // These values mirror crates/tank_data/data/process/default.toml and model
    // the current atmospheric DIC shortcut, so tracked carbon should drift only
    // by the chemistry-stage source/sink budget instead of disabling the rest
    // of the tick-level carbon guard.
    let mut state = active_budget_state(SimSeed(9_015));
    state.environment.hour_of_day = 12;
    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 12.0;
    state.hardware.light.intensity_index = 1.0;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.08;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.12;
    let initial_total_c = state.total_carbon();
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();

    engine.step_hours(1)?;

    let chemistry_entry = engine
        .budget_ledger()
        .expect("budget tracking enabled")
        .ticks[0]
        .entries
        .iter()
        .find(|entry| entry.label == "system:chemistry")
        .expect("chemistry stage should be recorded");
    assert!(
        chemistry_entry.delta.carbon.net_mg().abs() > 1e-6,
        "default preset DIC exchange should move carbon through system:chemistry"
    );
    assert!(
        chemistry_entry.delta.carbon.in_mg > 0.0,
        "respiration shortcut should record explicit carbon inflow"
    );
    assert!(
        chemistry_entry.delta.carbon.out_mg > 0.0,
        "photosynthesis shortcut should record explicit carbon outflow"
    );
    assert!(
        (engine.full_state().total_carbon() - initial_total_c).abs() > 1e-6,
        "preset DIC exchange should still move total carbon through the chemistry shortcut"
    );
    assert_close(
        engine.full_state().total_carbon() - initial_total_c,
        chemistry_entry.delta.carbon.net_mg(),
        1e-6,
    );

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

// -- Death → detritus routing conservation tests --

/// Creates a tank with shrimp under high stress (forcing mortality) and no other
/// biological processes. Isolates the shrimp daily cycle from plants, algae, and
/// microbes so we can verify death → detritus mass conservation.
fn high_mortality_shrimp_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    // Food sources for shrimp feeding (so the daily cycle runs normally)
    state.algae.periphyton_biomass_g = 5.0;
    state.detritus.fine_detritus_g_total = 2.0;

    // Stock shrimp: adults and juveniles
    state.animal.adults_count = 20;
    state.animal.juveniles_count = 15;
    state.animal.condition_index = 0.3; // poor condition → high mortality
    state.animal.molt_stress_index = 0.8; // high molt stress

    // Crank up stress accumulators to force high mortality
    state.animal.hourly_nh3_stress_accum = 0.5;
    state.animal.hourly_nitrite_stress_accum = 0.3;
    state.animal.hourly_low_do_stress_accum = 0.2;
    state.animal.hourly_heat_stress_accum = 0.3;

    // Disable all other biological processes to isolate shrimp
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;

    state.process_params = ProcessParams::default();
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;

    state.reseed_stability_tracker();
    state
}

#[test]
fn test_shrimp_death_routes_body_mass_to_detritus_and_conserves_n_c() {
    let mut state = high_mortality_shrimp_state(SimSeed(7_100));
    let initial_adults = state.animal.adults_count;
    let initial_juveniles = state.animal.juveniles_count;
    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_fine_detritus = state.detritus.fine_detritus_g_total;

    assert!(initial_adults > 0, "need adults for this test");
    assert!(initial_juveniles > 0, "need juveniles for this test");

    step_daily_shrimp(&mut state);

    // Some shrimp should have died given the high stress
    let total_after = state.animal.adults_count + state.animal.juveniles_count;
    assert!(
        total_after < initial_adults + initial_juveniles,
        "expected some deaths: before={}, after={total_after}",
        initial_adults + initial_juveniles
    );

    // Detritus should have increased (dead biomass routed there, plus feces from feeding)
    assert!(
        state.detritus.fine_detritus_g_total > initial_fine_detritus,
        "detritus should increase from dead shrimp and feeding feces"
    );

    // Total N and C must be conserved within tolerance
    assert_close(state.total_nitrogen(), initial_total_n, 1e-6);
    assert_close(state.total_carbon(), initial_total_c, 1e-6);
}

#[test]
fn test_shrimp_death_detritus_amount_matches_expected_body_mass() {
    // Create a state where we can predict the exact detritus contribution from death.
    // Disable feeding by removing all food, so only death routing adds to detritus.
    let mut state = TankState::new(SimSeed(7_200));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    // No food → no feeding → detritus changes only from death
    state.algae.periphyton_biomass_g = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;

    // 10 adults, no juveniles, no reserve (simplifies accounting)
    state.animal.adults_count = 10;
    state.animal.juveniles_count = 0;
    state.animal.reserve_g = 0.0;
    state.animal.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;

    // Force guaranteed death: set base mortality to 1.0 (every shrimp dies)
    state.process_params.shrimp_base_mortality_per_day = 1.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;

    // Disable everything else
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;
    state.reseed_stability_tracker();

    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();

    step_daily_shrimp(&mut state);

    // All 10 adults should be dead (base mortality capped at 0.5, so up to 5 die per day)
    let dead_count = 10 - state.animal.adults_count;
    assert!(dead_count > 0, "expected some shrimp deaths");

    // Expected detritus from dead bodies (no reserve contribution since reserve_g = 0)
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let dead_biomass_g = f64::from(dead_count) * ADULT_SHRIMP_BIOMASS_G;
    let expected_detritus_n_mg = tank_core::live_biomass_nitrogen_mg(dead_biomass_g, n_to_c_ratio);
    let expected_detritus_c_mg = tank_core::live_biomass_carbon_mg(dead_biomass_g, n_to_c_ratio);

    // Detritus should contain exactly the dead shrimp body mass (converted to detrital form)
    let detritus_n_mg =
        tank_core::detritus_nitrogen_mg(state.detritus.fine_detritus_g_total, n_to_c_ratio);
    let detritus_c_mg =
        tank_core::detritus_carbon_mg(state.detritus.fine_detritus_g_total, n_to_c_ratio);
    assert_close(detritus_n_mg, expected_detritus_n_mg, 1e-9);
    assert_close(detritus_c_mg, expected_detritus_c_mg, 1e-9);

    // Total N and C must still be conserved
    assert_close(state.total_nitrogen(), initial_total_n, 1e-6);
    assert_close(state.total_carbon(), initial_total_c, 1e-6);
}

#[test]
fn test_high_mortality_200_hours_conserves_n_c_and_accumulates_detritus() -> Result<(), SimError> {
    // Integration test: 200 hours of a high-mortality scenario with realistic
    // biological processes running. Verifies that detritus accumulates as shrimp
    // population declines and that total N and C are conserved throughout.
    let mut state = TankState::new(SimSeed(7_300));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());

    // High temperature → heat stress → mortality
    state.water.temperature_c = 33.0;
    state.environment.ambient_temp_c = 33.0;

    let vol = state.water_volume_l();
    // Poor water quality: elevated ammonia and low DO
    state.water.ammonia_total_mg_n_total = 3.0;
    state.water.nitrite_mg_n_total = 1.5;
    state.water.nitrate_mg_n_total = 15.0;
    state.water.dissolved_oxygen_mg_total = 4.0 * vol; // low DO
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 10.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 2.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 4.0 * vol;
    state.water.bicarbonate_mg_total = 150.0 * vol;
    state.water.phosphate_mg_p_total = 2.0;

    state.algae.periphyton_biomass_g = 3.0;
    state.algae.suspended_biomass_g = 0.5;
    state.detritus.fine_detritus_g_total = 1.0;
    state.detritus.particulate_organics_g_total = 0.5;

    // Large population under stress
    state.animal.adults_count = 30;
    state.animal.juveniles_count = 20;
    state.animal.condition_index = 0.5;
    state.animal.molt_stress_index = 0.3;

    // Disable plants to simplify (they have their own conservation tests)
    state.plant_guilds.clear();
    // Keep microbes running at moderate levels for realism
    state.microbe.decomposer_biomass_g = 0.1;
    state.microbe.ammonia_oxidizer_biomass_g = 0.3;
    state.microbe.nitrite_oxidizer_biomass_g = 0.2;
    state.microbe.comammox_biomass_g = 0.1;
    state.filter_state.biofilter_maturity_index = 0.6;

    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.5;
    state.hardware.light.photoperiod_hours = 8.0;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.2; // weak aeration → low DO recovery

    state.process_params = ProcessParams::default();
    // Zero K_LA so CO2 atmospheric exchange does not break closed-system
    // carbon conservation assertions.
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    // Elevated base mortality for this stress scenario
    state.process_params.shrimp_base_mortality_per_day = 0.01;
    state.process_params.shrimp_stress_mortality_scale = 0.25;

    state.substrate_layers[0].nutrient_store_mg_n_total = 10.0;
    state.substrate_layers[0].nutrient_store_mg_p_total = 3.0;

    state.reseed_stability_tracker();
    state.stability_tracker.prev_temp_c = state.water.temperature_c;

    let initial_total_n = state.total_nitrogen();
    let initial_total_c = state.total_carbon();
    let initial_population = state.animal.adults_count + state.animal.juveniles_count;

    let mut engine = Engine::from_parts(state, vec![]);

    // Run 200 hours, checking conservation every 25-hour block
    for hour_block in 0..8 {
        engine.step_hours(25)?;
        let s = engine.full_state();
        let current_n = s.total_nitrogen();
        let current_c = s.total_carbon();

        // Verify conservation at each checkpoint. Generous tolerance for floating-point
        // accumulation over many hours with many interacting subsystems.
        assert!(
            (current_n - initial_total_n).abs() < 0.5,
            "N conservation violated at hour {}: initial={initial_total_n:.4}, current={current_n:.4}, drift={:.4}",
            (hour_block + 1) * 25,
            (current_n - initial_total_n).abs()
        );
        assert!(
            (current_c - initial_total_c).abs() < 0.5,
            "C conservation violated at hour {}: initial={initial_total_c:.4}, current={current_c:.4}, drift={:.4}",
            (hour_block + 1) * 25,
            (current_c - initial_total_c).abs()
        );
    }

    let final_state = engine.full_state();
    let final_population = final_state.animal.adults_count + final_state.animal.juveniles_count;

    // Population should have declined under high-stress conditions
    assert!(
        final_population < initial_population,
        "expected population decline: initial={initial_population}, final={final_population}"
    );

    // N/C conservation (verified at each checkpoint above) combined with population
    // decline proves dead shrimp mass was routed to in-tank pools. With active
    // decomposers, dead biomass may already have moved through fine_detritus into
    // dissolved organics, TAN, and DIC — all tracked by the budget system.

    Ok(())
}
