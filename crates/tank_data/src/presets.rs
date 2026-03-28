use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tank_core::systems::chemistry::{
    validate_source_water_carbonate_profile, SourceWaterCarbonateValidationError, CARBONATE_PH_MAX,
    CARBONATE_PH_MIN,
};
use tank_core::types::provenance::{ParamMeta, RangeWarning};

const SHRIMP_ROUTE_SUM_TOLERANCE: f64 = 1e-9;

/// Preset-level provenance metadata (describes the preset file as a whole).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub source_title: Option<String>,
    pub source_year: Option<u16>,
    pub confidence: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceWaterPreset {
    pub id: String,
    pub name: String,
    pub temperature_c: f64,
    pub ammonia_mg_n_per_l: f64,
    pub nitrite_mg_n_per_l: f64,
    pub nitrate_mg_n_per_l: f64,
    pub phosphate_mg_p_per_l: f64,
    pub dic_mg_c_per_l: f64,
    pub doc_mg_c_per_l: f64,
    pub don_mg_n_per_l: f64,
    pub alkalinity_meq_per_l: f64,
    pub calcium_mg_per_l: f64,
    pub magnesium_mg_per_l: f64,
    pub sodium_mg_per_l: f64,
    pub potassium_mg_per_l: f64,
    pub bicarbonate_mg_per_l: f64,
    pub chloride_mg_per_l: f64,
    pub sulfate_mg_per_l: f64,
    pub provenance: Option<Provenance>,
}

