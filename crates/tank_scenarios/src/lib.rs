use std::collections::BTreeMap;

use tank_core::{
    AnimalState, FilterState, MicrobeState, PlantGuild, PlantGuildState, ProcessParams,
    ShrimpRuntimeParams, SimMeta, SimSeed, SourceWaterProfile, SubstrateLayerState, TankGeometry,
    TankState, WaterState,
};
use tank_data::{load_scenario, ScenarioPreset, ShrimpPreset};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenarioGeometryOverrides {
    pub size_scale: f64,
    pub fill_ratio: f64,
}

impl Default for ScenarioGeometryOverrides {
    fn default() -> Self {
        Self {
            size_scale: 1.0,
            fill_ratio: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupSubstratePreset {
    InertSand,
    InertGravel,
    ActivePlanted,
    ActivePlantedWithCoarsePorous,
}

impl StartupSubstratePreset {
    pub const ALL: [Self; 4] = [
        Self::InertSand,
        Self::InertGravel,
        Self::ActivePlanted,
        Self::ActivePlantedWithCoarsePorous,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::InertSand => "inert_sand",
            Self::InertGravel => "inert_gravel",
            Self::ActivePlanted => "active_planted",
            Self::ActivePlantedWithCoarsePorous => "active_planted + coarse_porous",
        }
    }

    pub fn total_depth_cm(self) -> f64 {
        self.preset_ids()
            .iter()
            .map(|id| {
                tank_data::load_substrate(id)
                    .expect("startup substrate preset ids should resolve")
                    .depth_cm
                    .max(0.0)
            })
            .sum()
    }

    fn preset_ids(self) -> &'static [&'static str] {
        match self {
            Self::InertSand => &["inert_sand"],
            Self::InertGravel => &["inert_gravel"],
            Self::ActivePlanted => &["active_planted"],
            Self::ActivePlantedWithCoarsePorous => &["active_planted", "coarse_porous"],
        }
    }

    fn from_ids(ids: &[String]) -> Option<Self> {
        let ids = ids.iter().map(String::as_str).collect::<Vec<_>>();
        match ids.as_slice() {
            ["inert_sand"] => Some(Self::InertSand),
            ["inert_gravel"] => Some(Self::InertGravel),
            ["active_planted"] => Some(Self::ActivePlanted),
            ["active_planted", "coarse_porous"] => Some(Self::ActivePlantedWithCoarsePorous),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPlantSelection {
    FastStemOnly,
    RootFeedingRosetteOnly,
    BothGuilds,
    None,
}

impl StartupPlantSelection {
    pub const ALL: [Self; 4] = [
        Self::FastStemOnly,
        Self::RootFeedingRosetteOnly,
        Self::BothGuilds,
        Self::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FastStemOnly => "FastStem only",
            Self::RootFeedingRosetteOnly => "RootFeedingRosette only",
            Self::BothGuilds => "Both guilds",
            Self::None => "None",
        }
    }

    fn preset_ids(self) -> &'static [&'static str] {
        match self {
            Self::FastStemOnly => &["fast_stem"],
            Self::RootFeedingRosetteOnly => &["root_rosette"],
            Self::BothGuilds => &["fast_stem", "root_rosette"],
            Self::None => &[],
        }
    }

    fn from_ids(ids: &[String]) -> Option<Self> {
        let mut normalized = ids.iter().map(String::as_str).collect::<Vec<_>>();
        normalized.sort_unstable();
        match normalized.as_slice() {
            [] => Some(Self::None),
            ["fast_stem"] => Some(Self::FastStemOnly),
            ["root_rosette"] => Some(Self::RootFeedingRosetteOnly),
            ["fast_stem", "root_rosette"] => Some(Self::BothGuilds),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupLightPreset {
    Hours6,
    Hours8,
    Hours10,
    Hours12,
}

impl StartupLightPreset {
    pub const ALL: [Self; 4] = [Self::Hours6, Self::Hours8, Self::Hours10, Self::Hours12];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hours6 => "6h",
            Self::Hours8 => "8h",
            Self::Hours10 => "10h",
            Self::Hours12 => "12h",
        }
    }

    pub fn hours(self) -> f64 {
        match self {
            Self::Hours6 => 6.0,
            Self::Hours8 => 8.0,
            Self::Hours10 => 10.0,
            Self::Hours12 => 12.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupHeaterPreset {
    Off,
    Celsius24,
    Celsius25,
    Celsius26,
}

impl StartupHeaterPreset {
    pub const ALL: [Self; 4] = [Self::Off, Self::Celsius24, Self::Celsius25, Self::Celsius26];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Celsius24 => "24 C",
            Self::Celsius25 => "25 C",
            Self::Celsius26 => "26 C",
        }
    }

    pub fn setpoint_c(self) -> Option<f64> {
        match self {
            Self::Off => None,
            Self::Celsius24 => Some(24.0),
            Self::Celsius25 => Some(25.0),
            Self::Celsius26 => Some(26.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StartupOverrides {
    pub geometry: ScenarioGeometryOverrides,
    pub source_water_profile_id: Option<String>,
    pub substrate_preset: Option<StartupSubstratePreset>,
    pub plant_selection: Option<StartupPlantSelection>,
    pub filter_enabled: Option<bool>,
    pub light_preset: Option<StartupLightPreset>,
    pub heater_preset: Option<StartupHeaterPreset>,
    pub aeration_enabled: Option<bool>,
    pub initial_adult_shrimp_count: Option<u32>,
}

pub fn default_scenario_ids() -> &'static [&'static str] {
    tank_data::scenario_ids()
}

pub fn startup_source_water_ids() -> &'static [&'static str] {
    tank_data::source_water_ids()
}

pub fn load_named_scenario(id: &str) -> Result<ScenarioPreset, tank_data::PresetError> {
    load_scenario(id)
}

pub fn seeded_state_with_overrides(
    seed: SimSeed,
    scenario_id: &str,
    overrides: ScenarioGeometryOverrides,
) -> Result<TankState, tank_data::PresetError> {
    seeded_state_with_full_overrides(
        seed,
        scenario_id,
        StartupOverrides {
            geometry: overrides,
            ..StartupOverrides::default()
        },
    )
}

pub fn seeded_state_with_full_overrides(
    seed: SimSeed,
    scenario_id: &str,
    overrides: StartupOverrides,
) -> Result<TankState, tank_data::PresetError> {
    validate_geometry_overrides(scenario_id, overrides.geometry)?;
    let mut scenario = load_named_scenario(scenario_id)?;
    let scenario_source_water_id = scenario.source_water_id.clone();
    scenario.tank_length_cm *= overrides.geometry.size_scale;
    scenario.tank_width_cm *= overrides.geometry.size_scale;
    scenario.tank_height_cm *= overrides.geometry.size_scale;
    scenario.fill_height_cm =
        (scenario.fill_height_cm * overrides.geometry.size_scale * overrides.geometry.fill_ratio)
            .min(scenario.tank_height_cm);

    // Scale substrate nutrient stores proportionally to the footprint change.
    // Preset charges are absolute totals designed for size_scale=1.0; a larger
    // footprint should carry proportionally more nutrients.
    let area_scale = overrides.geometry.size_scale * overrides.geometry.size_scale;

    let mut state = materialize_scenario(seed, scenario)?;
    apply_startup_overrides(
        &mut state,
        scenario_id,
        &scenario_source_water_id,
        overrides,
    )?;
    if (area_scale - 1.0).abs() > f64::EPSILON {
        for layer in &mut state.substrate_layers {
            layer.nutrient_store_mg_n_total *= area_scale;
            layer.nutrient_store_mg_p_total *= area_scale;
        }
    }

    Ok(state)
}

pub fn startup_defaults_for_scenario(
    scenario_id: &str,
) -> Result<StartupOverrides, tank_data::PresetError> {
    let scenario = load_named_scenario(scenario_id)?;
    let substrate_preset =
        StartupSubstratePreset::from_ids(&scenario.substrate_ids).ok_or_else(|| {
            tank_data::PresetError::Validation {
                category: "scenarios",
                id: scenario_id.to_string(),
                message: format!(
                    "startup template does not support substrate combination {:?}",
                    scenario.substrate_ids
                ),
            }
        })?;
    let plant_selection =
        StartupPlantSelection::from_ids(&scenario.plant_ids).ok_or_else(|| {
            tank_data::PresetError::Validation {
                category: "scenarios",
                id: scenario_id.to_string(),
                message: format!(
                    "startup template does not support plant combination {:?}",
                    scenario.plant_ids
                ),
            }
        })?;

    let (light_preset, heater_preset, aeration_enabled, initial_adult_shrimp_count) =
        match scenario_id {
            "medium_planted" => (
                StartupLightPreset::Hours12,
                StartupHeaterPreset::Celsius25,
                false,
                10,
            ),
            "warm_room" => (
                StartupLightPreset::Hours8,
                StartupHeaterPreset::Off,
                true,
                5,
            ),
            _ => (
                StartupLightPreset::Hours6,
                StartupHeaterPreset::Off,
                false,
                0,
            ),
        };

    Ok(StartupOverrides {
        geometry: ScenarioGeometryOverrides::default(),
        source_water_profile_id: Some(scenario.source_water_id),
        substrate_preset: Some(substrate_preset),
        plant_selection: Some(plant_selection),
        filter_enabled: Some(true),
        light_preset: Some(light_preset),
        heater_preset: Some(heater_preset),
        aeration_enabled: Some(aeration_enabled),
        initial_adult_shrimp_count: Some(initial_adult_shrimp_count),
    })
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
    let defaults = ProcessParams::default();
    ProcessParams {
        mineralization_rate_per_day: preset.mineralization_rate_per_day,
        nitrification_vmax: preset.nitrification_vmax,
        reaeration_kla_base: preset.reaeration_kla_base,
        aeration_kla_boost: preset.aeration_kla_boost,
        background_bod_mg_o2_per_g_biomass_per_hour: preset
            .background_bod_mg_o2_per_g_biomass_per_hour,
        plant_photosynthesis_o2_mg_per_g_per_hour: preset.plant_photosynthesis_o2_mg_per_g_per_hour,
        respiration_dic_rate_mg_c_per_g_per_hour: preset.respiration_dic_rate_mg_c_per_g_per_hour,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: preset
            .photosynthesis_dic_rate_mg_c_per_g_per_hour,
        k_surface_w_per_m2_k: preset.k_surface_w_per_m2_k,
        k_wall_w_per_m2_k: preset.k_wall_w_per_m2_k,

        feed_leach_rate_per_hour: preset.feed_leach_rate_per_hour,
        fine_detritus_dissolution_rate_per_hour: preset.fine_detritus_dissolution_rate_per_hour,
        feed_n_to_c_ratio: preset.feed_n_to_c_ratio,

        decomposer_vmax_per_hour: preset.decomposer_vmax_per_hour,
        decomposer_k_doc_mg_c_per_l: preset.decomposer_k_doc_mg_c_per_l,
        decomposer_k_do_mg_per_l: preset.decomposer_k_do_mg_per_l,
        decomposer_growth_yield: preset.decomposer_growth_yield,
        decomposer_decay_rate_per_hour: preset.decomposer_decay_rate_per_hour,

        aob_vmax_mg_n_per_g_per_hour: preset.aob_vmax_mg_n_per_g_per_hour,
        aob_k_tan_mg_n_per_l: preset.aob_k_tan_mg_n_per_l,
        aob_k_do_mg_per_l: preset.aob_k_do_mg_per_l,
        aob_growth_yield: preset.aob_growth_yield,
        aob_decay_rate_per_hour: preset.aob_decay_rate_per_hour,

        nob_vmax_mg_n_per_g_per_hour: preset.nob_vmax_mg_n_per_g_per_hour,
        nob_k_nitrite_mg_n_per_l: preset.nob_k_nitrite_mg_n_per_l,
        nob_k_do_mg_per_l: preset.nob_k_do_mg_per_l,
        nob_growth_yield: preset.nob_growth_yield,
        nob_decay_rate_per_hour: preset.nob_decay_rate_per_hour,

        comammox_vmax_fraction: preset.comammox_vmax_fraction,
        comammox_k_tan_mg_n_per_l: preset.comammox_k_tan_mg_n_per_l,
        comammox_k_do_mg_per_l: preset.comammox_k_do_mg_per_l,
        comammox_growth_yield: preset.comammox_growth_yield,
        comammox_decay_rate_per_hour: preset.comammox_decay_rate_per_hour,

        nitrifier_base_density_g_per_cm2: defaults.nitrifier_base_density_g_per_cm2,

        o2_per_mg_n_nitrified: preset.o2_per_mg_n_nitrified,
        alkalinity_meq_per_mg_n_nitrified: preset.alkalinity_meq_per_mg_n_nitrified,

        plant_max_growth_rate_fast_stem_per_day: preset.plant_max_growth_rate_fast_stem_per_day,
        plant_max_growth_rate_root_rosette_per_day: preset
            .plant_max_growth_rate_root_rosette_per_day,
        plant_respiration_fraction_per_day: preset.plant_respiration_fraction_per_day,
        plant_senescence_fraction_per_day: preset.plant_senescence_fraction_per_day,
        plant_health_recovery_per_day: preset.plant_health_recovery_per_day,
        plant_health_decline_per_day: preset.plant_health_decline_per_day,
        plant_half_saturation_n_mg_n_per_l: preset.plant_half_saturation_n_mg_n_per_l,
        plant_half_saturation_p_mg_p_per_l: preset.plant_half_saturation_p_mg_p_per_l,
        plant_half_saturation_c_mg_c_per_l: preset.plant_half_saturation_c_mg_c_per_l,
        plant_half_saturation_n_substrate_mg_n_per_m2: preset
            .plant_half_saturation_n_substrate_mg_n_per_m2,
        plant_half_saturation_p_substrate_mg_p_per_m2: preset
            .plant_half_saturation_p_substrate_mg_p_per_m2,
        plant_light_half_saturation: preset.plant_light_half_saturation,
        plant_temp_optimum_c: preset.plant_temp_optimum_c,
        plant_temp_sigma_c: preset.plant_temp_sigma_c,
        plant_crowding_biomass_g_per_m2: preset.plant_crowding_biomass_g_per_m2,

        algae_max_growth_rate_per_day: preset.algae_max_growth_rate_per_day,
        periphyton_max_growth_rate_per_day: preset.periphyton_max_growth_rate_per_day,
        algae_respiration_fraction_per_day: preset.algae_respiration_fraction_per_day,
        algae_half_saturation_n_mg_n_per_l: preset.algae_half_saturation_n_mg_n_per_l,
        algae_half_saturation_p_mg_p_per_l: preset.algae_half_saturation_p_mg_p_per_l,
        algae_light_half_saturation: preset.algae_light_half_saturation,
        algae_temp_optimum_c: preset.algae_temp_optimum_c,
        algae_temp_sigma_c: preset.algae_temp_sigma_c,
        periphyton_capacity_g_per_m2: preset.periphyton_capacity_g_per_m2,
        algae_bloom_threshold_g_per_l: preset.algae_bloom_threshold_g_per_l,
        algae_nuisance_biomass_g_per_m2: preset.algae_nuisance_biomass_g_per_m2,
        base_extinction_coeff_per_cm: defaults.base_extinction_coeff_per_cm,
        algae_extinction_coeff_per_cm_per_g_l: defaults.algae_extinction_coeff_per_cm_per_g_l,
        doc_extinction_coeff_per_cm_per_mg_c_l: defaults.doc_extinction_coeff_per_cm_per_mg_c_l,
        detritus_extinction_coeff_per_cm_per_g_l: defaults.detritus_extinction_coeff_per_cm_per_g_l,

        shrimp_base_mortality_per_day: preset.shrimp_base_mortality_per_day,
        shrimp_stress_mortality_scale: preset.shrimp_stress_mortality_scale,
        shrimp_juvenile_maturation_days: preset.shrimp_juvenile_maturation_days,
        shrimp_periphyton_grazing_g_per_shrimp_per_day: preset
            .shrimp_periphyton_grazing_g_per_shrimp_per_day,
        shrimp_condition_smoothing: preset.shrimp_condition_smoothing,
        shrimp_assimilation_efficiency: preset.shrimp_assimilation_efficiency,
        shrimp_respiration_fraction_of_assimilated: preset
            .shrimp_respiration_fraction_of_assimilated,
        shrimp_excretion_fraction_of_assimilated: preset.shrimp_excretion_fraction_of_assimilated,
        shrimp_growth_fraction_of_assimilated: preset.shrimp_growth_fraction_of_assimilated,
        shrimp_o2_per_mg_c_respired: preset.shrimp_o2_per_mg_c_respired,

        death_biomass_to_detritus_fraction: preset.death_biomass_to_detritus_fraction,

        microfauna_mineralization_boost: preset.microfauna_mineralization_boost,
        microfauna_periphyton_consumption: preset.microfauna_periphyton_consumption,
        microfauna_population_smoothing: preset.microfauna_population_smoothing,
        microfauna_shrimp_pressure_threshold: preset.microfauna_shrimp_pressure_threshold,

        microfauna_assimilation_efficiency: preset.microfauna_assimilation_efficiency,
        microfauna_respiration_fraction_of_assimilated: preset
            .microfauna_respiration_fraction_of_assimilated,
        microfauna_excretion_fraction_of_assimilated: preset
            .microfauna_excretion_fraction_of_assimilated,
        microfauna_growth_fraction_of_assimilated: preset.microfauna_growth_fraction_of_assimilated,
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
    materialize_scenario(seed, scenario)
}

fn materialize_scenario(
    seed: SimSeed,
    scenario: ScenarioPreset,
) -> Result<TankState, tank_data::PresetError> {
    // Geometry
    let geometry = TankGeometry {
        length_cm: scenario.tank_length_cm,
        width_cm: scenario.tank_width_cm,
        height_cm: scenario.tank_height_cm,
        fill_height_cm: scenario.fill_height_cm,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };

    // Substrate layers
    let substrate_layers = build_substrate_layers(&geometry, &scenario.substrate_ids)?;

    // Source water profile for initial fill
    let source_water_preset = tank_data::load_source_water(&scenario.source_water_id)?;
    let source_profile = source_water_to_profile(&source_water_preset);

    // Initial water state from source-water profile
    let water = WaterState::from_source_profile_for_volume_l(
        &source_profile,
        geometry.water_volume_l_with_substrate_depth(
            substrate_layers
                .iter()
                .map(|layer| layer.depth_cm.max(0.0))
                .sum(),
        ),
    );

    // Build source-water catalog with all known presets
    let mut source_water_catalog = BTreeMap::new();
    for sw_id in tank_data::source_water_ids() {
        let sw_preset = tank_data::load_source_water(sw_id)?;
        source_water_catalog.insert(sw_id.to_string(), source_water_to_profile(&sw_preset));
    }

    // Process parameters
    let process_preset = tank_data::load_process_params(&scenario.process_params_id)?;
    let process_params = process_preset_to_params(&process_preset);

    // Plant guilds
    let plant_guilds = build_plant_guilds(&scenario.plant_ids, &substrate_layers, true)?;

    // Shrimp species parameters
    let shrimp_preset = tank_data::load_shrimp(&scenario.shrimp_profile_id)?;
    let shrimp_params = shrimp_preset_to_params(
        &shrimp_preset,
        process_params.shrimp_juvenile_maturation_days,
    );

    // Environment
    let environment = tank_core::EnvironmentState {
        ambient_temp_c: scenario.ambient_temp_c,
        ..Default::default()
    };

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
    state.shrimp_params = shrimp_params;
    apply_named_scenario_tuning(&mut state);

    // Seed stability tracker baselines from actual water state to prevent
    // false chemistry-swing detection on the first update.
    state.reseed_stability_tracker();
    state.refresh_habitat_registry();

    Ok(state)
}

fn shrimp_preset_to_params(
    shrimp_preset: &ShrimpPreset,
    legacy_total_maturation_days: f64,
) -> ShrimpRuntimeParams {
    let mut params = ShrimpRuntimeParams {
        optimal_temp_min_c: shrimp_preset.optimal_temp_min_c,
        optimal_temp_max_c: shrimp_preset.optimal_temp_max_c,
        gh_min_d: shrimp_preset.gh_min_d,
        gh_max_d: shrimp_preset.gh_max_d,
        base_spawn_rate: shrimp_preset.base_spawn_rate,
        egg_duration_days: shrimp_preset.egg_duration_days,
        hatch_success_base: shrimp_preset.hatch_success_base,
        juvenile_sensitivity: shrimp_preset.juvenile_sensitivity,
        high_temp_repro_penalty_start_c: shrimp_preset.high_temp_repro_penalty_start_c,
        high_temp_repro_penalty_full_c: shrimp_preset.high_temp_repro_penalty_full_c,
        ..ShrimpRuntimeParams::default()
    };
    params.apply_legacy_total_maturation_days(legacy_total_maturation_days);

    if let Some(value) = shrimp_preset.body_nitrogen_mg_per_g_wet_mass {
        params.body_nitrogen_mg_per_g_wet_mass = value;
    }
    if let Some(value) = shrimp_preset.body_carbon_mg_per_g_wet_mass {
        params.body_carbon_mg_per_g_wet_mass = value;
    }
    if let Some(value) = shrimp_preset.juvenile_to_subadult_days {
        params.juvenile_to_subadult_days = value;
    }
    if let Some(value) = shrimp_preset.subadult_to_adult_days {
        params.subadult_to_adult_days = value;
    }
    if let Some(value) = shrimp_preset.juvenile_maturation_condition_threshold {
        params.juvenile_maturation_condition_threshold = value;
    }
    if let Some(value) = shrimp_preset.subadult_maturation_condition_threshold {
        params.subadult_maturation_condition_threshold = value;
    }
    if let Some(value) = shrimp_preset.base_molt_interval_days {
        params.base_molt_interval_days = value;
    }
    if let Some(value) = shrimp_preset.failed_molt_mortality_scale {
        params.failed_molt_mortality_scale = value;
    }
    if let Some(value) = shrimp_preset.sub_adult_sensitivity {
        params.sub_adult_sensitivity = value;
    }
    if let Some(value) = shrimp_preset.base_clutch_size {
        params.base_clutch_size = value;
    }
    if let Some(value) = shrimp_preset.min_clutch_condition {
        params.min_clutch_condition = value;
    }

    params
}

fn apply_startup_overrides(
    state: &mut TankState,
    scenario_id: &str,
    scenario_source_water_id: &str,
    overrides: StartupOverrides,
) -> Result<(), tank_data::PresetError> {
    let StartupOverrides {
        geometry: _,
        source_water_profile_id,
        substrate_preset,
        plant_selection,
        filter_enabled,
        light_preset,
        heater_preset,
        aeration_enabled,
        initial_adult_shrimp_count,
    } = overrides;

    let effective_source_profile =
        if source_water_profile_id.is_some() || substrate_preset.is_some() {
            let source_water_id = source_water_profile_id
                .as_deref()
                .unwrap_or(scenario_source_water_id);
            Some(resolve_source_profile(state, source_water_id)?)
        } else {
            None
        };

    if let Some(substrate_preset) = substrate_preset {
        let substrate_ids = substrate_preset
            .preset_ids()
            .iter()
            .map(|id| (*id).to_string())
            .collect::<Vec<_>>();
        state.substrate_layers = build_substrate_layers(&state.geometry, &substrate_ids)?;
    }

    if substrate_preset.is_some() || plant_selection.is_some() {
        let plant_ids = if let Some(plant_selection) = plant_selection {
            plant_selection
                .preset_ids()
                .iter()
                .map(|id| (*id).to_string())
                .collect::<Vec<_>>()
        } else {
            state
                .plant_guilds
                .iter()
                .map(|plant| match plant.guild {
                    PlantGuild::FastStem => "fast_stem".to_string(),
                    PlantGuild::RootFeedingRosette => "root_rosette".to_string(),
                })
                .collect::<Vec<_>>()
        };
        state.plant_guilds = build_plant_guilds(&plant_ids, &state.substrate_layers, false)?;
    }

    if let Some(profile) = effective_source_profile {
        state.water =
            WaterState::from_source_profile_for_volume_l(&profile, state.water_volume_l());
    }

    if let Some(filter_enabled) = filter_enabled {
        state.hardware.filter.enabled = filter_enabled;
        if filter_enabled {
            if state.hardware.filter.flow_lph <= 0.0 {
                state.hardware.filter.flow_lph = 200.0;
            }
        } else {
            state.hardware.filter.flow_lph = 0.0;
        }
    }

    if let Some(light_preset) = light_preset {
        state.hardware.light.enabled = true;
        state.hardware.light.photoperiod_hours = light_preset.hours();
    }

    if let Some(heater_preset) = heater_preset {
        match heater_preset.setpoint_c() {
            Some(setpoint_c) => {
                state.hardware.heater.enabled = true;
                state.hardware.heater.setpoint_c = setpoint_c;
            }
            None => {
                state.hardware.heater.enabled = false;
                state.hardware.heater.setpoint_c = 24.0;
            }
        }
        state.hardware.heater.last_output_w = 0.0;
    }

    if let Some(aeration_enabled) = aeration_enabled {
        state.hardware.aeration.enabled = aeration_enabled;
        state.hardware.aeration.intensity = if aeration_enabled { 0.35 } else { 0.0 };
    }

    if let Some(initial_adult_shrimp_count) = initial_adult_shrimp_count {
        state.animal = AnimalState::with_adults(initial_adult_shrimp_count);
    }

    if state.meta.scenario_id.as_deref() == Some(scenario_id) {
        apply_named_scenario_tuning(state);
    }

    // Reseed stability baselines so that overridden water chemistry is not
    // treated as a "swing" on the first daily update.
    state.reseed_stability_tracker();
    state.refresh_habitat_registry();

    Ok(())
}

fn apply_named_scenario_tuning(state: &mut TankState) {
    if state.meta.scenario_id.as_deref() == Some("medium_planted") {
        // Keep the shipped planted regression visibly active after the
        // hourly-DIC carbon cap by starting each selected guild at 6 g.
        for plant in &mut state.plant_guilds {
            plant.biomass_g = plant.biomass_g.max(6.0);
        }
    }
}

fn resolve_source_profile(
    state: &mut TankState,
    source_water_id: &str,
) -> Result<SourceWaterProfile, tank_data::PresetError> {
    if let Some(existing) = state.source_water_catalog.get(source_water_id) {
        Ok(existing.clone())
    } else {
        let preset = tank_data::load_source_water(source_water_id)?;
        let profile = source_water_to_profile(&preset);
        state
            .source_water_catalog
            .insert(source_water_id.to_string(), profile.clone());
        Ok(profile)
    }
}

fn build_substrate_layers(
    geometry: &TankGeometry,
    substrate_ids: &[String],
) -> Result<Vec<SubstrateLayerState>, tank_data::PresetError> {
    let mut substrate_layers = Vec::new();
    let footprint = geometry.footprint_area_cm2();
    for sub_id in substrate_ids {
        let sub_preset = tank_data::load_substrate(sub_id)?;
        let kind = match sub_preset.id.as_str() {
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
        };
        substrate_layers.push(SubstrateLayerState {
            kind,
            depth_cm: sub_preset.depth_cm,
            nutrient_store_mg_n_total: sub_preset.nutrient_charge_mg_n_total,
            nutrient_store_mg_p_total: sub_preset.nutrient_charge_mg_p_total,
            cation_exchange_capacity_index: sub_preset.cation_exchange_capacity_index,
            detritus_trapping_index: sub_preset.detritus_trapping_index,
            colonizable_area_factor: sub_preset.colonizable_area_factor,
            colonizable_area_cm2: SubstrateLayerState {
                kind,
                depth_cm: sub_preset.depth_cm,
                colonizable_area_factor: sub_preset.colonizable_area_factor,
                ..SubstrateLayerState::default()
            }
            .derived_colonizable_area_cm2(footprint),
            low_oxygen_tendency_index: sub_preset.low_oxygen_tendency_index,
            grazing_surface_index: sub_preset.grazing_surface_index,
        });
    }
    if substrate_layers.is_empty() {
        let default_substrate = SubstrateLayerState::default();
        substrate_layers.push(SubstrateLayerState {
            colonizable_area_cm2: default_substrate.derived_colonizable_area_cm2(footprint),
            ..default_substrate
        });
    }
    Ok(substrate_layers)
}

fn build_plant_guilds(
    plant_ids: &[String],
    substrate_layers: &[SubstrateLayerState],
    include_default_when_empty: bool,
) -> Result<Vec<PlantGuildState>, tank_data::PresetError> {
    let mut plant_guilds = Vec::new();
    for plant_id in plant_ids {
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
                root_rosette_habitat_index(substrate_layers)
            } else {
                0.8
            },
            water_column_uptake_bias: Some(plant_preset.water_column_uptake_bias),
            substrate_uptake_bias: Some(plant_preset.substrate_uptake_bias),
        });
    }
    if plant_guilds.is_empty() && include_default_when_empty {
        plant_guilds.push(PlantGuildState::default());
    }
    Ok(plant_guilds)
}

fn root_rosette_habitat_index(substrate_layers: &[SubstrateLayerState]) -> f64 {
    substrate_layers
        .iter()
        .map(|layer| layer.cation_exchange_capacity_index)
        .fold(0.0_f64, f64::max)
        .max(0.3)
}

fn validate_geometry_overrides(
    scenario_id: &str,
    overrides: ScenarioGeometryOverrides,
) -> Result<(), tank_data::PresetError> {
    if !overrides.size_scale.is_finite() || overrides.size_scale <= 0.0 {
        return Err(tank_data::PresetError::Validation {
            category: "scenarios",
            id: scenario_id.to_string(),
            message: format!(
                "size_scale must be finite and > 0.0, got {}",
                overrides.size_scale
            ),
        });
    }

    if !overrides.fill_ratio.is_finite() || !(0.1..=1.0).contains(&overrides.fill_ratio) {
        return Err(tank_data::PresetError::Validation {
            category: "scenarios",
            id: scenario_id.to_string(),
            message: format!(
                "fill_ratio must be finite and in 0.1..=1.0, got {}",
                overrides.fill_ratio
            ),
        });
    }

    Ok(())
}

/// Creates a deterministic cycling fixture pair: one seeded, one unseeded.
/// Both tanks are otherwise identical (~20L nano tank with moderate source water,
/// fed daily, with aeration enabled).
/// The seeded tank starts with elevated nitrifier biomass and higher maturity;
/// the unseeded tank starts with minimal microbes and low maturity.
pub fn cycling_fixture_pair(seed: SimSeed) -> (TankState, TankState) {
    let base = cycling_base_state(seed);

    let mut seeded = base.clone();
    seeded.meta.scenario_id = Some("cycling_seeded".to_string());
    // Seeded: same decomposer biomass but much higher nitrifier biomass + maturity
    seeded.microbe = MicrobeState {
        decomposer_biomass_g: 0.1,
        ammonia_oxidizer_biomass_g: 0.20,
        nitrite_oxidizer_biomass_g: 0.15,
        comammox_biomass_g: 0.05,
        maturity_index: 0.6,
    };
    seeded.filter_state = FilterState {
        biofilter_maturity_index: 0.6,
        clogging_index: 0.0,
        seeded_biomass_index: 0.8,
    };

    let mut unseeded = base;
    unseeded.meta.scenario_id = Some("cycling_unseeded".to_string());
    // Unseeded: same decomposer biomass but minimal nitrifier biomass + maturity
    unseeded.microbe = MicrobeState {
        decomposer_biomass_g: 0.1,
        ammonia_oxidizer_biomass_g: 0.005,
        nitrite_oxidizer_biomass_g: 0.005,
        comammox_biomass_g: 0.001,
        maturity_index: 0.05,
    };
    unseeded.filter_state = FilterState {
        biofilter_maturity_index: 0.05,
        clogging_index: 0.0,
        seeded_biomass_index: 0.0,
    };
    seeded.refresh_habitat_registry();
    unseeded.refresh_habitat_registry();

    (seeded, unseeded)
}

/// Base state shared by both cycling fixtures.
fn cycling_base_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
        length_cm: 30.0,
        width_cm: 25.0,
        height_cm: 30.0,
        fill_height_cm: 26.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };

    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 25.0;
    state.environment.ambient_temp_c = 25.0;

    // Enable moderate aeration
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;

    // Single fast-stem plant
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 3.0,
        health_index: 0.8,
        crowding_index: 0.1,
        habitat_index: 0.8,
        water_column_uptake_bias: Some(0.9),
        substrate_uptake_bias: Some(0.2),
    }];

