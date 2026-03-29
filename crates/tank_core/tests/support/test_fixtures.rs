use std::path::PathBuf;

use tank_core::{
    Engine, JsonLinesSink, PlantGuildState, SimSeed, SourceWaterProfile, TankState, TraceSink,
    WaterState,
};

/// Disable atmospheric gas exchange for closed-system tests.
pub fn close_gas_exchange(state: &mut TankState) {
    state.process_params.reaeration_kla_base = 0.0;
    state.process_params.aeration_kla_boost = 0.0;
    state.hardware.filter.flow_lph = 0.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
}

/// Disable hourly chemistry DIC shortcuts for clean carbon budget.
pub fn disable_dic_shortcuts(state: &mut TankState) {
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
}

/// Dump the current trace to a temp-file subdirectory and return the artifact path.
pub fn dump_trace_to_subdir(label: &str, engine: &Engine, trace_subdir: &str) -> Option<PathBuf> {
    let tracer = engine.tracer()?;
    let dir = std::env::temp_dir().join(trace_subdir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{label}.jsonl"));
    let mut buf = Vec::new();
    {
        let mut sink = JsonLinesSink::new(&mut buf);
        for tick in tracer.ticks() {
            let _ = sink.emit_tick(tick);
        }
    }
    let _ = std::fs::write(&path, &buf);
    Some(path)
}

/// Minimal tank isolating shrimp grazing: shrimp + periphyton only.
/// All other biology (plants, decomposers, nitrifiers, microfauna) is disabled
/// so the only mass movement is periphyton -> shrimp ingestion -> excretion/feces.
pub fn grazing_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_1));
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
    state.water.dissolved_organic_carbon_mg_c_total = 0.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.0;
    state.water.ammonia_total_mg_n_total = 1.0;
    state.water.nitrite_mg_n_total = 0.0;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    state.algae.periphyton_biomass_g = 5.0;
    state.detritus.particulate_organics_g_total = 0.0;
    state.detritus.fine_detritus_g_total = 0.0;
    state.detritus.dissolved_feed_residue_g_total = 0.0;

    state.animal.adult.count = 8;
    state.animal.adult.condition_index = 0.8;

    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microbe.decomposer_biomass_g = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = false;
    state.hardware.filter.enabled = false;

    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;

    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

/// Full-biology tank with pre-loaded feed, gas exchange disabled.
/// Exercises the complete feed -> detritus -> DOC -> mineralization ->
/// nitrification chain alongside shrimp grazing and plant/algae dynamics.
pub fn feeding_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_2));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 1.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.16;
    state.water.ammonia_total_mg_n_total = 2.0;
    state.water.nitrite_mg_n_total = 0.5;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.phosphate_mg_p_total = 2.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    state.algae.periphyton_biomass_g = 2.0;
    state.algae.suspended_biomass_g = 0.1;
    state.microbe.decomposer_biomass_g = 0.15;
    state.microbe.ammonia_oxidizer_biomass_g = 0.1;
    state.microbe.nitrite_oxidizer_biomass_g = 0.08;
    state.microbe.comammox_biomass_g = 0.03;
    state.microfauna.population_index = 0.3;
    state.microfauna.grazing_pressure_index = 0.2;
    state.animal.adult.count = 5;
    state.animal.adult.condition_index = 0.8;

    state.detritus.particulate_organics_g_total += 0.5;

    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);

    state.hardware.light.enabled = true;
    state.hardware.light.photoperiod_hours = 10.0;
    state.hardware.light.intensity_index = 0.7;

    state.shrimp_params.base_spawn_rate = 0.0;

    state.reseed_stability_tracker();
    state
}

/// Tank with high shrimp mortality and active plant senescence.
/// Decomposers and nitrifiers are present for downstream processing.
pub fn mortality_senescence_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_3));
    state.geometry.length_cm = 40.0;
    state.geometry.width_cm = 30.0;
    state.geometry.height_cm = 35.0;
    state.geometry.fill_height_cm = 30.0;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 2.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.5;
    state.water.ammonia_total_mg_n_total = 2.0;
    state.water.nitrite_mg_n_total = 0.5;
    state.water.nitrate_mg_n_total = 5.0;
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    if !state.plant_guilds.is_empty() {
        state.plant_guilds[0].biomass_g = 5.0;
    }
    if state.plant_guilds.len() > 1 {
        state.plant_guilds[1].biomass_g = 4.0;
    }
    state.process_params.plant_respiration_fraction_per_day = 0.08;
    state.process_params.plant_senescence_fraction_per_day = 0.18;

    state.animal.adult.count = 8;
    state.animal.adult.condition_index = 0.0;
    state.animal.molt_stress_index = 1.0;
    state.process_params.shrimp_base_mortality_per_day = 0.3;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;

    state.microbe.decomposer_biomass_g = 0.15;
    state.microbe.ammonia_oxidizer_biomass_g = 0.1;
    state.microbe.nitrite_oxidizer_biomass_g = 0.08;
    state.microbe.comammox_biomass_g = 0.03;

    state.algae.periphyton_biomass_g = 1.0;
    state.algae.suspended_biomass_g = 0.4;
    state.process_params.algae_respiration_fraction_per_day = 0.25;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);

    state.hardware.light.enabled = false;
    state.hardware.light.photoperiod_hours = 0.0;
    state.hardware.light.intensity_index = 0.0;

    state.reseed_stability_tracker();
    state
}

/// Quiescent tank with plants, no active biology.
pub fn trim_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_4));
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state.hardware.light.enabled = false;
    state.plant_guilds = vec![PlantGuildState::default(), PlantGuildState::default()];
    state.plant_guilds[0].biomass_g = 2.0;
    state.plant_guilds[1].biomass_g = 3.5;

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

    state.reseed_stability_tracker();
    state
}

/// Isolated tank with known dissolved chemistry and a non-zero source profile.
/// All biology and gas exchange disabled so only the water change action
/// affects the budget.
pub fn water_change_fixture() -> TankState {
    let mut state = TankState::new(SimSeed(6_3_6_6));

    state.environment.ambient_temp_c = state.water.temperature_c;
    state.hardware.light.enabled = false;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
    state.hardware.filter.enabled = false;
    state.hardware.filter.flow_lph = 0.0;
    close_gas_exchange(&mut state);
    disable_dic_shortcuts(&mut state);
    state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.process_params.feed_leach_rate_per_hour = 0.0;

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
    for layer in &mut state.substrate_layers {
        layer.nutrient_store_mg_n_total = 0.0;
        layer.nutrient_store_mg_p_total = 0.0;
    }

    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrite_mg_n_total = 3.0;
    state.water.nitrate_mg_n_total = 18.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 240.0;
    state.water.dissolved_organic_carbon_mg_c_total = 16.0;

    let mut source = SourceWaterProfile::zero();
    source.temperature_c = 24.0;
    source.ammonia_mg_n_per_l = 0.5;
    source.nitrite_mg_n_per_l = 0.25;
    source.nitrate_mg_n_per_l = 5.0;
    source.don_mg_n_per_l = 0.5;
    source.dic_mg_c_per_l = 30.0;
    source.doc_mg_c_per_l = 4.0;
    source.alkalinity_meq_per_l = 2.5;
    state
        .source_water_catalog
        .insert("test_source".to_string(), source);

    state.reseed_stability_tracker();
    state
}