impl SourceWaterPreset {
    /// Validates that all chemistry fields are finite and non-negative.
    pub fn validate(&self) -> Result<(), String> {
        let fields: &[(&str, f64)] = &[
            ("temperature_c", self.temperature_c),
            ("ammonia_mg_n_per_l", self.ammonia_mg_n_per_l),
            ("nitrite_mg_n_per_l", self.nitrite_mg_n_per_l),
            ("nitrate_mg_n_per_l", self.nitrate_mg_n_per_l),
            ("phosphate_mg_p_per_l", self.phosphate_mg_p_per_l),
            ("dic_mg_c_per_l", self.dic_mg_c_per_l),
            ("doc_mg_c_per_l", self.doc_mg_c_per_l),
            ("don_mg_n_per_l", self.don_mg_n_per_l),
            ("alkalinity_meq_per_l", self.alkalinity_meq_per_l),
            ("calcium_mg_per_l", self.calcium_mg_per_l),
            ("magnesium_mg_per_l", self.magnesium_mg_per_l),
            ("sodium_mg_per_l", self.sodium_mg_per_l),
            ("potassium_mg_per_l", self.potassium_mg_per_l),
            ("bicarbonate_mg_per_l", self.bicarbonate_mg_per_l),
            ("chloride_mg_per_l", self.chloride_mg_per_l),
            ("sulfate_mg_per_l", self.sulfate_mg_per_l),
        ];
        for (name, value) in fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        // Temperature must be strictly positive (0°C is rejected at runtime).
        if self.temperature_c <= 0.0 {
            return Err(format!(
                "field `temperature_c` must be > 0.0, got {}",
                self.temperature_c
            ));
        }
        validate_source_water_carbonate_profile(
            self.dic_mg_c_per_l,
            self.alkalinity_meq_per_l,
            self.temperature_c,
        )
        .map_err(|err| match err {
            SourceWaterCarbonateValidationError::OutOfRangePh(ph) => {
                format!(
                    "carbonate-derived pH {:.3} falls outside the calibrated source-water envelope or lands on an unsupported clamp boundary ({}, {}) for dic_mg_c_per_l={} and alkalinity_meq_per_l={}",
                    ph,
                    CARBONATE_PH_MIN,
                    CARBONATE_PH_MAX,
                    self.dic_mg_c_per_l,
                    self.alkalinity_meq_per_l
                )
            }
            SourceWaterCarbonateValidationError::NonFiniteNeutralFallback => {
                format!(
                    "carbonate-derived pH used the non-finite neutral fallback for dic_mg_c_per_l={} and alkalinity_meq_per_l={}",
                    self.dic_mg_c_per_l,
                    self.alkalinity_meq_per_l
                )
            }
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubstratePreset {
    pub id: String,
    pub name: String,
    pub depth_cm: f64,
    pub cation_exchange_capacity_index: f64,
    pub detritus_trapping_index: f64,
    pub colonizable_area_factor: f64,
    pub low_oxygen_tendency_index: f64,
    pub grazing_surface_index: f64,
    pub nutrient_charge_mg_n_total: f64,
    pub nutrient_charge_mg_p_total: f64,
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantPreset {
    pub id: String,
    pub name: String,
    pub guild: String,
    pub growth_rate_index: f64,
    pub water_column_uptake_bias: f64,
    pub substrate_uptake_bias: f64,
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShrimpPreset {
    pub id: String,
    pub name: String,
    pub species: String,
    pub optimal_temp_min_c: f64,
    pub optimal_temp_max_c: f64,
    pub gh_min_d: f64,
    pub gh_max_d: f64,
    #[serde(default = "default_shrimp_base_spawn_rate")]
    pub base_spawn_rate: f64,
    #[serde(default = "default_shrimp_egg_duration_days")]
    pub egg_duration_days: u32,
    #[serde(default = "default_shrimp_hatch_success_base")]
    pub hatch_success_base: f64,
    #[serde(default = "default_shrimp_juvenile_sensitivity")]
    pub juvenile_sensitivity: f64,
    #[serde(default = "default_shrimp_high_temp_repro_penalty_start")]
    pub high_temp_repro_penalty_start_c: f64,
    #[serde(default = "default_shrimp_high_temp_repro_penalty_full")]
    pub high_temp_repro_penalty_full_c: f64,
    pub provenance: Option<Provenance>,
}

impl ShrimpPreset {
    /// Validates that all shrimp species parameters are finite, non-negative,
    /// and logically consistent.
    pub fn validate(&self) -> Result<(), String> {
        let fields: &[(&str, f64)] = &[
            ("optimal_temp_min_c", self.optimal_temp_min_c),
            ("optimal_temp_max_c", self.optimal_temp_max_c),
            ("gh_min_d", self.gh_min_d),
            ("gh_max_d", self.gh_max_d),
            ("base_spawn_rate", self.base_spawn_rate),
            ("hatch_success_base", self.hatch_success_base),
            ("juvenile_sensitivity", self.juvenile_sensitivity),
            (
                "high_temp_repro_penalty_start_c",
                self.high_temp_repro_penalty_start_c,
            ),
            (
                "high_temp_repro_penalty_full_c",
                self.high_temp_repro_penalty_full_c,
            ),
        ];
        for (name, value) in fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        if self.egg_duration_days == 0 {
            return Err("egg_duration_days must be > 0".to_string());
        }
        if self.optimal_temp_min_c >= self.optimal_temp_max_c {
            return Err(format!(
                "optimal_temp_min_c ({}) must be < optimal_temp_max_c ({})",
                self.optimal_temp_min_c, self.optimal_temp_max_c
            ));
        }
        if self.gh_min_d >= self.gh_max_d {
            return Err(format!(
                "gh_min_d ({}) must be < gh_max_d ({})",
                self.gh_min_d, self.gh_max_d
            ));
        }
        if self.high_temp_repro_penalty_start_c >= self.high_temp_repro_penalty_full_c {
            return Err(format!(
                "high_temp_repro_penalty_start_c ({}) must be < high_temp_repro_penalty_full_c ({})",
                self.high_temp_repro_penalty_start_c, self.high_temp_repro_penalty_full_c
            ));
        }
        if self.base_spawn_rate > 1.0 {
            return Err(format!(
                "base_spawn_rate must be <= 1.0, got {}",
                self.base_spawn_rate
            ));
        }
        if self.hatch_success_base > 1.0 {
            return Err(format!(
                "hatch_success_base must be <= 1.0, got {}",
                self.hatch_success_base
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessParamsPreset {
    pub id: String,
    pub name: String,
    pub mineralization_rate_per_day: f64,
    pub nitrification_vmax: f64,
    pub reaeration_kla_base: f64,
    pub aeration_kla_boost: f64,
    pub background_bod_mg_o2_per_g_biomass_per_hour: f64,
    pub plant_photosynthesis_o2_mg_per_g_per_hour: f64,
    pub respiration_dic_rate_mg_c_per_g_per_hour: f64,
    pub photosynthesis_dic_rate_mg_c_per_g_per_hour: f64,
    pub k_surface_w_per_m2_k: f64,
    pub k_wall_w_per_m2_k: f64,

    // Nitrogen cycle: feed leaching
    #[serde(default = "default_feed_leach_rate")]
    pub feed_leach_rate_per_hour: f64,
    #[serde(default = "default_fine_detritus_dissolution_rate")]
    pub fine_detritus_dissolution_rate_per_hour: f64,
    #[serde(default = "default_feed_n_to_c_ratio")]
    pub feed_n_to_c_ratio: f64,

    // Decomposer mineralization
    #[serde(default = "default_decomposer_vmax")]
    pub decomposer_vmax_per_hour: f64,
    #[serde(default = "default_decomposer_k_doc")]
    pub decomposer_k_doc_mg: f64,
    #[serde(default = "default_decomposer_k_do")]
    pub decomposer_k_do_mg: f64,
    #[serde(default = "default_decomposer_growth_yield")]
    pub decomposer_growth_yield: f64,
    #[serde(default = "default_decomposer_decay_rate")]
    pub decomposer_decay_rate_per_hour: f64,

    // AOB kinetics
    #[serde(default = "default_aob_vmax")]
    pub aob_vmax_mg_n_per_g_per_hour: f64,
    #[serde(default = "default_aob_k_tan")]
    pub aob_k_tan_mg: f64,
    #[serde(default = "default_aob_k_do")]
    pub aob_k_do_mg: f64,
    #[serde(default = "default_aob_growth_yield")]
    pub aob_growth_yield: f64,
    #[serde(default = "default_aob_decay_rate")]
    pub aob_decay_rate_per_hour: f64,

    // NOB kinetics
    #[serde(default = "default_nob_vmax")]
    pub nob_vmax_mg_n_per_g_per_hour: f64,
    #[serde(default = "default_nob_k_nitrite")]
    pub nob_k_nitrite_mg: f64,
    #[serde(default = "default_nob_k_do")]
    pub nob_k_do_mg: f64,
    #[serde(default = "default_nob_growth_yield")]
    pub nob_growth_yield: f64,
    #[serde(default = "default_nob_decay_rate")]
    pub nob_decay_rate_per_hour: f64,

    // Comammox kinetics
    #[serde(default = "default_comammox_vmax_fraction")]
    pub comammox_vmax_fraction: f64,
    #[serde(default = "default_comammox_k_tan")]
    pub comammox_k_tan_mg: f64,
    #[serde(default = "default_comammox_k_do")]
    pub comammox_k_do_mg: f64,
    #[serde(default = "default_comammox_growth_yield")]
    pub comammox_growth_yield: f64,
    #[serde(default = "default_comammox_decay_rate")]
    pub comammox_decay_rate_per_hour: f64,

    // Stoichiometric constants
    #[serde(default = "default_o2_per_mg_n")]
    pub o2_per_mg_n_nitrified: f64,
    #[serde(default = "default_alk_per_mg_n")]
    pub alkalinity_meq_per_mg_n_nitrified: f64,

    #[serde(default = "default_plant_max_growth_fast_stem")]
    pub plant_max_growth_rate_fast_stem_per_day: f64,
    #[serde(default = "default_plant_max_growth_root_rosette")]
    pub plant_max_growth_rate_root_rosette_per_day: f64,
    #[serde(default = "default_plant_respiration_fraction")]
    pub plant_respiration_fraction_per_day: f64,
    #[serde(default = "default_plant_senescence_fraction")]
    pub plant_senescence_fraction_per_day: f64,
    #[serde(default = "default_plant_health_recovery")]
    pub plant_health_recovery_per_day: f64,
    #[serde(default = "default_plant_health_decline")]
    pub plant_health_decline_per_day: f64,
    #[serde(default = "default_plant_half_sat_n")]
    pub plant_half_saturation_n_mg_total: f64,
    #[serde(default = "default_plant_half_sat_p")]
    pub plant_half_saturation_p_mg_total: f64,
    #[serde(default = "default_plant_half_sat_c")]
    pub plant_half_saturation_c_mg_total: f64,
    #[serde(default = "default_plant_light_half_sat")]
    pub plant_light_half_saturation: f64,
    #[serde(default = "default_plant_temp_optimum")]
    pub plant_temp_optimum_c: f64,
    #[serde(default = "default_plant_temp_sigma")]
    pub plant_temp_sigma_c: f64,
    #[serde(default = "default_plant_crowding_biomass")]
    pub plant_crowding_biomass_g_per_m2: f64,

    #[serde(default = "default_algae_max_growth")]
    pub algae_max_growth_rate_per_day: f64,
    #[serde(default = "default_periphyton_max_growth")]
    pub periphyton_max_growth_rate_per_day: f64,
    #[serde(default = "default_algae_respiration_fraction")]
    pub algae_respiration_fraction_per_day: f64,
    #[serde(default = "default_algae_half_sat_n")]
    pub algae_half_saturation_n_mg_total: f64,
    #[serde(default = "default_algae_half_sat_p")]
    pub algae_half_saturation_p_mg_total: f64,
    #[serde(default = "default_algae_light_half_sat")]
    pub algae_light_half_saturation: f64,
    #[serde(default = "default_algae_temp_optimum")]
    pub algae_temp_optimum_c: f64,
    #[serde(default = "default_algae_temp_sigma")]
    pub algae_temp_sigma_c: f64,
    #[serde(default = "default_periphyton_capacity")]
    pub periphyton_capacity_g_per_m2: f64,
    #[serde(default = "default_algae_bloom_threshold")]
    pub algae_bloom_threshold_g_per_l: f64,
    #[serde(default = "default_algae_nuisance_biomass")]
    pub algae_nuisance_biomass_g_per_m2: f64,

    // -- Shrimp dynamics --
    #[serde(default = "default_shrimp_base_mortality")]
    pub shrimp_base_mortality_per_day: f64,
    #[serde(default = "default_shrimp_stress_mortality_scale")]
    pub shrimp_stress_mortality_scale: f64,
    #[serde(default = "default_shrimp_juvenile_maturation_days")]
    pub shrimp_juvenile_maturation_days: f64,
    #[serde(default = "default_shrimp_periphyton_grazing")]
    pub shrimp_periphyton_grazing_g_per_shrimp_per_day: f64,
    #[serde(default = "default_shrimp_condition_smoothing")]
    pub shrimp_condition_smoothing: f64,
    #[serde(default = "default_shrimp_assimilation_efficiency")]
    pub shrimp_assimilation_efficiency: f64,
    #[serde(default = "default_shrimp_respiration_fraction")]
    pub shrimp_respiration_fraction_of_assimilated: f64,
    #[serde(default = "default_shrimp_excretion_fraction")]
    pub shrimp_excretion_fraction_of_assimilated: f64,
    #[serde(default = "default_shrimp_growth_fraction")]
    pub shrimp_growth_fraction_of_assimilated: f64,
    #[serde(default = "default_shrimp_o2_per_mg_c_respired")]
    pub shrimp_o2_per_mg_c_respired: f64,

    // -- Microfauna turnover --
    #[serde(default = "default_microfauna_mineralization_boost")]
    pub microfauna_mineralization_boost: f64,
    #[serde(default = "default_microfauna_periphyton_consumption")]
    pub microfauna_periphyton_consumption: f64,
    #[serde(default = "default_microfauna_population_smoothing")]
    pub microfauna_population_smoothing: f64,
    #[serde(default = "default_microfauna_shrimp_pressure_threshold")]
    pub microfauna_shrimp_pressure_threshold: f64,

    pub provenance: Option<Provenance>,

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl ProcessParamsPreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        self.param_meta
            .keys()
            .filter(|name| self.param_value(name).is_none())
            .map(String::as_str)
            .collect()
    }

    /// Look up a parameter value by name. Returns `None` for unknown names.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        match name {
            "mineralization_rate_per_day" => Some(self.mineralization_rate_per_day),
            "nitrification_vmax" => Some(self.nitrification_vmax),
            "reaeration_kla_base" => Some(self.reaeration_kla_base),
            "aeration_kla_boost" => Some(self.aeration_kla_boost),
            "background_bod_mg_o2_per_g_biomass_per_hour" => {
                Some(self.background_bod_mg_o2_per_g_biomass_per_hour)
            }
            "plant_photosynthesis_o2_mg_per_g_per_hour" => {
                Some(self.plant_photosynthesis_o2_mg_per_g_per_hour)
            }
            "respiration_dic_rate_mg_c_per_g_per_hour" => {
                Some(self.respiration_dic_rate_mg_c_per_g_per_hour)
            }
            "photosynthesis_dic_rate_mg_c_per_g_per_hour" => {
                Some(self.photosynthesis_dic_rate_mg_c_per_g_per_hour)
            }
            "k_surface_w_per_m2_k" => Some(self.k_surface_w_per_m2_k),
            "k_wall_w_per_m2_k" => Some(self.k_wall_w_per_m2_k),
            "feed_leach_rate_per_hour" => Some(self.feed_leach_rate_per_hour),
            "fine_detritus_dissolution_rate_per_hour" => {
                Some(self.fine_detritus_dissolution_rate_per_hour)
            }
            "feed_n_to_c_ratio" => Some(self.feed_n_to_c_ratio),
            "decomposer_vmax_per_hour" => Some(self.decomposer_vmax_per_hour),
            "decomposer_k_doc_mg" => Some(self.decomposer_k_doc_mg),
            "decomposer_k_do_mg" => Some(self.decomposer_k_do_mg),
            "decomposer_growth_yield" => Some(self.decomposer_growth_yield),
            "decomposer_decay_rate_per_hour" => Some(self.decomposer_decay_rate_per_hour),
            "aob_vmax_mg_n_per_g_per_hour" => Some(self.aob_vmax_mg_n_per_g_per_hour),
            "aob_k_tan_mg" => Some(self.aob_k_tan_mg),
            "aob_k_do_mg" => Some(self.aob_k_do_mg),
            "aob_growth_yield" => Some(self.aob_growth_yield),
            "aob_decay_rate_per_hour" => Some(self.aob_decay_rate_per_hour),
            "nob_vmax_mg_n_per_g_per_hour" => Some(self.nob_vmax_mg_n_per_g_per_hour),
            "nob_k_nitrite_mg" => Some(self.nob_k_nitrite_mg),
            "nob_k_do_mg" => Some(self.nob_k_do_mg),
            "nob_growth_yield" => Some(self.nob_growth_yield),
            "nob_decay_rate_per_hour" => Some(self.nob_decay_rate_per_hour),
            "comammox_vmax_fraction" => Some(self.comammox_vmax_fraction),
            "comammox_k_tan_mg" => Some(self.comammox_k_tan_mg),
            "comammox_k_do_mg" => Some(self.comammox_k_do_mg),
            "comammox_growth_yield" => Some(self.comammox_growth_yield),
            "comammox_decay_rate_per_hour" => Some(self.comammox_decay_rate_per_hour),
            "o2_per_mg_n_nitrified" => Some(self.o2_per_mg_n_nitrified),
            "alkalinity_meq_per_mg_n_nitrified" => Some(self.alkalinity_meq_per_mg_n_nitrified),
            "plant_max_growth_rate_fast_stem_per_day" => {
                Some(self.plant_max_growth_rate_fast_stem_per_day)
            }
            "plant_max_growth_rate_root_rosette_per_day" => {
                Some(self.plant_max_growth_rate_root_rosette_per_day)
            }
            "plant_respiration_fraction_per_day" => Some(self.plant_respiration_fraction_per_day),
            "plant_senescence_fraction_per_day" => Some(self.plant_senescence_fraction_per_day),
            "plant_health_recovery_per_day" => Some(self.plant_health_recovery_per_day),
            "plant_health_decline_per_day" => Some(self.plant_health_decline_per_day),
            "plant_half_saturation_n_mg_total" => Some(self.plant_half_saturation_n_mg_total),
            "plant_half_saturation_p_mg_total" => Some(self.plant_half_saturation_p_mg_total),
            "plant_half_saturation_c_mg_total" => Some(self.plant_half_saturation_c_mg_total),
            "plant_light_half_saturation" => Some(self.plant_light_half_saturation),
            "plant_temp_optimum_c" => Some(self.plant_temp_optimum_c),
            "plant_temp_sigma_c" => Some(self.plant_temp_sigma_c),
            "plant_crowding_biomass_g_per_m2" => Some(self.plant_crowding_biomass_g_per_m2),
            "algae_max_growth_rate_per_day" => Some(self.algae_max_growth_rate_per_day),
            "periphyton_max_growth_rate_per_day" => Some(self.periphyton_max_growth_rate_per_day),
            "algae_respiration_fraction_per_day" => Some(self.algae_respiration_fraction_per_day),
            "algae_half_saturation_n_mg_total" => Some(self.algae_half_saturation_n_mg_total),
            "algae_half_saturation_p_mg_total" => Some(self.algae_half_saturation_p_mg_total),
            "algae_light_half_saturation" => Some(self.algae_light_half_saturation),
            "algae_temp_optimum_c" => Some(self.algae_temp_optimum_c),
            "algae_temp_sigma_c" => Some(self.algae_temp_sigma_c),
            "periphyton_capacity_g_per_m2" => Some(self.periphyton_capacity_g_per_m2),
            "algae_bloom_threshold_g_per_l" => Some(self.algae_bloom_threshold_g_per_l),
            "algae_nuisance_biomass_g_per_m2" => Some(self.algae_nuisance_biomass_g_per_m2),
            "shrimp_base_mortality_per_day" => Some(self.shrimp_base_mortality_per_day),
            "shrimp_stress_mortality_scale" => Some(self.shrimp_stress_mortality_scale),
            "shrimp_juvenile_maturation_days" => Some(self.shrimp_juvenile_maturation_days),
            "shrimp_periphyton_grazing_g_per_shrimp_per_day" => {
                Some(self.shrimp_periphyton_grazing_g_per_shrimp_per_day)
            }
            "shrimp_condition_smoothing" => Some(self.shrimp_condition_smoothing),
            "shrimp_assimilation_efficiency" => Some(self.shrimp_assimilation_efficiency),
            "shrimp_respiration_fraction_of_assimilated" => {
                Some(self.shrimp_respiration_fraction_of_assimilated)
            }
            "shrimp_excretion_fraction_of_assimilated" => {
                Some(self.shrimp_excretion_fraction_of_assimilated)
            }
            "shrimp_growth_fraction_of_assimilated" => {
                Some(self.shrimp_growth_fraction_of_assimilated)
            }
            "shrimp_o2_per_mg_c_respired" => Some(self.shrimp_o2_per_mg_c_respired),
            "microfauna_mineralization_boost" => Some(self.microfauna_mineralization_boost),
            "microfauna_periphyton_consumption" => Some(self.microfauna_periphyton_consumption),
            "microfauna_population_smoothing" => Some(self.microfauna_population_smoothing),
            "microfauna_shrimp_pressure_threshold" => {
                Some(self.microfauna_shrimp_pressure_threshold)
            }
            _ => None,
        }
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        tank_core::types::provenance::check_all_ranges(&self.param_meta, &|name| {
            self.param_value(name)
        })
    }
}

fn default_feed_leach_rate() -> f64 {
    0.12
}
fn default_fine_detritus_dissolution_rate() -> f64 {
    0.08
}
fn default_feed_n_to_c_ratio() -> f64 {
    0.16
}
fn default_decomposer_vmax() -> f64 {
    0.02
}
fn default_decomposer_k_doc() -> f64 {
    5.0
}
fn default_decomposer_k_do() -> f64 {
    2.0
}
fn default_decomposer_growth_yield() -> f64 {
    0.3
}
fn default_decomposer_decay_rate() -> f64 {
    0.002
}
fn default_aob_vmax() -> f64 {
    1.5
}
fn default_aob_k_tan() -> f64 {
    0.5
}
fn default_aob_k_do() -> f64 {
    1.0
}
fn default_aob_growth_yield() -> f64 {
    0.05
}
fn default_aob_decay_rate() -> f64 {
    0.003
}
fn default_nob_vmax() -> f64 {
    1.2
}
fn default_nob_k_nitrite() -> f64 {
    0.3
}
fn default_nob_k_do() -> f64 {
    1.0
}
fn default_nob_growth_yield() -> f64 {
    0.04
}
fn default_nob_decay_rate() -> f64 {
    0.003
}
fn default_comammox_vmax_fraction() -> f64 {
    0.4
}
fn default_comammox_k_tan() -> f64 {
    0.8
}
fn default_comammox_k_do() -> f64 {
    1.5
}
fn default_comammox_growth_yield() -> f64 {
    0.03
}
fn default_comammox_decay_rate() -> f64 {
    0.004
}
fn default_o2_per_mg_n() -> f64 {
    4.57
}
fn default_alk_per_mg_n() -> f64 {
    0.1428
}
fn default_plant_max_growth_fast_stem() -> f64 {
    0.08
}
fn default_plant_max_growth_root_rosette() -> f64 {
    0.06
}
fn default_plant_respiration_fraction() -> f64 {
    0.01
}
fn default_plant_senescence_fraction() -> f64 {
    0.005
}
fn default_plant_health_recovery() -> f64 {
    0.03
}
fn default_plant_health_decline() -> f64 {
    0.08
}
fn default_plant_half_sat_n() -> f64 {
    8.0
}
fn default_plant_half_sat_p() -> f64 {
    1.2
}
fn default_plant_half_sat_c() -> f64 {
    20.0
}
fn default_plant_light_half_sat() -> f64 {
    0.45
}
fn default_plant_temp_optimum() -> f64 {
    25.0
}
fn default_plant_temp_sigma() -> f64 {
    7.0
}
fn default_plant_crowding_biomass() -> f64 {
    250.0
}
fn default_algae_max_growth() -> f64 {
    0.2
}
fn default_periphyton_max_growth() -> f64 {
    0.15
}
fn default_algae_respiration_fraction() -> f64 {
    0.03
}
fn default_algae_half_sat_n() -> f64 {
    5.0
}
fn default_algae_half_sat_p() -> f64 {
    0.8
}
fn default_algae_light_half_sat() -> f64 {
    0.35
}
fn default_algae_temp_optimum() -> f64 {
    27.0
}
fn default_algae_temp_sigma() -> f64 {
    8.0
}
fn default_periphyton_capacity() -> f64 {
    6.0
}
fn default_algae_bloom_threshold() -> f64 {
    0.08
}
fn default_algae_nuisance_biomass() -> f64 {
    10.0
}

fn default_shrimp_base_spawn_rate() -> f64 {
    0.15
}
fn default_shrimp_egg_duration_days() -> u32 {
    21
}
fn default_shrimp_hatch_success_base() -> f64 {
    0.7
}
fn default_shrimp_juvenile_sensitivity() -> f64 {
    1.5
}
fn default_shrimp_high_temp_repro_penalty_start() -> f64 {
    28.0
}
fn default_shrimp_high_temp_repro_penalty_full() -> f64 {
    33.0
}

fn default_shrimp_base_mortality() -> f64 {
    0.002
}
fn default_shrimp_stress_mortality_scale() -> f64 {
    0.15
}
fn default_shrimp_juvenile_maturation_days() -> f64 {
    30.0
}
fn default_shrimp_periphyton_grazing() -> f64 {
    0.01
}
fn default_shrimp_condition_smoothing() -> f64 {
    0.15
}
fn default_shrimp_assimilation_efficiency() -> f64 {
    0.50
}
fn default_shrimp_respiration_fraction() -> f64 {
    0.70
}
fn default_shrimp_excretion_fraction() -> f64 {
    0.10
}
fn default_shrimp_growth_fraction() -> f64 {
    0.20
}
fn default_shrimp_o2_per_mg_c_respired() -> f64 {
    2.67
}
fn default_microfauna_mineralization_boost() -> f64 {
    0.15
}
fn default_microfauna_periphyton_consumption() -> f64 {
    0.02
}
fn default_microfauna_population_smoothing() -> f64 {
    0.1
}
fn default_microfauna_shrimp_pressure_threshold() -> f64 {
    3.0
}

impl ProcessParamsPreset {
    pub fn validate(&self) -> Result<(), String> {
        let unknown_param_meta_keys = self.unknown_param_meta_keys();
        if !unknown_param_meta_keys.is_empty() {
            return Err(format!(
                "unknown param_meta entries: {}",
                unknown_param_meta_keys.join(", ")
            ));
        }

        let fields: &[(&str, f64)] = &[
            (
                "mineralization_rate_per_day",
                self.mineralization_rate_per_day,
            ),
            ("nitrification_vmax", self.nitrification_vmax),
            ("reaeration_kla_base", self.reaeration_kla_base),
            ("aeration_kla_boost", self.aeration_kla_boost),
            (
                "background_bod_mg_o2_per_g_biomass_per_hour",
                self.background_bod_mg_o2_per_g_biomass_per_hour,
            ),
            (
                "plant_photosynthesis_o2_mg_per_g_per_hour",
                self.plant_photosynthesis_o2_mg_per_g_per_hour,
            ),
            ("k_surface_w_per_m2_k", self.k_surface_w_per_m2_k),
            ("k_wall_w_per_m2_k", self.k_wall_w_per_m2_k),
            ("feed_leach_rate_per_hour", self.feed_leach_rate_per_hour),
            (
                "fine_detritus_dissolution_rate_per_hour",
                self.fine_detritus_dissolution_rate_per_hour,
            ),
            ("feed_n_to_c_ratio", self.feed_n_to_c_ratio),
            ("decomposer_vmax_per_hour", self.decomposer_vmax_per_hour),
            ("decomposer_k_doc_mg", self.decomposer_k_doc_mg),
            ("decomposer_k_do_mg", self.decomposer_k_do_mg),
            ("decomposer_growth_yield", self.decomposer_growth_yield),
            (
                "decomposer_decay_rate_per_hour",
                self.decomposer_decay_rate_per_hour,
            ),
            (
                "aob_vmax_mg_n_per_g_per_hour",
                self.aob_vmax_mg_n_per_g_per_hour,
            ),
            ("aob_k_tan_mg", self.aob_k_tan_mg),
            ("aob_k_do_mg", self.aob_k_do_mg),
            ("aob_growth_yield", self.aob_growth_yield),
            ("aob_decay_rate_per_hour", self.aob_decay_rate_per_hour),
            (
                "nob_vmax_mg_n_per_g_per_hour",
                self.nob_vmax_mg_n_per_g_per_hour,
            ),
            ("nob_k_nitrite_mg", self.nob_k_nitrite_mg),
            ("nob_k_do_mg", self.nob_k_do_mg),
            ("nob_growth_yield", self.nob_growth_yield),
            ("nob_decay_rate_per_hour", self.nob_decay_rate_per_hour),
            ("comammox_vmax_fraction", self.comammox_vmax_fraction),
            ("comammox_k_tan_mg", self.comammox_k_tan_mg),
            ("comammox_k_do_mg", self.comammox_k_do_mg),
            ("comammox_growth_yield", self.comammox_growth_yield),
            (
                "comammox_decay_rate_per_hour",
                self.comammox_decay_rate_per_hour,
            ),
            ("o2_per_mg_n_nitrified", self.o2_per_mg_n_nitrified),
            (
                "alkalinity_meq_per_mg_n_nitrified",
                self.alkalinity_meq_per_mg_n_nitrified,
            ),
            (
                "plant_max_growth_rate_fast_stem_per_day",
                self.plant_max_growth_rate_fast_stem_per_day,
            ),
            (
                "plant_max_growth_rate_root_rosette_per_day",
                self.plant_max_growth_rate_root_rosette_per_day,
            ),
            (
                "plant_respiration_fraction_per_day",
                self.plant_respiration_fraction_per_day,
            ),
            (
                "plant_senescence_fraction_per_day",
                self.plant_senescence_fraction_per_day,
            ),
            (
                "plant_health_recovery_per_day",
                self.plant_health_recovery_per_day,
            ),
            (
                "plant_health_decline_per_day",
                self.plant_health_decline_per_day,
            ),
            (
                "plant_half_saturation_n_mg_total",
                self.plant_half_saturation_n_mg_total,
            ),
            (
                "plant_half_saturation_p_mg_total",
                self.plant_half_saturation_p_mg_total,
            ),
            (
                "plant_half_saturation_c_mg_total",
                self.plant_half_saturation_c_mg_total,
            ),
            (
                "plant_light_half_saturation",
                self.plant_light_half_saturation,
            ),
            ("plant_temp_optimum_c", self.plant_temp_optimum_c),
            ("plant_temp_sigma_c", self.plant_temp_sigma_c),
            (
                "plant_crowding_biomass_g_per_m2",
                self.plant_crowding_biomass_g_per_m2,
            ),
            (
                "algae_max_growth_rate_per_day",
                self.algae_max_growth_rate_per_day,
            ),
            (
                "periphyton_max_growth_rate_per_day",
                self.periphyton_max_growth_rate_per_day,
            ),
            (
                "algae_respiration_fraction_per_day",
                self.algae_respiration_fraction_per_day,
            ),
            (
                "algae_half_saturation_n_mg_total",
                self.algae_half_saturation_n_mg_total,
            ),
            (
                "algae_half_saturation_p_mg_total",
                self.algae_half_saturation_p_mg_total,
            ),
            (
                "algae_light_half_saturation",
                self.algae_light_half_saturation,
            ),
            ("algae_temp_optimum_c", self.algae_temp_optimum_c),
            ("algae_temp_sigma_c", self.algae_temp_sigma_c),
            (
                "periphyton_capacity_g_per_m2",
                self.periphyton_capacity_g_per_m2,
            ),
            (
                "algae_bloom_threshold_g_per_l",
                self.algae_bloom_threshold_g_per_l,
            ),
            (
                "algae_nuisance_biomass_g_per_m2",
                self.algae_nuisance_biomass_g_per_m2,
            ),
        ];
        for (name, value) in fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        // comammox_vmax_fraction must be <= 1.0 (spec requires lower vmax than AOB)
        if self.comammox_vmax_fraction > 1.0 {
            return Err(format!(
                "comammox_vmax_fraction must be <= 1.0, got {}",
                self.comammox_vmax_fraction
            ));
        }
        // Validate shrimp dynamics fields
        let shrimp_fields: &[(&str, f64)] = &[
            (
                "shrimp_base_mortality_per_day",
                self.shrimp_base_mortality_per_day,
            ),
            (
                "shrimp_stress_mortality_scale",
                self.shrimp_stress_mortality_scale,
            ),
            (
                "shrimp_juvenile_maturation_days",
                self.shrimp_juvenile_maturation_days,
            ),
            (
                "shrimp_periphyton_grazing_g_per_shrimp_per_day",
                self.shrimp_periphyton_grazing_g_per_shrimp_per_day,
            ),
            (
                "shrimp_condition_smoothing",
                self.shrimp_condition_smoothing,
            ),
            (
                "shrimp_assimilation_efficiency",
                self.shrimp_assimilation_efficiency,
            ),
            (
                "shrimp_respiration_fraction_of_assimilated",
                self.shrimp_respiration_fraction_of_assimilated,
            ),
            (
                "shrimp_excretion_fraction_of_assimilated",
                self.shrimp_excretion_fraction_of_assimilated,
            ),
            (
                "shrimp_growth_fraction_of_assimilated",
                self.shrimp_growth_fraction_of_assimilated,
            ),
            (
                "shrimp_o2_per_mg_c_respired",
                self.shrimp_o2_per_mg_c_respired,
            ),
        ];
        for (name, value) in shrimp_fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        if self.shrimp_assimilation_efficiency <= 0.0 || self.shrimp_assimilation_efficiency >= 1.0
        {
            return Err(format!(
                "shrimp_assimilation_efficiency must be in (0, 1), got {}",
                self.shrimp_assimilation_efficiency
            ));
        }
        if self.shrimp_respiration_fraction_of_assimilated > 1.0 {
            return Err(format!(
                "shrimp_respiration_fraction_of_assimilated must be <= 1.0, got {}",
                self.shrimp_respiration_fraction_of_assimilated
            ));
        }
        if self.shrimp_excretion_fraction_of_assimilated > 1.0 {
            return Err(format!(
                "shrimp_excretion_fraction_of_assimilated must be <= 1.0, got {}",
                self.shrimp_excretion_fraction_of_assimilated
            ));
        }
        if self.shrimp_growth_fraction_of_assimilated > 1.0 {
            return Err(format!(
                "shrimp_growth_fraction_of_assimilated must be <= 1.0, got {}",
                self.shrimp_growth_fraction_of_assimilated
            ));
        }
        if self.shrimp_o2_per_mg_c_respired <= 0.0 {
            return Err(format!(
                "shrimp_o2_per_mg_c_respired must be > 0.0, got {}",
                self.shrimp_o2_per_mg_c_respired
            ));
        }
        let shrimp_partition_sum = self.shrimp_respiration_fraction_of_assimilated
            + self.shrimp_excretion_fraction_of_assimilated
            + self.shrimp_growth_fraction_of_assimilated;
        if (shrimp_partition_sum - 1.0).abs() > SHRIMP_ROUTE_SUM_TOLERANCE {
            return Err(format!(
                "shrimp assimilated partition sum must equal 1.0, got {}",
                shrimp_partition_sum
            ));
        }
        // Validate microfauna turnover fields
        let microfauna_fields: &[(&str, f64)] = &[
            (
                "microfauna_mineralization_boost",
                self.microfauna_mineralization_boost,
            ),
            (
                "microfauna_periphyton_consumption",
                self.microfauna_periphyton_consumption,
            ),
            (
                "microfauna_population_smoothing",
                self.microfauna_population_smoothing,
            ),
            (
                "microfauna_shrimp_pressure_threshold",
                self.microfauna_shrimp_pressure_threshold,
            ),
        ];
        for (name, value) in microfauna_fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioPreset {
    pub id: String,
    pub name: String,
    pub source_water_id: String,
    pub substrate_ids: Vec<String>,
    pub plant_ids: Vec<String>,
    pub shrimp_profile_id: String,
    pub process_params_id: String,
    pub tank_length_cm: f64,
    pub tank_width_cm: f64,
    pub tank_height_cm: f64,
    pub fill_height_cm: f64,
    pub ambient_temp_c: f64,
    pub provenance: Option<Provenance>,
}

#[cfg(test)]
mod tests {
    use super::{ProcessParamsPreset, SourceWaterPreset};
    use tank_core::types::provenance::{
        check_param_range, format_param, ConfidenceLevel, ParamMeta,
    };

    fn default_process_preset() -> ProcessParamsPreset {
        toml::from_str(include_str!("../data/process/default.toml"))
            .expect("default process preset should parse")
    }

    fn source_preset(contents: &str) -> SourceWaterPreset {
        toml::from_str(contents).expect("source preset should parse")
    }

    // ---- Provenance acceptance-criteria tests ----

    /// AC 1: A parameter with full provenance metadata serializes and
    /// deserializes correctly via TOML.
    #[test]
    fn test_provenance_schema_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0

[param_meta.aob_k_tan_mg]
unit = "mg N/L"
source = "EPA 2013 ammonia criteria"
confidence = "literature"
valid_range = [0.1, 5.0]
notes = "K_s for AOB in biofilter context; may differ for free-living AOB"
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;
        let meta = preset
            .param_meta
            .get("aob_k_tan_mg")
            .expect("metadata should exist");
        assert_eq!(meta.unit.as_deref(), Some("mg N/L"));
        assert_eq!(meta.source.as_deref(), Some("EPA 2013 ammonia criteria"));
        assert_eq!(meta.confidence, Some(ConfidenceLevel::Literature));
        assert_eq!(meta.valid_range, Some([0.1, 5.0]));
        assert!(meta.notes.as_ref().unwrap().contains("AOB"));

        // Round-trip: serialize back to TOML and re-parse.
        let serialized = toml::to_string(&preset)?;
        let roundtrip: ProcessParamsPreset = toml::from_str(&serialized)?;
        assert_eq!(
            roundtrip.param_meta.get("aob_k_tan_mg"),
            preset.param_meta.get("aob_k_tan_mg")
        );
        Ok(())
    }

    /// AC 2: Provenance fields are all optional. A parameter with just
    /// value + unit loads fine.
    #[test]
    fn test_provenance_optional_fields() -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0

[param_meta.k_surface_w_per_m2_k]
unit = "W/(m²·K)"
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;
        let meta = preset
            .param_meta
            .get("k_surface_w_per_m2_k")
            .expect("metadata should exist");
        assert_eq!(meta.unit.as_deref(), Some("W/(m²·K)"));
        assert_eq!(meta.source, None);
        assert_eq!(meta.confidence, None);
        assert_eq!(meta.valid_range, None);
        assert_eq!(meta.notes, None);
        Ok(())
    }

    /// AC 3: Confidence levels are validated on load. Unknown confidence → error.
    #[test]
    fn test_confidence_levels_enum() -> Result<(), Box<dyn std::error::Error>> {
        // Valid levels all parse.
        for level in ["literature", "expert", "heuristic", "placeholder"] {
            let toml_str = format!(
                r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0

[param_meta.aob_k_tan_mg]
confidence = "{level}"
"#
            );
            let result: Result<ProcessParamsPreset, _> = toml::from_str(&toml_str);
            assert!(
                result.is_ok(),
                "confidence level '{level}' should parse, got: {:?}",
                result.err()
            );
        }

        // Unknown level → error.
        let bad_toml = r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0

[param_meta.aob_k_tan_mg]
confidence = "medium"
"#;
        let result: Result<ProcessParamsPreset, _> = toml::from_str(bad_toml);
        assert!(result.is_err(), "unknown confidence 'medium' should fail");
        Ok(())
    }

    /// AC 4: valid_range specified and value outside → warning (not error).
    #[test]
    fn test_valid_range_enforcement() -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0
aob_k_tan_mg = 10.0

[param_meta.aob_k_tan_mg]
unit = "mg N/L"
valid_range = [0.1, 5.0]
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;

        // Validation should still succeed (range check is a warning path).
        preset.validate().expect("validate should pass");

        // But range check should produce a warning.
        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].param_name, "aob_k_tan_mg");
        assert_eq!(warnings[0].value, 10.0);
        assert_eq!(warnings[0].range, [0.1, 5.0]);

        // In-range value → no warning.
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([0.1, 5.0]),
            notes: None,
        };
        assert!(check_param_range("aob_k_tan_mg", 0.5, &meta).is_none());
        Ok(())
    }

    /// AC 5: Existing TOML data files WITHOUT provenance metadata still load.
    #[test]
    fn test_backward_compatible_loading() -> Result<(), Box<dyn std::error::Error>> {
        // The shipped default.toml has no param_meta section.
        let preset = default_process_preset();
        assert!(
            preset.param_meta.is_empty(),
            "legacy file should load with empty param_meta"
        );
        preset
            .validate()
            .expect("legacy file should still validate");

        // Also verify a minimal TOML with only required fields.
        let minimal = r#"
id = "minimal"
name = "Minimal"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0
"#;
        let preset: ProcessParamsPreset = toml::from_str(minimal)?;
        assert!(preset.param_meta.is_empty());
        assert!(preset.provenance.is_none());
        Ok(())
    }

    /// AC 6: Provenance metadata can be formatted for developer display.
    #[test]
    fn test_provenance_display() {
        let meta = ParamMeta {
            unit: Some("mg N/L".into()),
            source: Some("EPA 2013".into()),
            confidence: Some(ConfidenceLevel::Literature),
            valid_range: None,
            notes: None,
        };
        let display = format_param("K_s_aob", 0.5, &meta);
        assert_eq!(display, "K_s_aob: 0.5 mg N/L [literature, EPA 2013]");

        // With all fields.
        let meta_full = ParamMeta {
            unit: Some("mg N/L".into()),
            source: Some("EPA 2013".into()),
            confidence: Some(ConfidenceLevel::Literature),
            valid_range: Some([0.1, 5.0]),
            notes: Some("biofilter context".into()),
        };
        let display_full = format_param("K_s_aob", 0.5, &meta_full);
        assert!(display_full.contains("K_s_aob: 0.5"));
        assert!(display_full.contains("mg N/L"));
        assert!(display_full.contains("literature"));
        assert!(display_full.contains("EPA 2013"));
        assert!(display_full.contains("range [0.1, 5.0]"));
        assert!(display_full.contains("notes: biofilter context"));
    }

    #[test]
    fn test_unknown_param_meta_keys_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
mineralization_rate_per_day = 0.15
nitrification_vmax = 0.08
reaeration_kla_base = 0.35
aeration_kla_boost = 0.9
background_bod_mg_o2_per_g_biomass_per_hour = 0.05
plant_photosynthesis_o2_mg_per_g_per_hour = 0.2
respiration_dic_rate_mg_c_per_g_per_hour = 0.0
photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0
k_surface_w_per_m2_k = 10.0
k_wall_w_per_m2_k = 5.0

[param_meta.aob_k_tan_typo]
unit = "mg N/L"
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;
        let err = preset.validate().expect_err("unknown param_meta key should fail");
        assert!(err.contains("unknown param_meta entries"));
        assert!(err.contains("aob_k_tan_typo"));
        Ok(())
    }

    // ---- Original tests ----

    #[test]
    fn process_preset_rejects_degenerate_shrimp_assimilation_efficiency() {
        let mut preset = default_process_preset();
        preset.shrimp_assimilation_efficiency = 0.0;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("shrimp_assimilation_efficiency"));
    }

    #[test]
    fn process_preset_rejects_invalid_shrimp_partition_sum() {
        let mut preset = default_process_preset();
        preset.shrimp_growth_fraction_of_assimilated = 0.25;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("partition sum"));
    }

    #[test]
    fn source_water_preset_rejects_out_of_range_carbonate_ph() {
        let mut preset = source_preset(include_str!("../data/source_water/moderate.toml"));
        preset.dic_mg_c_per_l = 10.0;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("carbonate-derived pH"));
    }