    state.process_params = ProcessParams::default();

    // Re-seed stability baseline after replacing geometry/water above.
    let volume_l = state.water_volume_l();
    state
        .stability_tracker
        .seed_from_water(&state.water, volume_l);
    state.refresh_habitat_registry();

    state
}

#[cfg(test)]
mod tests {
    use super::{process_preset_to_params, shrimp_preset_to_params, source_water_to_profile};
    use tank_core::{ProcessParams, WaterState, NITRIFICATION_ALK_MEQ_PER_MG_N};

    #[test]
    fn process_preset_mapping_carries_shrimp_routing_fields() {
        let mut preset =
            tank_data::load_process_params("default").expect("default process preset should load");
        preset.shrimp_assimilation_efficiency = 0.61;
        preset.shrimp_respiration_fraction_of_assimilated = 0.62;
        preset.shrimp_excretion_fraction_of_assimilated = 0.13;
        preset.shrimp_growth_fraction_of_assimilated = 0.25;
        preset.shrimp_o2_per_mg_c_respired = 2.91;

        let params = process_preset_to_params(&preset);

        assert_eq!(params.shrimp_assimilation_efficiency, 0.61);
        assert_eq!(params.shrimp_respiration_fraction_of_assimilated, 0.62);
        assert_eq!(params.shrimp_excretion_fraction_of_assimilated, 0.13);
        assert_eq!(params.shrimp_growth_fraction_of_assimilated, 0.25);
        assert_eq!(params.shrimp_o2_per_mg_c_respired, 2.91);
    }

