use tank_core::{
    systems::{algae_growth::step_daily_algae, plant_growth::step_daily_plants},
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
    let favorable_volume_l = favorable.water_volume_l();
    favorable.water.ammonia_total_mg_n_total = 2.0 * favorable_volume_l;
    favorable.water.nitrate_mg_n_total = 10.0 * favorable_volume_l;
    favorable.water.phosphate_mg_p_total = 1.2 * favorable_volume_l;
    favorable.water.dissolved_inorganic_carbon_mg_c_total = 24.0 * favorable_volume_l;

    let mut poor = favorable.clone();
    poor.hardware.light.intensity_index = 0.15;
    let poor_volume_l = poor.water_volume_l();
    poor.water.ammonia_total_mg_n_total = 0.02 * poor_volume_l;
    poor.water.nitrate_mg_n_total = 0.08 * poor_volume_l;
    poor.water.phosphate_mg_p_total = 0.01 * poor_volume_l;
    poor.water.dissolved_inorganic_carbon_mg_c_total = 2.0 * poor_volume_l;

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
    control.water.ammonia_total_mg_n_total = 0.2;
    control.water.nitrate_mg_n_total = 1.0;
    control.water.phosphate_mg_p_total = 0.4;
    control.microfauna.population_index = 0.0;
    control.microfauna.grazing_pressure_index = 0.0;
    let overfed = control.clone();

    let mut control_engine = Engine::from_parts(control, vec![]);
    let mut overfed_engine = Engine::from_parts(overfed, vec![]);
    for _ in 0..21 {
        control_engine.apply_action(PlayerAction::Feed { grams: 0.0 })?;
        overfed_engine.apply_action(PlayerAction::Feed { grams: 2.5 })?;
        control_engine.step_hours(24)?;
        overfed_engine.step_hours(24)?;
    }

    assert!(
        overfed_engine.snapshot().algae_nuisance_index
            > control_engine.snapshot().algae_nuisance_index,
        "overfeeding should increase algae nuisance pressure (control={:.3}, overfed={:.3})",
        control_engine.snapshot().algae_nuisance_index,
        overfed_engine.snapshot().algae_nuisance_index
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

#[test]
fn plant_water_column_limitation_is_volume_invariant_at_fixed_concentration() {
    let build_state = |fill_height_cm: f64| {
        let mut state = base_growth_state(SimSeed(8105));
        state.geometry.fill_height_cm = fill_height_cm;
        state.substrate_layers.clear();
        let volume_l = state.water_volume_l();
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        }];
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 2.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.3 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 12.0 * volume_l;
        state
    };

    let mut shallow = build_state(8.0);
    let mut deep = build_state(18.0);

    step_daily_plants(&mut shallow);
    step_daily_plants(&mut deep);

    assert!(
        (shallow.plant_guilds[0].biomass_g - deep.plant_guilds[0].biomass_g).abs() <= 1e-9,
        "Same water-column nutrient concentrations should yield the same plant growth regardless of tank volume"
    );
}

#[test]
fn later_plant_guilds_see_updated_water_column_chemistry() {
    let mut state = base_growth_state(SimSeed(8108));
    state.substrate_layers.clear();
    state.plant_guilds = vec![
        PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        },
        PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        },
    ];

    let volume_l = state.water_volume_l();
    state.water.ammonia_total_mg_n_total = 0.1 * volume_l;
    state.water.nitrate_mg_n_total = 0.6 * volume_l;
    state.water.phosphate_mg_p_total = 0.08 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 30.0 * volume_l;

    step_daily_plants(&mut state);

    assert!(
        state.plant_guilds[0].biomass_g > state.plant_guilds[1].biomass_g + 0.005,
        "later guilds should compute growth against the depleted water column after earlier guild uptake"
    );
    assert!(
        state.plant_guilds[0].health_index > state.plant_guilds[1].health_index,
        "later guilds should also register the stronger nutrient stress after earlier guild uptake"
    );
}

#[test]
fn plant_substrate_limitation_drops_when_areal_store_is_depleted() {
    let build_state = |substrate_n_mg_total: f64, substrate_p_mg_total: f64| {
        let mut state = base_growth_state(SimSeed(8107));
        let volume_l = state.water_volume_l();
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::RootFeedingRosette,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(0.2),
            substrate_uptake_bias: Some(0.8),
        }];
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 4.0,
            nutrient_store_mg_n_total: substrate_n_mg_total,
            nutrient_store_mg_p_total: substrate_p_mg_total,
            cation_exchange_capacity_index: 0.9,
            detritus_trapping_index: 0.4,
            colonizable_area_cm2: state.geometry.footprint_area_cm2(),
            low_oxygen_tendency_index: 0.4,
            grazing_surface_index: 0.5,
        }];
        state.water.ammonia_total_mg_n_total = 0.08 * volume_l;
        state.water.nitrate_mg_n_total = 0.24 * volume_l;
        state.water.phosphate_mg_p_total = 0.04 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 18.0 * volume_l;
        state
    };

    let mut rich = build_state(40.0, 6.0);
    let mut depleted = build_state(8.0, 1.2);

    assert!(
        rich.substrate_n_mg_n_per_m2() > depleted.substrate_n_mg_n_per_m2(),
        "rich substrate should start with a higher areal N density"
    );
    assert!(
        rich.substrate_p_mg_p_per_m2() > depleted.substrate_p_mg_p_per_m2(),
        "rich substrate should start with a higher areal P density"
    );

    step_daily_plants(&mut rich);
    step_daily_plants(&mut depleted);

    assert!(
        rich.plant_guilds[0].biomass_g > depleted.plant_guilds[0].biomass_g + 0.03,
        "depleted substrate should materially reduce rooted-plant growth"
    );
}

#[test]
fn algae_water_column_limitation_is_volume_invariant_at_fixed_concentration() {
    let build_state = |substrate_depth_cm: f64| {
        let mut state = base_growth_state(SimSeed(8106));
        state.plant_guilds.clear();
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::InertSand,
            depth_cm: substrate_depth_cm,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.3,
            colonizable_area_cm2: 500.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.4,
        }];
        let volume_l = state.water_volume_l();
        state.algae.suspended_biomass_g = 0.5;
        state.algae.periphyton_biomass_g = 0.4;
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 2.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.3 * volume_l;
        state
    };

    let mut shallow = build_state(1.0);
    let mut deep = build_state(6.0);

    step_daily_algae(&mut shallow);
    step_daily_algae(&mut deep);

    assert!(
        (shallow.algae.suspended_biomass_g - deep.algae.suspended_biomass_g).abs() <= 1e-9,
        "Same nutrient concentrations should yield the same suspended-algae growth regardless of tank volume"
    );
    assert!(
        (shallow.algae.periphyton_biomass_g - deep.algae.periphyton_biomass_g).abs() <= 1e-9,
        "Same nutrient concentrations should yield the same periphyton growth regardless of tank volume"
    );
}
