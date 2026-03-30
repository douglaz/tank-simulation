use std::collections::BTreeMap;

use tank_core::{
    AnimalState, FilterState, MicrobeState, PlantGuild, PlantGuildState, ProcessParams,
    ShrimpRuntimeParams, SimMeta, SimSeed, SourceWaterProfile, SubstrateLayerState, TankGeometry,
    TankState, WaterState,
};
use tank_data::{load_scenario, ScenarioPreset, ShrimpPreset};

// ---------------------------------------------------------------------------
// Geometry-aware scaling constants
// ---------------------------------------------------------------------------
// These named constants define how hardware and biomass defaults scale with
// tank geometry. Each formula is documented inline. Scenario-authored overrides
// always win over auto-scaled values.

/// Filter flow rate: 10 turnovers of total volume per hour.
/// Standard aquarium guideline is 5–10×; we use the upper end for adequate
/// circulation in planted shrimp tanks.
pub const FILTER_FLOW_TURNOVERS_PER_HOUR: f64 = 10.0;

/// Baseline heater sizing guideline: 0.75 W per liter.
/// Standard recommendation is 0.5–1.0 W/L depending on ambient-to-target delta;
/// 0.75 is our default target for standard open-top tanks.
pub const HEATER_WATTS_PER_LITER: f64 = 0.75;

/// Upper heater sizing cap: 1.0 W per liter.
/// Very exposed or shallow geometries can need more replacement heat than the
/// 0.75 W/L midpoint, but we still clamp to the top of the common 0.5–1.0 W/L
/// guideline band.
pub const HEATER_MAX_WATTS_PER_LITER: f64 = 1.0;

/// Design ambient-to-target delta used when converting geometry-driven heat
/// loss (`UA`) into a heater recommendation.
/// This keeps shallow/high-exposure tanks from being undersized while leaving
/// standard geometries near the 0.75 W/L midpoint.
pub const HEATER_DESIGN_DELTA_C: f64 = 8.0;

/// Initial plant biomass per guild: 3.0 g per 1000 cm² of substrate footprint.
/// Conservative starter density — below the "moderately planted" guideline
/// (5–15 g/1000 cm²) to preserve biofilter maturation dynamics during the
/// cycling phase. Higher densities are reachable through natural plant growth.
/// Total initial plant mass = density × footprint / 1000 × number of guilds.
pub const PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT: f64 = 3.0;

/// Midpoint startup shrimp density for auto-stocking mode: 0.2 adults per liter.
/// The effective density ramps between 0.15 and 0.25 adults/L as tank water
/// volume grows, with this midpoint reached around a 75 L filled setup.
/// Applied against the current filled water volume (after substrate
/// displacement), not the nominal empty-box volume, so shallow heavily
/// hardscaped starts do not silently overstock relative to actual water.
/// This remains well below the mature colony range of 2–5/L. Only used when
/// `auto_stock_shrimp` is enabled and no explicit count is provided.
pub const AUTO_STOCK_ADULTS_PER_LITER: f64 = 0.2;

/// Lower bound for conservative startup auto-stocking.
pub const AUTO_STOCK_MIN_ADULTS_PER_LITER: f64 = 0.15;

/// Upper bound for conservative startup auto-stocking.
pub const AUTO_STOCK_MAX_ADULTS_PER_LITER: f64 = 0.25;

/// Filled-water volume at which auto-stocking reaches the upper conservative
/// density bound. Smaller tanks interpolate between the min and max bounds.
pub const AUTO_STOCK_FULL_DENSITY_VOLUME_L: f64 = 150.0;

/// Reference footprint for initial nitrifier biomass scaling (cm²).
/// `MicrobeState::default()` was calibrated for the default `TankGeometry`
/// (40 × 25 = 1000 cm² footprint). Tanks with larger footprint receive
/// proportionally more initial nitrifiers so that biofilter maturity ratios
/// remain consistent as habitat area scales.
const MICROBE_REFERENCE_FOOTPRINT_CM2: f64 = 1000.0;

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