    #[test]
    fn source_water_preset_rejects_nonfinite_carbonate_fallback() {
        let mut preset = source_preset(include_str!("../data/source_water/moderate.toml"));
        preset.dic_mg_c_per_l = 1.0e308;
        preset.alkalinity_meq_per_l = 1.0;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("non-finite neutral fallback"));
    }

    #[test]
    fn source_water_preset_accepts_zero_dic_buffered_limit_case() {
        let mut preset = source_preset(include_str!("../data/source_water/moderate.toml"));
        preset.dic_mg_c_per_l = 0.0;
        preset.alkalinity_meq_per_l = 2.0;

        preset
            .validate()
            .expect("zero-DIC buffered source water should remain valid at the alkaline ceiling");
    }

    #[test]
    fn shipped_source_water_presets_have_distinct_carbonate_ph() {
        let soft = source_preset(include_str!("../data/source_water/soft_acidic.toml"));
        let moderate = source_preset(include_str!("../data/source_water/moderate.toml"));
        let hard = source_preset(include_str!("../data/source_water/hard_shrimp.toml"));
        let ro = source_preset(include_str!("../data/source_water/ro_like.toml"));

        for preset in [&soft, &moderate, &hard, &ro] {
            preset.validate().expect("shipped preset should validate");
        }

        let soft_ph = tank_core::systems::chemistry::preview_source_water_carbonate_equilibrium(
            soft.dic_mg_c_per_l,
            soft.alkalinity_meq_per_l,
            soft.temperature_c,
        )
        .ph;
        let moderate_ph =
            tank_core::systems::chemistry::preview_source_water_carbonate_equilibrium(
                moderate.dic_mg_c_per_l,
                moderate.alkalinity_meq_per_l,
                moderate.temperature_c,
            )
            .ph;
        let hard_ph = tank_core::systems::chemistry::preview_source_water_carbonate_equilibrium(
            hard.dic_mg_c_per_l,
            hard.alkalinity_meq_per_l,
            hard.temperature_c,
        )
        .ph;
        let ro_ph = tank_core::systems::chemistry::preview_source_water_carbonate_equilibrium(
            ro.dic_mg_c_per_l,
            ro.alkalinity_meq_per_l,
            ro.temperature_c,
        )
        .ph;

        assert!(
            soft_ph < moderate_ph,
            "soft acidic should stay below moderate"
        );
        assert!(
            moderate_ph < hard_ph,
            "moderate should stay below hard shrimp"
        );
        assert!(soft_ph > super::CARBONATE_PH_MIN && soft_ph < super::CARBONATE_PH_MAX);
        assert!(moderate_ph > super::CARBONATE_PH_MIN && moderate_ph < super::CARBONATE_PH_MAX);
        assert!(hard_ph > super::CARBONATE_PH_MIN && hard_ph < super::CARBONATE_PH_MAX);
        assert_eq!(ro_ph, 7.0);
    }
}
