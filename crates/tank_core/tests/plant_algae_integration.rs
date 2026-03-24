use tank_core::{
    Engine, EventKind, PlantGuild, PlantGuildState, PlayerAction, SimSeed, SimulationEngine,
    SubstrateKind, SubstrateLayerState, TankState,
};

fn base_growth_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrate_mg_n_total = 40.0;
    state.water.phosphate_mg_p_total = 6.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 260.0;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.microfauna.population_index = 0.1;
    state.microfauna.grazing_pressure_index = 0.1;
    state
}

#[test]
fn plant_growth_improves_with_light_and_nutrients() -> Result<(), tank_core::SimError> {
    let mut favorable = base_growth_state(SimSeed(8100));
    favorable.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 4.0,
        health_index: 0.8,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];

    let mut poor = favorable.clone();
    poor.hardware.light.intensity_index = 0.15;
    poor.water.ammonia_total_mg_n_total = 0.2;
    poor.water.nitrate_mg_n_total = 0.8;
    poor.water.phosphate_mg_p_total = 0.1;

    let mut favorable_engine = Engine::from_parts(favorable, vec![]);
    let mut poor_engine = Engine::from_parts(poor, vec![]);

    favorable_engine.step_hours(24 * 14)?;
    poor_engine.step_hours(24 * 14)?;

    assert!(
        favorable_engine.snapshot().fast_stem_biomass_g
            > poor_engine.snapshot().fast_stem_biomass_g * 1.2,
        "favorable conditions should support materially better growth"
    );

    Ok(())
}

#[test]
fn guild_differentiated_uptake_prefers_expected_pools() -> Result<(), tank_core::SimError> {
    let mut fast_stem_state = base_growth_state(SimSeed(8101));
    fast_stem_state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 5.0,
        health_index: 0.85,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];
    fast_stem_state.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::ActivePlanted,
        depth_cm: 3.0,
        nutrient_store_mg_n_total: 60.0,
        nutrient_store_mg_p_total: 12.0,
        cation_exchange_capacity_index: 0.8,
        detritus_trapping_index: 0.4,
        colonizable_area_cm2: 500.0,
        low_oxygen_tendency_index: 0.3,
        grazing_surface_index: 0.4,
    }];

    let mut rosette_state = fast_stem_state.clone();
    rosette_state.plant_guilds[0].guild = PlantGuild::RootFeedingRosette;
    rosette_state.plant_guilds[0].water_column_uptake_bias = Some(0.3);
    rosette_state.plant_guilds[0].substrate_uptake_bias = Some(0.9);

    let fast_initial_water = fast_stem_state.water.ammonia_total_mg_n_total
        + fast_stem_state.water.nitrate_mg_n_total
        + fast_stem_state.water.phosphate_mg_p_total;
    let fast_initial_substrate = fast_stem_state.substrate_layers[0].nutrient_store_mg_n_total
        + fast_stem_state.substrate_layers[0].nutrient_store_mg_p_total;
    let rosette_initial_water = rosette_state.water.ammonia_total_mg_n_total
        + rosette_state.water.nitrate_mg_n_total
        + rosette_state.water.phosphate_mg_p_total;
    let rosette_initial_substrate = rosette_state.substrate_layers[0].nutrient_store_mg_n_total
        + rosette_state.substrate_layers[0].nutrient_store_mg_p_total;

    let mut fast_engine = Engine::from_parts(fast_stem_state, vec![]);
    let mut rosette_engine = Engine::from_parts(rosette_state, vec![]);
    fast_engine.step_hours(24)?;
    rosette_engine.step_hours(24)?;

    let fast = fast_engine.full_state();
    let rosette = rosette_engine.full_state();
    let fast_water_loss = fast_initial_water
        - (fast.water.ammonia_total_mg_n_total
            + fast.water.nitrate_mg_n_total
            + fast.water.phosphate_mg_p_total);
    let fast_substrate_loss = fast_initial_substrate
        - (fast.substrate_layers[0].nutrient_store_mg_n_total
            + fast.substrate_layers[0].nutrient_store_mg_p_total);
    let rosette_water_loss = rosette_initial_water
        - (rosette.water.ammonia_total_mg_n_total
            + rosette.water.nitrate_mg_n_total
            + rosette.water.phosphate_mg_p_total);
    let rosette_substrate_loss = rosette_initial_substrate
        - (rosette.substrate_layers[0].nutrient_store_mg_n_total
            + rosette.substrate_layers[0].nutrient_store_mg_p_total);

    assert!(
        fast_water_loss > fast_substrate_loss,
        "fast stems should pull more nutrients from the water column"
    );
    assert!(
        rosette_substrate_loss > rosette_water_loss,
        "root-feeding rosettes should pull more nutrients from substrate stores"
    );

    Ok(())
}

#[test]
fn algae_bloom_conditions_emit_events_and_overfeeding_raises_nuisance(
) -> Result<(), tank_core::SimError> {
    let mut bloom_state = base_growth_state(SimSeed(8102));
    bloom_state.plant_guilds.clear();
    bloom_state.algae.suspended_biomass_g = 2.5;
    bloom_state.algae.periphyton_biomass_g = 0.5;
    bloom_state.water.ammonia_total_mg_n_total = 12.0;
    bloom_state.water.nitrate_mg_n_total = 50.0;
    bloom_state.water.phosphate_mg_p_total = 8.0;
    bloom_state.environment.ambient_temp_c = 28.0;
    bloom_state.water.temperature_c = 28.0;

    let mut bloom_engine = Engine::from_parts(bloom_state.clone(), vec![]);
    bloom_engine.step_hours(24)?;

    assert!(
        bloom_engine
            .full_state()
            .event_log
            .iter()
            .any(|event| event.kind == EventKind::AlgaeBloom),
        "daily algae step should emit an AlgaeBloom event when suspended biomass crosses threshold"
    );

    let mut control = base_growth_state(SimSeed(8104));
    control.plant_guilds.clear();
    control.environment.ambient_temp_c = 28.0;
    control.water.temperature_c = 28.0;
    control.algae.suspended_biomass_g = 0.05;
    control.algae.periphyton_biomass_g = 0.2;
    control.water.ammonia_total_mg_n_total = 1.0;
    control.water.nitrate_mg_n_total = 4.0;
    control.water.phosphate_mg_p_total = 4.0;
    let overfed = control.clone();

    let mut control_engine = Engine::from_parts(control, vec![]);
    let mut overfed_engine = Engine::from_parts(overfed, vec![]);
    for _ in 0..14 {
        control_engine.apply_action(PlayerAction::Feed { grams: 0.2 })?;
        overfed_engine.apply_action(PlayerAction::Feed { grams: 1.5 })?;
        control_engine.step_hours(24)?;
        overfed_engine.step_hours(24)?;
    }

    assert!(
        overfed_engine.snapshot().algae_nuisance_index
            > control_engine.snapshot().algae_nuisance_index,
        "overfeeding should increase algae nuisance pressure"
    );

    Ok(())
}

#[test]
fn trim_plants_routes_mass_to_detritus() -> Result<(), tank_core::SimError> {
    let mut state = base_growth_state(SimSeed(8103));
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 10.0,
        health_index: 0.9,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::TrimPlants { fraction: 0.25 })?;
    engine.step_hours(1)?;

    let state = engine.full_state();
    assert!((state.plant_guilds[0].biomass_g - 7.5).abs() < 1e-6);
    assert!((state.detritus.fine_detritus_g_total - 2.5).abs() < 1e-6);

    Ok(())
}