pub fn recommended_auto_stock_adults_per_liter(volume_l: f64) -> f64 {
    let normalized_volume = (volume_l / AUTO_STOCK_FULL_DENSITY_VOLUME_L).clamp(0.0, 1.0);
    AUTO_STOCK_MIN_ADULTS_PER_LITER
        + (AUTO_STOCK_MAX_ADULTS_PER_LITER - AUTO_STOCK_MIN_ADULTS_PER_LITER) * normalized_volume
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
    /// Optional explicit biomedia area (cm²). When `None`, starter hardware
    /// scales the default filter media area with the tank volume.
    pub filter_media_area_cm2: Option<f64>,
    pub light_preset: Option<StartupLightPreset>,
    pub heater_preset: Option<StartupHeaterPreset>,
    pub aeration_enabled: Option<bool>,
    pub initial_adult_shrimp_count: Option<u32>,
    /// When true and `initial_adult_shrimp_count` is `None`, auto-stock using
    /// a conservative geometry-aware density between
    /// [`AUTO_STOCK_MIN_ADULTS_PER_LITER`] and
    /// [`AUTO_STOCK_MAX_ADULTS_PER_LITER`] adults per liter. Explicit counts
    /// always take priority over auto-stocking.
    pub auto_stock_shrimp: bool,
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
                StartupLightPreset::Hours10,
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
        filter_media_area_cm2: scenario.filter_media_area_cm2,
        light_preset: Some(light_preset),
        heater_preset: Some(heater_preset),
        aeration_enabled: Some(aeration_enabled),
        initial_adult_shrimp_count: Some(initial_adult_shrimp_count),
        auto_stock_shrimp: false,
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

        denitrification_vmax_mg_n_per_l_per_hour: defaults.denitrification_vmax_mg_n_per_l_per_hour,
        denitrification_k_no3_mg_n_per_l: defaults.denitrification_k_no3_mg_n_per_l,
        denitrification_k_doc_mg_c_per_l: defaults.denitrification_k_doc_mg_c_per_l,
        denitrification_pore_water_mixing_factor: defaults.denitrification_pore_water_mixing_factor,
        denitrification_activity_maturation_days: defaults.denitrification_activity_maturation_days,

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
        rol_rate_cm_per_g: defaults.rol_rate_cm_per_g,

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

pub fn recommended_heater_max_watts(
    geometry: &TankGeometry,
    process_params: &ProcessParams,
) -> f64 {
    let volume_l = geometry.gross_water_volume_l().max(0.0);
    if volume_l <= f64::EPSILON {
        return 0.0;
    }

    let surface_area_m2 = geometry.surface_area_cm2() / 10_000.0;
    let wall_area_m2 = geometry.wall_area_cm2() / 10_000.0;
    let ua_total_w_per_k =
        (process_params.k_surface_w_per_m2_k * surface_area_m2 * geometry.top_exchange_factor())
            + (process_params.k_wall_w_per_m2_k * wall_area_m2);
    let volume_guideline_w = volume_l * HEATER_WATTS_PER_LITER;
    let exposed_loss_w = ua_total_w_per_k * HEATER_DESIGN_DELTA_C;

    volume_guideline_w
        .max(exposed_loss_w)
        .min(volume_l * HEATER_MAX_WATTS_PER_LITER)
}

/// Scales hardware and biological defaults to match the current tank geometry.
///
/// Called once during materialization so that defaults are proportional to tank
/// size before any explicit overrides are applied. Explicit scenario overrides
/// in [`apply_startup_overrides`] always take priority.
///
/// **Scaling rules applied:**
/// - Filter flow: `volume_l × FILTER_FLOW_TURNOVERS_PER_HOUR` (10 turnovers/hr)
/// - Filter media area: default biomedia scales with gross water volume
///   relative to the default 22 L reference tank unless the scenario/startup
///   config supplies an
///   explicit hardware-specific media area.
/// - Heater max watts: `max(volume_l × HEATER_WATTS_PER_LITER,
///   UA(geometry) × HEATER_DESIGN_DELTA_C)`, capped at
///   `volume_l × HEATER_MAX_WATTS_PER_LITER`
/// - Initial nitrifier biomass: scaled proportionally with footprint area
///   relative to the 1000 cm² reference, floored at 1.0× (tanks ≤ 1000 cm²
///   keep defaults).
/// - Initial decomposer biomass: scaled with gross water volume relative to the
///   default 22 L reference so feed/DOC mineralization starts at a similar
///   conservative per-liter intensity across geometry-scaled stocked runs.
///
/// Properties that do **not** scale automatically:
/// - Light intensity/photoperiod (fixture property, independent of tank size)
/// - Aeration intensity (setting property; physical effect already scales via
///   the habitat registry's surface-area calculations)
/// - Substrate depth (preset property, independent of tank footprint)
fn scale_hardware_to_geometry(state: &mut TankState, process_params: &ProcessParams) {
    let volume_l = state.geometry.gross_water_volume_l();
    let footprint_cm2 = state.geometry.footprint_area_cm2();
    let footprint_scale = (footprint_cm2 / MICROBE_REFERENCE_FOOTPRINT_CM2).max(0.0);
    let reference_volume_l = TankGeometry::default().gross_water_volume_l();
    let volume_scale = if reference_volume_l > f64::EPSILON {
        volume_l / reference_volume_l
    } else {
        1.0
    };

    // Filter: flow scales with volume (turnovers stay constant per hour)
    if state.hardware.filter.enabled {
        state.hardware.filter.flow_lph = volume_l * FILTER_FLOW_TURNOVERS_PER_HOUR;
    }
    state.hardware.filter.media_area_cm2 *= volume_scale.max(0.0);

    // Heater: recommendation combines the usual W/L sizing guideline with an
    // exposed-area heat-loss floor derived from the current geometry.
    state.hardware.heater.max_watts = recommended_heater_max_watts(&state.geometry, process_params);

    // Nitrifiers: scale with footprint area so startup biofilter maturity
    // tracks the surface-driven habitat available to colonize.
    let nitrifier_scale = footprint_scale.max(1.0);
    if nitrifier_scale > 1.0 {
        state.microbe.ammonia_oxidizer_biomass_g *= nitrifier_scale;
        state.microbe.nitrite_oxidizer_biomass_g *= nitrifier_scale;
        state.microbe.comammox_biomass_g *= nitrifier_scale;
    }

    // Decomposers: scale with gross water volume so proportional stocking and
    // feeding do not produce a geometry-driven TAN pulse solely because larger
    // tanks would otherwise start with less decomposer biomass per liter.
    let decomposer_scale = volume_scale.max(1.0);
    if decomposer_scale > 1.0 {
        state.microbe.decomposer_biomass_g *= decomposer_scale;
        for val in state.microbe.decomposer_by_habitat.values_mut() {
            *val *= decomposer_scale;
        }
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
    let scenario_id = scenario.id.clone();
    let scenario_name = scenario.name.clone();

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

    // Plant guilds — biomass scales with substrate footprint
    let plant_guilds = build_plant_guilds(
        &scenario.plant_ids,
        &substrate_layers,
        true,
        geometry.footprint_area_cm2(),
    )?;

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
        scenario_id: Some(scenario_id.clone()),
        notes: Some(scenario_name),
    };
    state.geometry = geometry;
    scale_hardware_to_geometry(&mut state, &process_params);
    if let Some(filter_media_area_cm2) = scenario.filter_media_area_cm2 {
        state.hardware.filter.media_area_cm2 =
            validate_filter_media_area_cm2(filter_media_area_cm2, "scenarios", &scenario_id)?;
    }
    state.environment = environment;
    state.water = water;
    state.substrate_layers = substrate_layers;
    state.plant_guilds = plant_guilds;
    state.source_water_catalog = source_water_catalog;
    state.process_params = process_params;
    state.shrimp_params = shrimp_params;

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
    if let Some(value) = shrimp_preset.failed_molt_accum_increase_per_failed_stage {
        params.failed_molt_accum_increase_per_failed_stage = value;
    }
    if let Some(value) = shrimp_preset.failed_molt_accum_recovery_per_successful_stage {
        params.failed_molt_accum_recovery_per_successful_stage = value;
    }
    if let Some(value) = shrimp_preset.failed_molt_stress_blend {
        params.failed_molt_stress_blend = value;
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
    if let Some(value) = shrimp_preset.ca_min_mg_per_l {
        params.ca_min_mg_per_l = value;
    }
    if let Some(value) = shrimp_preset.mg_min_mg_per_l {
        params.mg_min_mg_per_l = value;
    }
    if let Some(value) = shrimp_preset.molt_reserve_fraction {
        params.molt_reserve_fraction = value;
    }
    if let Some(value) = shrimp_preset.molt_reserve_factor_floor {
        params.molt_reserve_factor_floor = value;
    }
    if let Some(value) = shrimp_preset.molt_condition_weight {
        params.molt_condition_weight = value;
    }
    if let Some(value) = shrimp_preset.molt_reserve_weight {
        params.molt_reserve_weight = value;
    }
    if let Some(value) = shrimp_preset.molt_failure_poor_condition_threshold {
        params.molt_failure_poor_condition_threshold = value;
    }
    if let Some(value) = shrimp_preset.molt_failure_instability_threshold {
        params.molt_failure_instability_threshold = value;
    }
    if let Some(value) = shrimp_preset.juvenile_molt_interval_days {
        params.juvenile_molt_interval_days = value;
    }
    if let Some(value) = shrimp_preset.sub_adult_molt_interval_days {
        params.sub_adult_molt_interval_days = value;
    }
    if let Some(value) = shrimp_preset.molt_success_threshold {
        params.molt_success_threshold = value;
    }
    if let Some(value) = shrimp_preset.critical_molt_gh_ratio {
        params.critical_molt_gh_ratio = value;
    }
    if let Some(value) = shrimp_preset.chloride_protection_factor {
        params.chloride_protection_factor = value;
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
        filter_media_area_cm2,
        light_preset,
        heater_preset,
        aeration_enabled,
        initial_adult_shrimp_count,
        auto_stock_shrimp,
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
        state.plant_guilds = build_plant_guilds(
            &plant_ids,
            &state.substrate_layers,
            false,
            state.geometry.footprint_area_cm2(),
        )?;
    }

    if let Some(profile) = effective_source_profile {
        state.water =
            WaterState::from_source_profile_for_volume_l(&profile, state.water_volume_l());
    }

    if let Some(filter_enabled) = filter_enabled {
        state.hardware.filter.enabled = filter_enabled;
        if filter_enabled {
            // Geometry-scaled flow was already set by scale_hardware_to_geometry();
            // only override if flow is somehow zero (e.g. filter was previously disabled).
            if state.hardware.filter.flow_lph <= 0.0 {
                let volume_l = state.geometry.gross_water_volume_l();
                state.hardware.filter.flow_lph = volume_l * FILTER_FLOW_TURNOVERS_PER_HOUR;
            }
        } else {
            state.hardware.filter.flow_lph = 0.0;
        }
    }
    if let Some(filter_media_area_cm2) = filter_media_area_cm2 {
        state.hardware.filter.media_area_cm2 = validate_filter_media_area_cm2(
            filter_media_area_cm2,
            "startup_overrides",
            scenario_id,
        )?;
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
        // Explicit scenario-authored count always wins.
        state.animal = AnimalState::with_adults(initial_adult_shrimp_count);
    } else if auto_stock_shrimp {
        // Conservative auto-stocking based on current filled water volume.
        let volume_l = state.water_volume_l();
        let density = recommended_auto_stock_adults_per_liter(volume_l);
        let count = (volume_l * density).round().max(1.0) as u32;
        state.animal = AnimalState::with_adults(count);
    }

    // Reseed stability baselines so that overridden water chemistry is not
    // treated as a "swing" on the first daily update.
    state.reseed_stability_tracker();
    state.refresh_habitat_registry();

    Ok(())
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
            porosity: kind.default_porosity(),
            o2_penetration_depth_cm: sub_preset.depth_cm.max(0.0),
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
    footprint_cm2: f64,
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
        let biomass_g = (footprint_cm2 * PLANT_BIOMASS_G_PER_1000_CM2_FOOTPRINT / 1000.0).max(1.0);
        plant_guilds.push(PlantGuildState {
            guild,
            biomass_g,
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

fn validate_filter_media_area_cm2(
    area_cm2: f64,
    category: &'static str,
    id: &str,
) -> Result<f64, tank_data::PresetError> {
    if !area_cm2.is_finite() || area_cm2 < 0.0 {
        return Err(tank_data::PresetError::Validation {
            category,
            id: id.to_string(),
            message: format!(
                "filter_media_area_cm2 must be finite and >= 0.0, got {}",
                area_cm2
            ),
        });
    }

    Ok(area_cm2)
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
        ..MicrobeState::default()
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
        ..MicrobeState::default()
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
    use super::{
        materialize_scenario, process_preset_to_params, shrimp_preset_to_params,
        source_water_to_profile,
    };
    use tank_core::{SimSeed, WaterState};

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
        preset.failed_molt_accum_increase_per_failed_stage = Some(0.18);
        preset.failed_molt_accum_recovery_per_successful_stage = Some(0.41);
        preset.failed_molt_stress_blend = Some(0.33);
        preset.sub_adult_sensitivity = Some(1.9);
        preset.base_clutch_size = Some(17);
        preset.min_clutch_condition = Some(0.44);
        preset.ca_min_mg_per_l = Some(18.0);
        preset.mg_min_mg_per_l = Some(4.0);
        preset.molt_failure_poor_condition_threshold = Some(0.7);
        preset.molt_failure_instability_threshold = Some(0.24);
        preset.juvenile_molt_interval_days = Some(11.0);
        preset.sub_adult_molt_interval_days = Some(17.0);
        preset.molt_success_threshold = Some(0.61);
        preset.critical_molt_gh_ratio = Some(0.22);

        let params = shrimp_preset_to_params(&preset, 40.0);
        assert_eq!(params.body_nitrogen_mg_per_g_wet_mass, 31.0);
        assert_eq!(params.body_carbon_mg_per_g_wet_mass, 165.0);
        assert!((params.juvenile_to_subadult_days - 24.0).abs() < 1e-9);
        assert!((params.subadult_to_adult_days - 16.0).abs() < 1e-9);
        assert_eq!(params.juvenile_maturation_condition_threshold, 0.22);
        assert_eq!(params.subadult_maturation_condition_threshold, 0.41);
        assert_eq!(params.base_molt_interval_days, 19.0);
        assert_eq!(params.failed_molt_mortality_scale, 0.27);
        assert_eq!(params.failed_molt_accum_increase_per_failed_stage, 0.18);
        assert_eq!(params.failed_molt_accum_recovery_per_successful_stage, 0.41);
        assert_eq!(params.failed_molt_stress_blend, 0.33);
        assert_eq!(params.sub_adult_sensitivity, 1.9);
        assert_eq!(params.base_clutch_size, 17);
        assert_eq!(params.min_clutch_condition, 0.44);
        assert_eq!(params.ca_min_mg_per_l, 18.0);
        assert_eq!(params.mg_min_mg_per_l, 4.0);
        assert_eq!(params.molt_failure_poor_condition_threshold, 0.7);
        assert_eq!(params.molt_failure_instability_threshold, 0.24);
        assert_eq!(params.juvenile_molt_interval_days, 11.0);
        assert_eq!(params.sub_adult_molt_interval_days, 17.0);
        assert_eq!(params.molt_success_threshold, 0.61);
        assert_eq!(params.critical_molt_gh_ratio, 0.22);

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
        assert!(ro_water.ph < soft_water.ph);
        assert!(
            hard_water.ph - soft_water.ph >= 0.3,
            "expected at least 0.3 pH spread, got soft={:.3}, hard={:.3}",
            soft_water.ph,
            hard_water.ph
        );
        assert!(
            (5.5..=6.5).contains(&ro_water.ph),
            "expected ro_like to land in the low-buffer acidic band, got {:.3}",
            ro_water.ph
        );
    }

    #[test]
    fn materialize_scenario_honors_authored_filter_media_area() {
        let mut scenario = tank_data::load_scenario("nano_cycle").expect("nano_cycle should load");
        scenario.filter_media_area_cm2 = Some(4321.0);

        let state = materialize_scenario(SimSeed(55), scenario)
            .expect("scenario-authored filter media area should materialize");

        assert_eq!(state.hardware.filter.media_area_cm2, 4321.0);
    }
}
