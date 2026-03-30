use tank_core::{
    Engine, PlantGuild, PlantGuildState, SimSeed, SimulationEngine, SubstrateKind,
    SubstrateLayerState, TankState,
};

fn rosette_state(seed: SimSeed, substrate_kind: SubstrateKind) -> TankState {
    let mut state = TankState::new(seed);
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.water.ammonia_total_mg_n_total = 1.0;
    state.water.nitrate_mg_n_total = 4.0;
    state.water.phosphate_mg_p_total = 0.4;
    state.water.dissolved_inorganic_carbon_mg_c_total = 240.0;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::RootFeedingRosette,
        biomass_g: 4.0,
        health_index: 0.85,
        crowding_index: 0.0,
        habitat_index: 0.5,
        water_column_uptake_bias: Some(0.3),
        substrate_uptake_bias: Some(0.9),
    }];
    state.algae.suspended_biomass_g = 0.0;
    state.algae.set_periphyton_total(0.0);
    state.microfauna.population_index = 0.1;
    state.microfauna.grazing_pressure_index = 0.1;
    state.substrate_layers = vec![match substrate_kind {
        SubstrateKind::ActivePlanted => SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 4.0,
            nutrient_store_mg_n_total: 90.0,
            nutrient_store_mg_p_total: 18.0,
            cation_exchange_capacity_index: 0.9,
            detritus_trapping_index: 0.4,
            colonizable_area_factor: SubstrateKind::ActivePlanted.default_colonizable_area_factor(),
            colonizable_area_cm2: 600.0,
            low_oxygen_tendency_index: 0.4,
            grazing_surface_index: 0.5,
        },
        _ => SubstrateLayerState {
            kind: SubstrateKind::InertSand,
            depth_cm: 4.0,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.05,
            detritus_trapping_index: 0.2,
            colonizable_area_factor: SubstrateKind::InertSand.default_colonizable_area_factor(),
            colonizable_area_cm2: 450.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.3,
        },
    }];
    state
}

#[test]
fn rooted_substrate_advantage() -> Result<(), tank_core::SimError> {
    let mut active = Engine::from_parts(
        rosette_state(SimSeed(8001), SubstrateKind::ActivePlanted),
        vec![],
    );
    let mut inert = Engine::from_parts(
        rosette_state(SimSeed(8001), SubstrateKind::InertSand),
        vec![],
    );

    active.step_hours(24 * 30)?;
    inert.step_hours(24 * 30)?;

    let active_biomass = active.snapshot().root_feeding_rosette_biomass_g;
    let inert_biomass = inert.snapshot().root_feeding_rosette_biomass_g;

    assert!(
        active_biomass >= inert_biomass * 1.15,
        "active substrate biomass {active_biomass:.3} should be at least 15% above inert {inert_biomass:.3}"
    );
    assert!(
        active.snapshot().substrate_nutrient_remaining_mg_n_total < 90.0,
        "rooted uptake should deplete active substrate nutrients"
    );

    Ok(())
}
