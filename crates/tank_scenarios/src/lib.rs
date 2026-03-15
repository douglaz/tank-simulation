use std::collections::BTreeMap;

use tank_core::{
    PlantGuild, PlantGuildState, ProcessParams, SimMeta, SimSeed, SourceWaterProfile,
    SubstrateLayerState, TankGeometry, TankState, WaterState,
};
use tank_data::{load_scenario, ScenarioPreset};

pub fn default_scenario_ids() -> &'static [&'static str] {
    tank_data::scenario_ids()
}

pub fn load_named_scenario(id: &str) -> Result<ScenarioPreset, tank_data::PresetError> {
    load_scenario(id)
}

/// Converts a `tank_data::SourceWaterPreset` into a `tank_core::SourceWaterProfile`.
fn source_water_to_profile(preset: &tank_data::SourceWaterPreset) -> SourceWaterProfile {
    SourceWaterProfile {
        temperature_c: preset.temperature_c,
        ammonia_mg_n_per_l: preset.ammonia_mg_n_per_l,
        nitrite_mg_n_per_l: preset.nitrite_mg_n_per_l,
        nitrate_mg_n_per_l: preset.nitrate_mg_n_per_l,
        phosphate_mg_p_per_l: preset.phosphate_mg_p_per_l,
        dic_mg_c_per_l: preset.dic_mg_c_per_l,
        doc_mg_c_per_l: preset.doc_mg_c_per_l,
        don_mg_n_per_l: preset.don_mg_n_per_l,
        alkalinity_meq_per_l: preset.alkalinity_meq_per_l,
        calcium_mg_per_l: preset.calcium_mg_per_l,
        magnesium_mg_per_l: preset.magnesium_mg_per_l,
        sodium_mg_per_l: preset.sodium_mg_per_l,
        potassium_mg_per_l: preset.potassium_mg_per_l,
        bicarbonate_mg_per_l: preset.bicarbonate_mg_per_l,
        chloride_mg_per_l: preset.chloride_mg_per_l,
        sulfate_mg_per_l: preset.sulfate_mg_per_l,
    }
}

/// Converts a `tank_data::ProcessParamsPreset` into `tank_core::ProcessParams`.
fn process_preset_to_params(preset: &tank_data::ProcessParamsPreset) -> ProcessParams {
    ProcessParams {
        mineralization_rate_per_day: preset.mineralization_rate_per_day,
        nitrification_vmax: preset.nitrification_vmax,
        reaeration_kla: preset.reaeration_kla,
        k_surface_w_per_m2_k: preset.k_surface_w_per_m2_k,
        k_wall_w_per_m2_k: preset.k_wall_w_per_m2_k,
    }
}

/// Materializes a scenario preset into a fully resolved `TankState`.
///
/// - Geometry comes from the scenario.
/// - `environment.ambient_temp_c` comes from the scenario.
/// - Initial `WaterState` totals come from the source-water profile × fill volume.
/// - Substrate layers come from the scenario's substrate preset IDs.
/// - Plant guilds come from the scenario's plant preset IDs.
/// - Source-water catalog is populated with all known source-water presets.
/// - Process parameters come from the scenario's process params preset.
pub fn seeded_state(seed: SimSeed, scenario_id: &str) -> Result<TankState, tank_data::PresetError> {
    let scenario = load_named_scenario(scenario_id)?;

    // Geometry
    let geometry = TankGeometry {
        length_cm: scenario.tank_length_cm,
        width_cm: scenario.tank_width_cm,
        height_cm: scenario.tank_height_cm,
        fill_height_cm: scenario.fill_height_cm,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };

    // Source water profile for initial fill
    let source_water_preset = tank_data::load_source_water(&scenario.source_water_id)?;
    let source_profile = source_water_to_profile(&source_water_preset);

    // Initial water state from source-water profile
    let water = WaterState::from_source_profile(&source_profile, &geometry);

    // Build source-water catalog with all known presets
    let mut source_water_catalog = BTreeMap::new();
    for sw_id in tank_data::source_water_ids() {
        let sw_preset = tank_data::load_source_water(sw_id)?;
        source_water_catalog.insert(sw_id.to_string(), source_water_to_profile(&sw_preset));
    }

    // Process parameters
    let process_preset = tank_data::load_process_params(&scenario.process_params_id)?;
    let process_params = process_preset_to_params(&process_preset);

    // Substrate layers
    let mut substrate_layers = Vec::new();
    for sub_id in &scenario.substrate_ids {
        let sub_preset = tank_data::load_substrate(sub_id)?;
        let footprint = geometry.footprint_area_cm2();
        substrate_layers.push(SubstrateLayerState {
            kind: match sub_preset.id.as_str() {
                "inert_sand" => tank_core::SubstrateKind::InertSand,
                "inert_gravel" => tank_core::SubstrateKind::InertGravel,
                "active_planted" => tank_core::SubstrateKind::ActivePlanted,
                "coarse_porous" => tank_core::SubstrateKind::CoarsePorous,
                other => {
                    return Err(tank_data::PresetError::Validation {
                        category: "substrate",
                        id: other.to_string(),
                        message: format!("unsupported substrate kind: `{other}`"),
                    });
                }
            },
            depth_cm: sub_preset.depth_cm,
            nutrient_store_mg_n_total: sub_preset.nutrient_charge_mg_n_total,
            nutrient_store_mg_p_total: sub_preset.nutrient_charge_mg_p_total,
            cation_exchange_capacity_index: sub_preset.cation_exchange_capacity_index,
            detritus_trapping_index: sub_preset.detritus_trapping_index,
            colonizable_area_cm2: footprint * sub_preset.colonizable_area_factor,
            low_oxygen_tendency_index: sub_preset.low_oxygen_tendency_index,
            grazing_surface_index: sub_preset.grazing_surface_index,
        });
    }
    if substrate_layers.is_empty() {
        substrate_layers.push(SubstrateLayerState::default());
    }

    // Plant guilds
    let mut plant_guilds = Vec::new();
    for plant_id in &scenario.plant_ids {
        let plant_preset = tank_data::load_plant(plant_id)?;
        let guild = match plant_preset.guild.as_str() {
            "FastStem" => PlantGuild::FastStem,
            "RootFeedingRosette" => PlantGuild::RootFeedingRosette,
            other => {
                return Err(tank_data::PresetError::Validation {
                    category: "plants",
                    id: plant_id.clone(),
                    message: format!("unsupported plant guild: `{other}`"),
                });
            }
        };
        plant_guilds.push(PlantGuildState {
            guild,
            biomass_g: 5.0,
            health_index: 0.8,
            crowding_index: 0.1,
            habitat_index: if guild == PlantGuild::RootFeedingRosette {
                // Habitat factor depends on substrate quality
                substrate_layers
                    .iter()
                    .map(|l| l.cation_exchange_capacity_index)
                    .fold(0.0_f64, f64::max)
                    .max(0.3)
            } else {
                0.8
            },
        });
    }
    if plant_guilds.is_empty() {
        plant_guilds.push(PlantGuildState::default());
    }

    // Environment
    let mut environment = tank_core::EnvironmentState::default();
    environment.ambient_temp_c = scenario.ambient_temp_c;

    let mut state = TankState::new(seed);
    state.meta = SimMeta {
        scenario_id: Some(scenario.id),
        notes: Some(scenario.name),
    };
    state.geometry = geometry;
    state.environment = environment;
    state.water = water;
    state.substrate_layers = substrate_layers;
    state.plant_guilds = plant_guilds;
    state.source_water_catalog = source_water_catalog;
    state.process_params = process_params;

    Ok(state)
}