    #[test]
    fn process_preset_mapping_carries_microfauna_and_death_routing_fields() {
        let mut preset =
            tank_data::load_process_params("default").expect("default process preset should load");
        preset.death_biomass_to_detritus_fraction = 1.0;
        preset.microfauna_assimilation_efficiency = 0.57;
        preset.microfauna_respiration_fraction_of_assimilated = 0.64;
        preset.microfauna_excretion_fraction_of_assimilated = 0.11;
        preset.microfauna_growth_fraction_of_assimilated = 0.25;

        let params = process_preset_to_params(&preset);

        assert_eq!(params.death_biomass_to_detritus_fraction, 1.0);
        assert_eq!(params.microfauna_assimilation_efficiency, 0.57);
        assert_eq!(params.microfauna_respiration_fraction_of_assimilated, 0.64);
        assert_eq!(params.microfauna_excretion_fraction_of_assimilated, 0.11);
        assert_eq!(params.microfauna_growth_fraction_of_assimilated, 0.25);
    }

    #[test]
    fn default_process_preset_matches_runtime_nitrification_alkalinity_constant() {
        let preset =
            tank_data::load_process_params("default").expect("default process preset should load");
        let params = process_preset_to_params(&preset);

        assert_eq!(
            preset.alkalinity_meq_per_mg_n_nitrified, NITRIFICATION_ALK_MEQ_PER_MG_N,
            "the shipped process preset should stay aligned with the named stoichiometric constant"
        );
        assert_eq!(
            params.alkalinity_meq_per_mg_n_nitrified,
            ProcessParams::default().alkalinity_meq_per_mg_n_nitrified,
            "preset-driven scenarios should match the runtime ProcessParams default"
        );
    }

    #[test]
    fn shrimp_preset_mapping_carries_stage_runtime_fields_and_legacy_split() {
        let mut preset = tank_data::load_shrimp("neocaridina_davidi")
            .expect("default shrimp preset should load");
        preset.body_nitrogen_mg_per_g_wet_mass = Some(31.0);
        preset.body_carbon_mg_per_g_wet_mass = Some(165.0);
        preset.juvenile_to_subadult_days = None;
        preset.subadult_to_adult_days = None;
        preset.juvenile_maturation_condition_threshold = Some(0.22);
        preset.subadult_maturation_condition_threshold = Some(0.41);
        preset.base_molt_interval_days = Some(19.0);
        preset.failed_molt_mortality_scale = Some(0.27);
        preset.sub_adult_sensitivity = Some(1.9);
        preset.base_clutch_size = Some(17);
        preset.min_clutch_condition = Some(0.44);

        let params = shrimp_preset_to_params(&preset, 40.0);
        assert_eq!(params.body_nitrogen_mg_per_g_wet_mass, 31.0);
        assert_eq!(params.body_carbon_mg_per_g_wet_mass, 165.0);
        assert!((params.juvenile_to_subadult_days - 24.0).abs() < 1e-9);
        assert!((params.subadult_to_adult_days - 16.0).abs() < 1e-9);
        assert_eq!(params.juvenile_maturation_condition_threshold, 0.22);
        assert_eq!(params.subadult_maturation_condition_threshold, 0.41);
        assert_eq!(params.base_molt_interval_days, 19.0);
        assert_eq!(params.failed_molt_mortality_scale, 0.27);
        assert_eq!(params.sub_adult_sensitivity, 1.9);
        assert_eq!(params.base_clutch_size, 17);
        assert_eq!(params.min_clutch_condition, 0.44);

        preset.juvenile_to_subadult_days = Some(15.0);
        preset.subadult_to_adult_days = Some(9.0);
        let overridden = shrimp_preset_to_params(&preset, 40.0);
        assert_eq!(overridden.juvenile_to_subadult_days, 15.0);
        assert_eq!(overridden.subadult_to_adult_days, 9.0);
    }

    #[test]
    fn shipped_source_water_profiles_materialize_distinct_initial_ph() {
        let volume_l = 20.0;
        let soft = source_water_to_profile(
            &tank_data::load_source_water("soft_acidic").expect("soft preset should load"),
        );
        let moderate = source_water_to_profile(
            &tank_data::load_source_water("moderate").expect("moderate preset should load"),
        );
        let hard = source_water_to_profile(
            &tank_data::load_source_water("hard_shrimp").expect("hard preset should load"),
        );
        let ro = source_water_to_profile(
            &tank_data::load_source_water("ro_like").expect("ro preset should load"),
        );

        let soft_water = WaterState::from_source_profile_for_volume_l(&soft, volume_l);
        let moderate_water = WaterState::from_source_profile_for_volume_l(&moderate, volume_l);
        let hard_water = WaterState::from_source_profile_for_volume_l(&hard, volume_l);
        let ro_water = WaterState::from_source_profile_for_volume_l(&ro, volume_l);

        assert!(soft_water.ph < moderate_water.ph);
        assert!(moderate_water.ph < hard_water.ph);
        assert!(
            hard_water.ph - soft_water.ph >= 0.3,
            "expected at least 0.3 pH spread, got soft={:.3}, hard={:.3}",
            soft_water.ph,
            hard_water.ph
        );
        assert!(ro_water.ph < soft_water.ph);
        assert!((5.5..=8.5).contains(&ro_water.ph));
    }
}
