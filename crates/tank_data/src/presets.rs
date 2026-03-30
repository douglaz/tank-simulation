use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tank_core::systems::chemistry::{
    validate_source_water_carbonate_profile, SourceWaterCarbonateValidationError, CARBONATE_PH_MAX,
    CARBONATE_PH_MIN,
};
use tank_core::types::{
    check_all_ranges, format_param, legacy_total_param_to_mg_per_l,
    legacy_total_param_to_mg_per_m2, ParamMeta, RangeWarning, ShrimpRuntimeParams,
    NITRIFICATION_ALK_MEQ_PER_MG_N,
};

const SHRIMP_ROUTE_SUM_TOLERANCE: f64 = 1e-9;
const CLOSED_LOOP_DEATH_FRACTION_TOLERANCE: f64 = 1e-9;

/// Preset-level provenance metadata (describes the preset file as a whole).
///
/// `confidence` intentionally remains free-form so whole-file summaries can
/// use coarse labels such as `"high"` or `"medium"`. Per-parameter
/// `param_meta.*.confidence` uses the validated `ConfidenceLevel` enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub source_title: Option<String>,
    pub source_year: Option<u16>,
    pub confidence: Option<String>,
    pub notes: Option<String>,
}

fn empty_param_meta() -> ParamMeta {
    ParamMeta {
        unit: None,
        source: None,
        confidence: None,
        valid_range: None,
        notes: None,
    }
}

trait ParamMetaPreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta>;
    fn lookup_param_value(&self, name: &str) -> Option<f64>;

    fn has_param_named(&self, name: &str) -> bool {
        self.lookup_param_value(name).is_some()
    }

    fn provenance_param_value(&self, name: &str) -> Option<f64> {
        self.lookup_param_value(name)
    }

    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        self.param_meta_map()
            .keys()
            .filter(|name| !self.has_param_named(name))
            .map(String::as_str)
            .collect()
    }

    fn format_param(&self, name: &str) -> Option<String> {
        let meta = self.param_meta_map().get(name);
        let value = match meta {
            Some(_) => self.provenance_param_value(name)?,
            None => self.lookup_param_value(name)?,
        };
        Some(match meta {
            Some(meta) => format_param(name, value, meta),
            None => format_param(name, value, &empty_param_meta()),
        })
    }

    fn check_ranges(&self) -> Vec<RangeWarning> {
        check_all_ranges(self.param_meta_map(), &|name| {
            self.provenance_param_value(name)
        })
    }
}

fn shrimp_runtime_default_param_value(name: &str) -> Option<f64> {
    let defaults = ShrimpRuntimeParams::default();
    match name {
        "body_nitrogen_mg_per_g_wet_mass" => Some(defaults.body_nitrogen_mg_per_g_wet_mass),
        "body_carbon_mg_per_g_wet_mass" => Some(defaults.body_carbon_mg_per_g_wet_mass),
        "juvenile_to_subadult_days" => Some(defaults.juvenile_to_subadult_days),
        "subadult_to_adult_days" => Some(defaults.subadult_to_adult_days),
        "juvenile_maturation_condition_threshold" => {
            Some(defaults.juvenile_maturation_condition_threshold)
        }
        "subadult_maturation_condition_threshold" => {
            Some(defaults.subadult_maturation_condition_threshold)
        }
        "base_molt_interval_days" => Some(defaults.base_molt_interval_days),
        "failed_molt_mortality_scale" => Some(defaults.failed_molt_mortality_scale),
        "failed_molt_accum_increase_per_failed_stage" => {
            Some(defaults.failed_molt_accum_increase_per_failed_stage)
        }
        "failed_molt_accum_recovery_per_successful_stage" => {
            Some(defaults.failed_molt_accum_recovery_per_successful_stage)
        }
        "failed_molt_stress_blend" => Some(defaults.failed_molt_stress_blend),
        "sub_adult_sensitivity" => Some(defaults.sub_adult_sensitivity),
        "low_temp_repro_ramp_width_c" => Some(defaults.low_temp_repro_ramp_width_c),
        "base_clutch_size" => Some(f64::from(defaults.base_clutch_size)),
        "min_clutch_condition" => Some(defaults.min_clutch_condition),
        "ca_min_mg_per_l" => Some(defaults.ca_min_mg_per_l),
        "mg_min_mg_per_l" => Some(defaults.mg_min_mg_per_l),
        "molt_reserve_fraction" => Some(defaults.molt_reserve_fraction),
        "molt_reserve_factor_floor" => Some(defaults.molt_reserve_factor_floor),
        "molt_condition_weight" => Some(defaults.molt_condition_weight),
        "molt_reserve_weight" => Some(defaults.molt_reserve_weight),
        "molt_failure_poor_condition_threshold" => {
            Some(defaults.molt_failure_poor_condition_threshold)
        }
        "molt_failure_instability_threshold" => Some(defaults.molt_failure_instability_threshold),
        "temp_condition_low_divisor_c" => Some(defaults.temp_condition_low_divisor_c),
        "temp_condition_high_divisor_c" => Some(defaults.temp_condition_high_divisor_c),
        "molt_stress_warning_threshold" => Some(defaults.molt_stress_warning_threshold),
        "molt_stress_mortality_threshold" => Some(defaults.molt_stress_mortality_threshold),
        "molt_stress_mineral_gh_weight" => Some(defaults.molt_stress_mineral_gh_weight),
        "molt_stress_mineral_ca_weight" => Some(defaults.molt_stress_mineral_ca_weight),
        "molt_stress_mineral_mg_weight" => Some(defaults.molt_stress_mineral_mg_weight),
        "molt_stress_pressure_mineral_weight" => Some(defaults.molt_stress_pressure_mineral_weight),
        "molt_stress_pressure_instability_weight" => {
            Some(defaults.molt_stress_pressure_instability_weight)
        }
        "molt_stress_pressure_condition_weight" => {
            Some(defaults.molt_stress_pressure_condition_weight)
        }
        "molt_stress_condition_midpoint" => Some(defaults.molt_stress_condition_midpoint),
        "molt_stress_pressure_thermal_weight" => Some(defaults.molt_stress_pressure_thermal_weight),
        "molt_stress_thermal_cap" => Some(defaults.molt_stress_thermal_cap),
        "molt_stress_pressure_hourly_weight" => Some(defaults.molt_stress_pressure_hourly_weight),
        "molt_stress_rise_smoothing" => Some(defaults.molt_stress_rise_smoothing),
        "molt_stress_decay_smoothing" => Some(defaults.molt_stress_decay_smoothing),
        "molt_gh_excess_penalty_divisor" => Some(defaults.molt_gh_excess_penalty_divisor),
        "molt_mineral_factor_floor" => Some(defaults.molt_mineral_factor_floor),
        "juvenile_molt_interval_days" => Some(defaults.juvenile_molt_interval_days),
        "sub_adult_molt_interval_days" => Some(defaults.sub_adult_molt_interval_days),
        "molt_success_threshold" => Some(defaults.molt_success_threshold),
        "critical_molt_gh_ratio" => Some(defaults.critical_molt_gh_ratio),
        "chloride_protection_factor" => Some(defaults.chloride_protection_factor),
        "nh3_stress_threshold_mg_n_per_l" => Some(defaults.nh3_stress_threshold_mg_n_per_l),
        "nh3_stress_response_scale" => Some(defaults.nh3_stress_response_scale),
        "density_repro_threshold_per_l" => Some(defaults.density_repro_threshold_per_l),
        "density_repro_half_suppression_per_l" => {
            Some(defaults.density_repro_half_suppression_per_l)
        }
        "tan_repro_threshold_mg_n_per_l" => Some(defaults.tan_repro_threshold_mg_n_per_l),
        "tan_repro_full_suppression_mg_n_per_l" => {
            Some(defaults.tan_repro_full_suppression_mg_n_per_l)
        }
        "no2_repro_threshold_mg_n_per_l" => Some(defaults.no2_repro_threshold_mg_n_per_l),
        "no2_repro_full_suppression_mg_n_per_l" => {
            Some(defaults.no2_repro_full_suppression_mg_n_per_l)
        }
        "egg_drop_temp_swing_c" => Some(defaults.egg_drop_temp_swing_c),
        "egg_drop_instability_threshold" => Some(defaults.egg_drop_instability_threshold),
        "egg_drop_max_probability" => Some(defaults.egg_drop_max_probability),
        "egg_oxygen_reference_mg_l" => Some(defaults.egg_oxygen_reference_mg_l),
        "reproductive_readiness_smoothing" => Some(defaults.reproductive_readiness_smoothing),
        "full_clutch_condition_threshold" => Some(defaults.full_clutch_condition_threshold),
        "instability_temp_swing_c" => Some(defaults.instability_temp_swing_c),
        "instability_ph_swing" => Some(defaults.instability_ph_swing),
        "instability_gh_swing_d" => Some(defaults.instability_gh_swing_d),
        "instability_do_swing_mg_l" => Some(defaults.instability_do_swing_mg_l),
        "instability_rise_smoothing" => Some(defaults.instability_rise_smoothing),
        "instability_decay_smoothing" => Some(defaults.instability_decay_smoothing),
        _ => None,
    }
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

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl SourceWaterPreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        ParamMetaPreset::unknown_param_meta_keys(self)
    }

    /// Look up a source-water parameter value by name. Returns `None` for
    /// unknown names.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        ParamMetaPreset::lookup_param_value(self, name)
    }

    pub fn format_param(&self, name: &str) -> Option<String> {
        ParamMetaPreset::format_param(self, name)
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        ParamMetaPreset::check_ranges(self)
    }

    /// Validates that all chemistry fields are finite and non-negative.
    pub fn validate(&self) -> Result<(), String> {
        let unknown_param_meta_keys = self.unknown_param_meta_keys();
        if !unknown_param_meta_keys.is_empty() {
            return Err(format!(
                "unknown param_meta entries: {}",
                unknown_param_meta_keys.join(", ")
            ));
        }

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

impl ParamMetaPreset for SourceWaterPreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta> {
        &self.param_meta
    }

    fn lookup_param_value(&self, name: &str) -> Option<f64> {
        match name {
            "temperature_c" => Some(self.temperature_c),
            "ammonia_mg_n_per_l" => Some(self.ammonia_mg_n_per_l),
            "nitrite_mg_n_per_l" => Some(self.nitrite_mg_n_per_l),
            "nitrate_mg_n_per_l" => Some(self.nitrate_mg_n_per_l),
            "phosphate_mg_p_per_l" => Some(self.phosphate_mg_p_per_l),
            "dic_mg_c_per_l" => Some(self.dic_mg_c_per_l),
            "doc_mg_c_per_l" => Some(self.doc_mg_c_per_l),
            "don_mg_n_per_l" => Some(self.don_mg_n_per_l),
            "alkalinity_meq_per_l" => Some(self.alkalinity_meq_per_l),
            "calcium_mg_per_l" => Some(self.calcium_mg_per_l),
            "magnesium_mg_per_l" => Some(self.magnesium_mg_per_l),
            "sodium_mg_per_l" => Some(self.sodium_mg_per_l),
            "potassium_mg_per_l" => Some(self.potassium_mg_per_l),
            "bicarbonate_mg_per_l" => Some(self.bicarbonate_mg_per_l),
            "chloride_mg_per_l" => Some(self.chloride_mg_per_l),
            "sulfate_mg_per_l" => Some(self.sulfate_mg_per_l),
            _ => None,
        }
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

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl SubstratePreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        ParamMetaPreset::unknown_param_meta_keys(self)
    }

    /// Look up a substrate parameter value by name. Returns `None` for
    /// unknown names.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        ParamMetaPreset::lookup_param_value(self, name)
    }

    pub fn format_param(&self, name: &str) -> Option<String> {
        ParamMetaPreset::format_param(self, name)
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        ParamMetaPreset::check_ranges(self)
    }

    /// Validates that all substrate fields are finite and non-negative.
    pub fn validate(&self) -> Result<(), String> {
        let unknown_param_meta_keys = self.unknown_param_meta_keys();
        if !unknown_param_meta_keys.is_empty() {
            return Err(format!(
                "unknown param_meta entries: {}",
                unknown_param_meta_keys.join(", ")
            ));
        }

        let fields: &[(&str, f64)] = &[
            ("depth_cm", self.depth_cm),
            (
                "cation_exchange_capacity_index",
                self.cation_exchange_capacity_index,
            ),
            ("detritus_trapping_index", self.detritus_trapping_index),
            ("colonizable_area_factor", self.colonizable_area_factor),
            ("low_oxygen_tendency_index", self.low_oxygen_tendency_index),
            ("grazing_surface_index", self.grazing_surface_index),
            (
                "nutrient_charge_mg_n_total",
                self.nutrient_charge_mg_n_total,
            ),
            (
                "nutrient_charge_mg_p_total",
                self.nutrient_charge_mg_p_total,
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
        Ok(())
    }
}

impl ParamMetaPreset for SubstratePreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta> {
        &self.param_meta
    }

    fn lookup_param_value(&self, name: &str) -> Option<f64> {
        match name {
            "depth_cm" => Some(self.depth_cm),
            "cation_exchange_capacity_index" => Some(self.cation_exchange_capacity_index),
            "detritus_trapping_index" => Some(self.detritus_trapping_index),
            "colonizable_area_factor" => Some(self.colonizable_area_factor),
            "low_oxygen_tendency_index" => Some(self.low_oxygen_tendency_index),
            "grazing_surface_index" => Some(self.grazing_surface_index),
            "nutrient_charge_mg_n_total" => Some(self.nutrient_charge_mg_n_total),
            "nutrient_charge_mg_p_total" => Some(self.nutrient_charge_mg_p_total),
            _ => None,
        }
    }
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

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl PlantPreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        ParamMetaPreset::unknown_param_meta_keys(self)
    }

    /// Look up a plant parameter value by name. Returns `None` for unknown
    /// names.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        ParamMetaPreset::lookup_param_value(self, name)
    }

    pub fn format_param(&self, name: &str) -> Option<String> {
        ParamMetaPreset::format_param(self, name)
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        ParamMetaPreset::check_ranges(self)
    }

    /// Validates that all plant fields are finite and non-negative.
    pub fn validate(&self) -> Result<(), String> {
        let unknown_param_meta_keys = self.unknown_param_meta_keys();
        if !unknown_param_meta_keys.is_empty() {
            return Err(format!(
                "unknown param_meta entries: {}",
                unknown_param_meta_keys.join(", ")
            ));
        }

        let fields: &[(&str, f64)] = &[
            ("growth_rate_index", self.growth_rate_index),
            ("water_column_uptake_bias", self.water_column_uptake_bias),
            ("substrate_uptake_bias", self.substrate_uptake_bias),
        ];
        for (name, value) in fields {
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

impl ParamMetaPreset for PlantPreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta> {
        &self.param_meta
    }

    fn lookup_param_value(&self, name: &str) -> Option<f64> {
        match name {
            "growth_rate_index" => Some(self.growth_rate_index),
            "water_column_uptake_bias" => Some(self.water_column_uptake_bias),
            "substrate_uptake_bias" => Some(self.substrate_uptake_bias),
            _ => None,
        }
    }
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
    #[serde(default)]
    pub low_temp_repro_ramp_width_c: Option<f64>,
    #[serde(default)]
    pub body_nitrogen_mg_per_g_wet_mass: Option<f64>,
    #[serde(default)]
    pub body_carbon_mg_per_g_wet_mass: Option<f64>,
    #[serde(default)]
    pub juvenile_to_subadult_days: Option<f64>,
    #[serde(default)]
    pub subadult_to_adult_days: Option<f64>,
    #[serde(default)]
    pub juvenile_maturation_condition_threshold: Option<f64>,
    #[serde(default)]
    pub subadult_maturation_condition_threshold: Option<f64>,
    #[serde(default)]
    pub base_molt_interval_days: Option<f64>,
    #[serde(default)]
    pub failed_molt_mortality_scale: Option<f64>,
    #[serde(default)]
    pub failed_molt_accum_increase_per_failed_stage: Option<f64>,
    #[serde(default)]
    pub failed_molt_accum_recovery_per_successful_stage: Option<f64>,
    #[serde(default)]
    pub failed_molt_stress_blend: Option<f64>,
    #[serde(default)]
    pub sub_adult_sensitivity: Option<f64>,
    #[serde(default)]
    pub base_clutch_size: Option<u32>,
    #[serde(default)]
    pub min_clutch_condition: Option<f64>,
    #[serde(default)]
    pub ca_min_mg_per_l: Option<f64>,
    #[serde(default)]
    pub mg_min_mg_per_l: Option<f64>,
    #[serde(default)]
    pub molt_reserve_fraction: Option<f64>,
    #[serde(default)]
    pub molt_reserve_factor_floor: Option<f64>,
    #[serde(default)]
    pub molt_condition_weight: Option<f64>,
    #[serde(default)]
    pub molt_reserve_weight: Option<f64>,
    #[serde(default)]
    pub molt_failure_poor_condition_threshold: Option<f64>,
    #[serde(default)]
    pub molt_failure_instability_threshold: Option<f64>,
    #[serde(default)]
    pub temp_condition_low_divisor_c: Option<f64>,
    #[serde(default)]
    pub temp_condition_high_divisor_c: Option<f64>,
    #[serde(default)]
    pub molt_stress_warning_threshold: Option<f64>,
    #[serde(default)]
    pub molt_stress_mortality_threshold: Option<f64>,
    #[serde(default)]
    pub molt_stress_mineral_gh_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_mineral_ca_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_mineral_mg_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_pressure_mineral_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_pressure_instability_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_pressure_condition_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_condition_midpoint: Option<f64>,
    #[serde(default)]
    pub molt_stress_pressure_thermal_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_thermal_cap: Option<f64>,
    #[serde(default)]
    pub molt_stress_pressure_hourly_weight: Option<f64>,
    #[serde(default)]
    pub molt_stress_rise_smoothing: Option<f64>,
    #[serde(default)]
    pub molt_stress_decay_smoothing: Option<f64>,
    #[serde(default)]
    pub molt_gh_excess_penalty_divisor: Option<f64>,
    #[serde(default)]
    pub molt_mineral_factor_floor: Option<f64>,
    #[serde(default)]
    pub juvenile_molt_interval_days: Option<f64>,
    #[serde(default)]
    pub sub_adult_molt_interval_days: Option<f64>,
    #[serde(default)]
    pub molt_success_threshold: Option<f64>,
    #[serde(default)]
    pub critical_molt_gh_ratio: Option<f64>,
    #[serde(default)]
    pub chloride_protection_factor: Option<f64>,
    #[serde(default)]
    pub nh3_stress_threshold_mg_n_per_l: Option<f64>,
    #[serde(default)]
    pub nh3_stress_response_scale: Option<f64>,
    #[serde(default)]
    pub density_repro_threshold_per_l: Option<f64>,
    #[serde(default)]
    pub density_repro_half_suppression_per_l: Option<f64>,
    #[serde(default)]
    pub tan_repro_threshold_mg_n_per_l: Option<f64>,
    #[serde(default)]
    pub tan_repro_full_suppression_mg_n_per_l: Option<f64>,
    #[serde(default)]
    pub no2_repro_threshold_mg_n_per_l: Option<f64>,
    #[serde(default)]
    pub no2_repro_full_suppression_mg_n_per_l: Option<f64>,
    #[serde(default)]
    pub egg_drop_temp_swing_c: Option<f64>,
    #[serde(default)]
    pub egg_drop_instability_threshold: Option<f64>,
    #[serde(default)]
    pub egg_drop_max_probability: Option<f64>,
    #[serde(default)]
    pub egg_oxygen_reference_mg_l: Option<f64>,
    #[serde(default)]
    pub reproductive_readiness_smoothing: Option<f64>,
    #[serde(default)]
    pub full_clutch_condition_threshold: Option<f64>,
    #[serde(default)]
    pub instability_temp_swing_c: Option<f64>,
    #[serde(default)]
    pub instability_ph_swing: Option<f64>,
    #[serde(default)]
    pub instability_gh_swing_d: Option<f64>,
    #[serde(default)]
    pub instability_do_swing_mg_l: Option<f64>,
    #[serde(default)]
    pub instability_rise_smoothing: Option<f64>,
    #[serde(default)]
    pub instability_decay_smoothing: Option<f64>,
    pub provenance: Option<Provenance>,

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl ShrimpPreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        ParamMetaPreset::unknown_param_meta_keys(self)
    }

    /// Look up a shrimp parameter value by name. Returns `None` for unknown
    /// names. Optional preset fields fall back to the runtime defaults used by
    /// `ShrimpRuntimeParams` so provenance on those defaults stays visible.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        ParamMetaPreset::lookup_param_value(self, name)
    }

    pub fn format_param(&self, name: &str) -> Option<String> {
        ParamMetaPreset::format_param(self, name)
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        ParamMetaPreset::check_ranges(self)
    }

    /// Validates that all shrimp species parameters are finite, non-negative,
    /// and logically consistent.
    pub fn validate(&self) -> Result<(), String> {
        let unknown_param_meta_keys = self.unknown_param_meta_keys();
        if !unknown_param_meta_keys.is_empty() {
            return Err(format!(
                "unknown param_meta entries: {}",
                unknown_param_meta_keys.join(", ")
            ));
        }

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
        for (name, value) in [
            (
                "body_nitrogen_mg_per_g_wet_mass",
                self.body_nitrogen_mg_per_g_wet_mass,
            ),
            (
                "body_carbon_mg_per_g_wet_mass",
                self.body_carbon_mg_per_g_wet_mass,
            ),
            ("juvenile_to_subadult_days", self.juvenile_to_subadult_days),
            ("subadult_to_adult_days", self.subadult_to_adult_days),
            (
                "juvenile_maturation_condition_threshold",
                self.juvenile_maturation_condition_threshold,
            ),
            (
                "subadult_maturation_condition_threshold",
                self.subadult_maturation_condition_threshold,
            ),
            ("base_molt_interval_days", self.base_molt_interval_days),
            (
                "failed_molt_mortality_scale",
                self.failed_molt_mortality_scale,
            ),
            (
                "failed_molt_accum_increase_per_failed_stage",
                self.failed_molt_accum_increase_per_failed_stage,
            ),
            (
                "failed_molt_accum_recovery_per_successful_stage",
                self.failed_molt_accum_recovery_per_successful_stage,
            ),
            ("failed_molt_stress_blend", self.failed_molt_stress_blend),
            ("sub_adult_sensitivity", self.sub_adult_sensitivity),
            (
                "low_temp_repro_ramp_width_c",
                self.low_temp_repro_ramp_width_c,
            ),
            ("min_clutch_condition", self.min_clutch_condition),
            ("ca_min_mg_per_l", self.ca_min_mg_per_l),
            ("mg_min_mg_per_l", self.mg_min_mg_per_l),
            ("molt_reserve_fraction", self.molt_reserve_fraction),
            ("molt_reserve_factor_floor", self.molt_reserve_factor_floor),
            ("molt_condition_weight", self.molt_condition_weight),
            ("molt_reserve_weight", self.molt_reserve_weight),
            (
                "molt_failure_poor_condition_threshold",
                self.molt_failure_poor_condition_threshold,
            ),
            (
                "molt_failure_instability_threshold",
                self.molt_failure_instability_threshold,
            ),
            (
                "temp_condition_low_divisor_c",
                self.temp_condition_low_divisor_c,
            ),
            (
                "temp_condition_high_divisor_c",
                self.temp_condition_high_divisor_c,
            ),
            (
                "molt_stress_warning_threshold",
                self.molt_stress_warning_threshold,
            ),
            (
                "molt_stress_mortality_threshold",
                self.molt_stress_mortality_threshold,
            ),
            (
                "molt_stress_mineral_gh_weight",
                self.molt_stress_mineral_gh_weight,
            ),
            (
                "molt_stress_mineral_ca_weight",
                self.molt_stress_mineral_ca_weight,
            ),
            (
                "molt_stress_mineral_mg_weight",
                self.molt_stress_mineral_mg_weight,
            ),
            (
                "molt_stress_pressure_mineral_weight",
                self.molt_stress_pressure_mineral_weight,
            ),
            (
                "molt_stress_pressure_instability_weight",
                self.molt_stress_pressure_instability_weight,
            ),
            (
                "molt_stress_pressure_condition_weight",
                self.molt_stress_pressure_condition_weight,
            ),
            (
                "molt_stress_condition_midpoint",
                self.molt_stress_condition_midpoint,
            ),
            (
                "molt_stress_pressure_thermal_weight",
                self.molt_stress_pressure_thermal_weight,
            ),
            (
                "molt_stress_thermal_cap",
                self.molt_stress_thermal_cap,
            ),
            (
                "molt_stress_pressure_hourly_weight",
                self.molt_stress_pressure_hourly_weight,
            ),
            (
                "molt_stress_rise_smoothing",
                self.molt_stress_rise_smoothing,
            ),
            (
                "molt_stress_decay_smoothing",
                self.molt_stress_decay_smoothing,
            ),
            (
                "molt_gh_excess_penalty_divisor",
                self.molt_gh_excess_penalty_divisor,
            ),
            ("molt_mineral_factor_floor", self.molt_mineral_factor_floor),
            (
                "juvenile_molt_interval_days",
                self.juvenile_molt_interval_days,
            ),
            (
                "sub_adult_molt_interval_days",
                self.sub_adult_molt_interval_days,
            ),
            ("molt_success_threshold", self.molt_success_threshold),
            ("critical_molt_gh_ratio", self.critical_molt_gh_ratio),
            (
                "chloride_protection_factor",
                self.chloride_protection_factor,
            ),
            (
                "nh3_stress_threshold_mg_n_per_l",
                self.nh3_stress_threshold_mg_n_per_l,
            ),
            (
                "nh3_stress_response_scale",
                self.nh3_stress_response_scale,
            ),
            (
                "density_repro_threshold_per_l",
                self.density_repro_threshold_per_l,
            ),
            (
                "density_repro_half_suppression_per_l",
                self.density_repro_half_suppression_per_l,
            ),
            (
                "tan_repro_threshold_mg_n_per_l",
                self.tan_repro_threshold_mg_n_per_l,
            ),
            (
                "tan_repro_full_suppression_mg_n_per_l",
                self.tan_repro_full_suppression_mg_n_per_l,
            ),
            (
                "no2_repro_threshold_mg_n_per_l",
                self.no2_repro_threshold_mg_n_per_l,
            ),
            (
                "no2_repro_full_suppression_mg_n_per_l",
                self.no2_repro_full_suppression_mg_n_per_l,
            ),
            ("egg_drop_temp_swing_c", self.egg_drop_temp_swing_c),
            (
                "egg_drop_instability_threshold",
                self.egg_drop_instability_threshold,
            ),
            ("egg_drop_max_probability", self.egg_drop_max_probability),
            ("egg_oxygen_reference_mg_l", self.egg_oxygen_reference_mg_l),
            (
                "reproductive_readiness_smoothing",
                self.reproductive_readiness_smoothing,
            ),
            (
                "full_clutch_condition_threshold",
                self.full_clutch_condition_threshold,
            ),
            (
                "nh3_stress_threshold_mg_n_per_l",
                self.nh3_stress_threshold_mg_n_per_l,
            ),
            ("nh3_stress_response_scale", self.nh3_stress_response_scale),
            ("instability_temp_swing_c", self.instability_temp_swing_c),
            ("instability_ph_swing", self.instability_ph_swing),
            ("instability_gh_swing_d", self.instability_gh_swing_d),
            ("instability_do_swing_mg_l", self.instability_do_swing_mg_l),
            (
                "instability_rise_smoothing",
                self.instability_rise_smoothing,
            ),
            (
                "instability_decay_smoothing",
                self.instability_decay_smoothing,
            ),
        ] {
            let Some(value) = value else {
                continue;
            };
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
        }
        for (name, value) in [
            (
                "body_nitrogen_mg_per_g_wet_mass",
                self.body_nitrogen_mg_per_g_wet_mass,
            ),
            (
                "body_carbon_mg_per_g_wet_mass",
                self.body_carbon_mg_per_g_wet_mass,
            ),
            ("juvenile_to_subadult_days", self.juvenile_to_subadult_days),
            ("subadult_to_adult_days", self.subadult_to_adult_days),
            ("base_molt_interval_days", self.base_molt_interval_days),
            (
                "low_temp_repro_ramp_width_c",
                self.low_temp_repro_ramp_width_c,
            ),
            (
                "temp_condition_low_divisor_c",
                self.temp_condition_low_divisor_c,
            ),
            (
                "temp_condition_high_divisor_c",
                self.temp_condition_high_divisor_c,
            ),
            ("ca_min_mg_per_l", self.ca_min_mg_per_l),
            ("mg_min_mg_per_l", self.mg_min_mg_per_l),
            ("molt_reserve_fraction", self.molt_reserve_fraction),
            (
                "molt_gh_excess_penalty_divisor",
                self.molt_gh_excess_penalty_divisor,
            ),
            (
                "juvenile_molt_interval_days",
                self.juvenile_molt_interval_days,
            ),
            (
                "sub_adult_molt_interval_days",
                self.sub_adult_molt_interval_days,
            ),
            (
                "density_repro_half_suppression_per_l",
                self.density_repro_half_suppression_per_l,
            ),
            (
                "tan_repro_full_suppression_mg_n_per_l",
                self.tan_repro_full_suppression_mg_n_per_l,
            ),
            (
                "no2_repro_full_suppression_mg_n_per_l",
                self.no2_repro_full_suppression_mg_n_per_l,
            ),
            ("egg_drop_temp_swing_c", self.egg_drop_temp_swing_c),
            ("egg_oxygen_reference_mg_l", self.egg_oxygen_reference_mg_l),
            ("instability_temp_swing_c", self.instability_temp_swing_c),
            ("instability_ph_swing", self.instability_ph_swing),
            ("instability_gh_swing_d", self.instability_gh_swing_d),
            ("instability_do_swing_mg_l", self.instability_do_swing_mg_l),
        ] {
            if let Some(value) = value {
                if value <= 0.0 {
                    return Err(format!("field `{name}` must be > 0.0, got {value}"));
                }
            }
        }
        for (name, value) in [
            (
                "juvenile_maturation_condition_threshold",
                self.juvenile_maturation_condition_threshold,
            ),
            (
                "subadult_maturation_condition_threshold",
                self.subadult_maturation_condition_threshold,
            ),
            ("min_clutch_condition", self.min_clutch_condition),
            ("molt_reserve_factor_floor", self.molt_reserve_factor_floor),
            ("molt_condition_weight", self.molt_condition_weight),
            ("molt_reserve_weight", self.molt_reserve_weight),
            ("failed_molt_stress_blend", self.failed_molt_stress_blend),
            (
                "molt_failure_poor_condition_threshold",
                self.molt_failure_poor_condition_threshold,
            ),
            (
                "molt_failure_instability_threshold",
                self.molt_failure_instability_threshold,
            ),
            (
                "temp_condition_low_divisor_c",
                self.temp_condition_low_divisor_c,
            ),
            (
                "temp_condition_high_divisor_c",
                self.temp_condition_high_divisor_c,
            ),
            (
                "molt_stress_warning_threshold",
                self.molt_stress_warning_threshold,
            ),
            (
                "molt_stress_mortality_threshold",
                self.molt_stress_mortality_threshold,
            ),
            (
                "molt_stress_mineral_gh_weight",
                self.molt_stress_mineral_gh_weight,
            ),
            (
                "molt_stress_mineral_ca_weight",
                self.molt_stress_mineral_ca_weight,
            ),
            (
                "molt_stress_mineral_mg_weight",
                self.molt_stress_mineral_mg_weight,
            ),
            (
                "molt_stress_pressure_mineral_weight",
                self.molt_stress_pressure_mineral_weight,
            ),
            (
                "molt_stress_pressure_instability_weight",
                self.molt_stress_pressure_instability_weight,
            ),
            (
                "molt_stress_pressure_condition_weight",
                self.molt_stress_pressure_condition_weight,
            ),
            (
                "molt_stress_condition_midpoint",
                self.molt_stress_condition_midpoint,
            ),
            (
                "molt_stress_pressure_thermal_weight",
                self.molt_stress_pressure_thermal_weight,
            ),
            ("molt_stress_thermal_cap", self.molt_stress_thermal_cap),
            (
                "molt_stress_pressure_hourly_weight",
                self.molt_stress_pressure_hourly_weight,
            ),
            (
                "molt_stress_rise_smoothing",
                self.molt_stress_rise_smoothing,
            ),
            (
                "molt_stress_decay_smoothing",
                self.molt_stress_decay_smoothing,
            ),
            ("molt_mineral_factor_floor", self.molt_mineral_factor_floor),
            ("molt_success_threshold", self.molt_success_threshold),
            ("critical_molt_gh_ratio", self.critical_molt_gh_ratio),
            (
                "egg_drop_instability_threshold",
                self.egg_drop_instability_threshold,
            ),
            ("egg_drop_max_probability", self.egg_drop_max_probability),
            (
                "reproductive_readiness_smoothing",
                self.reproductive_readiness_smoothing,
            ),
            (
                "full_clutch_condition_threshold",
                self.full_clutch_condition_threshold,
            ),
            (
                "instability_rise_smoothing",
                self.instability_rise_smoothing,
            ),
            (
                "instability_decay_smoothing",
                self.instability_decay_smoothing,
            ),
        ] {
            if let Some(value) = value {
                if value > 1.0 {
                    return Err(format!("field `{name}` must be <= 1.0, got {value}"));
                }
            }
        }
        let juvenile_molt_interval_days = self
            .juvenile_molt_interval_days
            .unwrap_or(ShrimpRuntimeParams::default().juvenile_molt_interval_days);
        let sub_adult_molt_interval_days = self
            .sub_adult_molt_interval_days
            .unwrap_or(ShrimpRuntimeParams::default().sub_adult_molt_interval_days);
        let base_molt_interval_days = self
            .base_molt_interval_days
            .unwrap_or(ShrimpRuntimeParams::default().base_molt_interval_days);
        if juvenile_molt_interval_days >= sub_adult_molt_interval_days {
            return Err(format!(
                "juvenile_molt_interval_days ({juvenile_molt_interval_days}) must be < sub_adult_molt_interval_days ({sub_adult_molt_interval_days})"
            ));
        }
        if sub_adult_molt_interval_days >= base_molt_interval_days {
            return Err(format!(
                "sub_adult_molt_interval_days ({sub_adult_molt_interval_days}) must be < base_molt_interval_days ({base_molt_interval_days})"
            ));
        }
        let molt_condition_weight = self
            .molt_condition_weight
            .unwrap_or(ShrimpRuntimeParams::default().molt_condition_weight);
        let molt_reserve_weight = self
            .molt_reserve_weight
            .unwrap_or(ShrimpRuntimeParams::default().molt_reserve_weight);
        if (molt_condition_weight + molt_reserve_weight - 1.0).abs() > 1.0e-9 {
            return Err(format!(
                "molt_condition_weight ({molt_condition_weight}) + molt_reserve_weight ({molt_reserve_weight}) must sum to 1.0"
            ));
        }
        let molt_stress_mineral_gh_weight = self
            .molt_stress_mineral_gh_weight
            .unwrap_or(ShrimpRuntimeParams::default().molt_stress_mineral_gh_weight);
        let molt_stress_mineral_ca_weight = self
            .molt_stress_mineral_ca_weight
            .unwrap_or(ShrimpRuntimeParams::default().molt_stress_mineral_ca_weight);
        let molt_stress_mineral_mg_weight = self
            .molt_stress_mineral_mg_weight
            .unwrap_or(ShrimpRuntimeParams::default().molt_stress_mineral_mg_weight);
        if (molt_stress_mineral_gh_weight
            + molt_stress_mineral_ca_weight
            + molt_stress_mineral_mg_weight
            - 1.0)
            .abs()
            > 1.0e-9
        {
            return Err(format!(
                "molt_stress_mineral_gh_weight ({molt_stress_mineral_gh_weight}) + molt_stress_mineral_ca_weight ({molt_stress_mineral_ca_weight}) + molt_stress_mineral_mg_weight ({molt_stress_mineral_mg_weight}) must sum to 1.0"
            ));
        }
        if let Some(base_clutch_size) = self.base_clutch_size {
            if base_clutch_size == 0 {
                return Err("base_clutch_size must be > 0".to_string());
            }
        }
        let density_repro_threshold_per_l = self
            .density_repro_threshold_per_l
            .unwrap_or(ShrimpRuntimeParams::default().density_repro_threshold_per_l);
        let density_repro_half_suppression_per_l = self
            .density_repro_half_suppression_per_l
            .unwrap_or(ShrimpRuntimeParams::default().density_repro_half_suppression_per_l);
        if density_repro_threshold_per_l >= density_repro_half_suppression_per_l {
            return Err(format!(
                "density_repro_threshold_per_l ({density_repro_threshold_per_l}) must be < density_repro_half_suppression_per_l ({density_repro_half_suppression_per_l})"
            ));
        }
        let tan_repro_threshold_mg_n_per_l = self
            .tan_repro_threshold_mg_n_per_l
            .unwrap_or(ShrimpRuntimeParams::default().tan_repro_threshold_mg_n_per_l);
        let tan_repro_full_suppression_mg_n_per_l = self
            .tan_repro_full_suppression_mg_n_per_l
            .unwrap_or(ShrimpRuntimeParams::default().tan_repro_full_suppression_mg_n_per_l);
        if tan_repro_threshold_mg_n_per_l >= tan_repro_full_suppression_mg_n_per_l {
            return Err(format!(
                "tan_repro_threshold_mg_n_per_l ({tan_repro_threshold_mg_n_per_l}) must be < tan_repro_full_suppression_mg_n_per_l ({tan_repro_full_suppression_mg_n_per_l})"
            ));
        }
        let no2_repro_threshold_mg_n_per_l = self
            .no2_repro_threshold_mg_n_per_l
            .unwrap_or(ShrimpRuntimeParams::default().no2_repro_threshold_mg_n_per_l);
        let no2_repro_full_suppression_mg_n_per_l = self
            .no2_repro_full_suppression_mg_n_per_l
            .unwrap_or(ShrimpRuntimeParams::default().no2_repro_full_suppression_mg_n_per_l);
        if no2_repro_threshold_mg_n_per_l >= no2_repro_full_suppression_mg_n_per_l {
            return Err(format!(
                "no2_repro_threshold_mg_n_per_l ({no2_repro_threshold_mg_n_per_l}) must be < no2_repro_full_suppression_mg_n_per_l ({no2_repro_full_suppression_mg_n_per_l})"
            ));
        }
        let min_clutch_condition = self
            .min_clutch_condition
            .unwrap_or(ShrimpRuntimeParams::default().min_clutch_condition);
        let full_clutch_condition_threshold = self
            .full_clutch_condition_threshold
            .unwrap_or(ShrimpRuntimeParams::default().full_clutch_condition_threshold);
        if min_clutch_condition >= full_clutch_condition_threshold {
            return Err(format!(
                "min_clutch_condition ({min_clutch_condition}) must be < full_clutch_condition_threshold ({full_clutch_condition_threshold})"
            ));
        }
        Ok(())
    }
}

impl ParamMetaPreset for ShrimpPreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta> {
        &self.param_meta
    }

    fn lookup_param_value(&self, name: &str) -> Option<f64> {
        match name {
            "optimal_temp_min_c" => Some(self.optimal_temp_min_c),
            "optimal_temp_max_c" => Some(self.optimal_temp_max_c),
            "gh_min_d" => Some(self.gh_min_d),
            "gh_max_d" => Some(self.gh_max_d),
            "base_spawn_rate" => Some(self.base_spawn_rate),
            "egg_duration_days" => Some(f64::from(self.egg_duration_days)),
            "hatch_success_base" => Some(self.hatch_success_base),
            "juvenile_sensitivity" => Some(self.juvenile_sensitivity),
            "high_temp_repro_penalty_start_c" => Some(self.high_temp_repro_penalty_start_c),
            "high_temp_repro_penalty_full_c" => Some(self.high_temp_repro_penalty_full_c),
            "low_temp_repro_ramp_width_c" => self
                .low_temp_repro_ramp_width_c
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "body_nitrogen_mg_per_g_wet_mass" => self
                .body_nitrogen_mg_per_g_wet_mass
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "body_carbon_mg_per_g_wet_mass" => self
                .body_carbon_mg_per_g_wet_mass
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "juvenile_to_subadult_days" => self
                .juvenile_to_subadult_days
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "subadult_to_adult_days" => self
                .subadult_to_adult_days
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "juvenile_maturation_condition_threshold" => self
                .juvenile_maturation_condition_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "subadult_maturation_condition_threshold" => self
                .subadult_maturation_condition_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "base_molt_interval_days" => self
                .base_molt_interval_days
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "failed_molt_mortality_scale" => self
                .failed_molt_mortality_scale
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "failed_molt_accum_increase_per_failed_stage" => self
                .failed_molt_accum_increase_per_failed_stage
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "failed_molt_accum_recovery_per_successful_stage" => self
                .failed_molt_accum_recovery_per_successful_stage
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "failed_molt_stress_blend" => self
                .failed_molt_stress_blend
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "sub_adult_sensitivity" => self
                .sub_adult_sensitivity
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "base_clutch_size" => self
                .base_clutch_size
                .map(f64::from)
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "min_clutch_condition" => self
                .min_clutch_condition
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "ca_min_mg_per_l" => self
                .ca_min_mg_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "mg_min_mg_per_l" => self
                .mg_min_mg_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_reserve_fraction" => self
                .molt_reserve_fraction
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_reserve_factor_floor" => self
                .molt_reserve_factor_floor
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_condition_weight" => self
                .molt_condition_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_reserve_weight" => self
                .molt_reserve_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_failure_poor_condition_threshold" => self
                .molt_failure_poor_condition_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_failure_instability_threshold" => self
                .molt_failure_instability_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "temp_condition_low_divisor_c" => self
                .temp_condition_low_divisor_c
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "temp_condition_high_divisor_c" => self
                .temp_condition_high_divisor_c
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_warning_threshold" => self
                .molt_stress_warning_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_mortality_threshold" => self
                .molt_stress_mortality_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_mineral_gh_weight" => self
                .molt_stress_mineral_gh_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_mineral_ca_weight" => self
                .molt_stress_mineral_ca_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_mineral_mg_weight" => self
                .molt_stress_mineral_mg_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_pressure_mineral_weight" => self
                .molt_stress_pressure_mineral_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_pressure_instability_weight" => self
                .molt_stress_pressure_instability_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_pressure_condition_weight" => self
                .molt_stress_pressure_condition_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_condition_midpoint" => self
                .molt_stress_condition_midpoint
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_pressure_thermal_weight" => self
                .molt_stress_pressure_thermal_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_thermal_cap" => self
                .molt_stress_thermal_cap
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_pressure_hourly_weight" => self
                .molt_stress_pressure_hourly_weight
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_rise_smoothing" => self
                .molt_stress_rise_smoothing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_stress_decay_smoothing" => self
                .molt_stress_decay_smoothing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_gh_excess_penalty_divisor" => self
                .molt_gh_excess_penalty_divisor
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_mineral_factor_floor" => self
                .molt_mineral_factor_floor
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "juvenile_molt_interval_days" => self
                .juvenile_molt_interval_days
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "sub_adult_molt_interval_days" => self
                .sub_adult_molt_interval_days
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "molt_success_threshold" => self
                .molt_success_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "critical_molt_gh_ratio" => self
                .critical_molt_gh_ratio
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "chloride_protection_factor" => self
                .chloride_protection_factor
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "nh3_stress_threshold_mg_n_per_l" => self
                .nh3_stress_threshold_mg_n_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "nh3_stress_response_scale" => self
                .nh3_stress_response_scale
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "density_repro_threshold_per_l" => self
                .density_repro_threshold_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "density_repro_half_suppression_per_l" => self
                .density_repro_half_suppression_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "tan_repro_threshold_mg_n_per_l" => self
                .tan_repro_threshold_mg_n_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "tan_repro_full_suppression_mg_n_per_l" => self
                .tan_repro_full_suppression_mg_n_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "no2_repro_threshold_mg_n_per_l" => self
                .no2_repro_threshold_mg_n_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "no2_repro_full_suppression_mg_n_per_l" => self
                .no2_repro_full_suppression_mg_n_per_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "egg_drop_temp_swing_c" => self
                .egg_drop_temp_swing_c
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "egg_drop_instability_threshold" => self
                .egg_drop_instability_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "egg_drop_max_probability" => self
                .egg_drop_max_probability
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "egg_oxygen_reference_mg_l" => self
                .egg_oxygen_reference_mg_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "reproductive_readiness_smoothing" => self
                .reproductive_readiness_smoothing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "full_clutch_condition_threshold" => self
                .full_clutch_condition_threshold
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_temp_swing_c" => self
                .instability_temp_swing_c
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_ph_swing" => self
                .instability_ph_swing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_gh_swing_d" => self
                .instability_gh_swing_d
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_do_swing_mg_l" => self
                .instability_do_swing_mg_l
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_rise_smoothing" => self
                .instability_rise_smoothing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            "instability_decay_smoothing" => self
                .instability_decay_smoothing
                .or_else(|| shrimp_runtime_default_param_value(name)),
            _ => None,
        }
    }

    fn has_param_named(&self, name: &str) -> bool {
        matches!(
            name,
            "optimal_temp_min_c"
                | "optimal_temp_max_c"
                | "gh_min_d"
                | "gh_max_d"
                | "base_spawn_rate"
                | "egg_duration_days"
                | "hatch_success_base"
                | "juvenile_sensitivity"
                | "high_temp_repro_penalty_start_c"
                | "high_temp_repro_penalty_full_c"
                | "low_temp_repro_ramp_width_c"
                | "body_nitrogen_mg_per_g_wet_mass"
                | "body_carbon_mg_per_g_wet_mass"
                | "juvenile_to_subadult_days"
                | "subadult_to_adult_days"
                | "juvenile_maturation_condition_threshold"
                | "subadult_maturation_condition_threshold"
                | "base_molt_interval_days"
                | "failed_molt_mortality_scale"
                | "failed_molt_accum_increase_per_failed_stage"
                | "failed_molt_accum_recovery_per_successful_stage"
                | "failed_molt_stress_blend"
                | "sub_adult_sensitivity"
                | "base_clutch_size"
                | "min_clutch_condition"
                | "ca_min_mg_per_l"
                | "mg_min_mg_per_l"
                | "molt_reserve_fraction"
                | "molt_reserve_factor_floor"
                | "molt_condition_weight"
                | "molt_reserve_weight"
                | "molt_failure_poor_condition_threshold"
                | "molt_failure_instability_threshold"
                | "temp_condition_low_divisor_c"
                | "temp_condition_high_divisor_c"
                | "molt_stress_warning_threshold"
                | "molt_stress_mortality_threshold"
                | "molt_stress_mineral_gh_weight"
                | "molt_stress_mineral_ca_weight"
                | "molt_stress_mineral_mg_weight"
                | "molt_stress_pressure_mineral_weight"
                | "molt_stress_pressure_instability_weight"
                | "molt_stress_pressure_condition_weight"
                | "molt_stress_condition_midpoint"
                | "molt_stress_pressure_thermal_weight"
                | "molt_stress_thermal_cap"
                | "molt_stress_pressure_hourly_weight"
                | "molt_stress_rise_smoothing"
                | "molt_stress_decay_smoothing"
                | "molt_gh_excess_penalty_divisor"
                | "molt_mineral_factor_floor"
                | "juvenile_molt_interval_days"
                | "sub_adult_molt_interval_days"
                | "molt_success_threshold"
                | "critical_molt_gh_ratio"
                | "chloride_protection_factor"
                | "nh3_stress_threshold_mg_n_per_l"
                | "nh3_stress_response_scale"
                | "density_repro_threshold_per_l"
                | "density_repro_half_suppression_per_l"
                | "tan_repro_threshold_mg_n_per_l"
                | "tan_repro_full_suppression_mg_n_per_l"
                | "no2_repro_threshold_mg_n_per_l"
                | "no2_repro_full_suppression_mg_n_per_l"
                | "egg_drop_temp_swing_c"
                | "egg_drop_instability_threshold"
                | "egg_drop_max_probability"
                | "egg_oxygen_reference_mg_l"
                | "reproductive_readiness_smoothing"
                | "full_clutch_condition_threshold"
                | "instability_temp_swing_c"
                | "instability_ph_swing"
                | "instability_gh_swing_d"
                | "instability_do_swing_mg_l"
                | "instability_rise_smoothing"
                | "instability_decay_smoothing"
        )
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
    #[serde(default = "default_decomposer_k_doc", alias = "decomposer_k_doc_mg")]
    pub decomposer_k_doc_mg_c_per_l: f64,
    #[serde(default = "default_decomposer_k_do", alias = "decomposer_k_do_mg")]
    pub decomposer_k_do_mg_per_l: f64,
    #[serde(default = "default_decomposer_growth_yield")]
    pub decomposer_growth_yield: f64,
    #[serde(default = "default_decomposer_decay_rate")]
    pub decomposer_decay_rate_per_hour: f64,

    // AOB kinetics
    #[serde(default = "default_aob_vmax")]
    pub aob_vmax_mg_n_per_g_per_hour: f64,
    #[serde(default = "default_aob_k_tan", alias = "aob_k_tan_mg_n_per_l")]
    pub aob_k_tan_mg_n_per_l: f64,
    #[serde(default = "default_aob_k_do", alias = "aob_k_do_mg")]
    pub aob_k_do_mg_per_l: f64,
    #[serde(default = "default_aob_growth_yield")]
    pub aob_growth_yield: f64,
    #[serde(default = "default_aob_decay_rate")]
    pub aob_decay_rate_per_hour: f64,

    // NOB kinetics
    #[serde(default = "default_nob_vmax")]
    pub nob_vmax_mg_n_per_g_per_hour: f64,
    #[serde(default = "default_nob_k_nitrite", alias = "nob_k_nitrite_mg")]
    pub nob_k_nitrite_mg_n_per_l: f64,
    #[serde(default = "default_nob_k_do", alias = "nob_k_do_mg")]
    pub nob_k_do_mg_per_l: f64,
    #[serde(default = "default_nob_growth_yield")]
    pub nob_growth_yield: f64,
    #[serde(default = "default_nob_decay_rate")]
    pub nob_decay_rate_per_hour: f64,

    // Comammox kinetics
    #[serde(default = "default_comammox_vmax_fraction")]
    pub comammox_vmax_fraction: f64,
    #[serde(default = "default_comammox_k_tan", alias = "comammox_k_tan_mg")]
    pub comammox_k_tan_mg_n_per_l: f64,
    #[serde(default = "default_comammox_k_do", alias = "comammox_k_do_mg")]
    pub comammox_k_do_mg_per_l: f64,
    #[serde(default = "default_comammox_growth_yield")]
    pub comammox_growth_yield: f64,
    #[serde(default = "default_comammox_decay_rate")]
    pub comammox_decay_rate_per_hour: f64,

    // Denitrification kinetics
    #[serde(default = "default_denitrification_vmax")]
    pub denitrification_vmax_mg_n_per_l_per_hour: f64,
    #[serde(default = "default_denitrification_k_no3")]
    pub denitrification_k_no3_mg_n_per_l: f64,
    #[serde(default = "default_denitrification_k_doc")]
    pub denitrification_k_doc_mg_c_per_l: f64,
    #[serde(default = "default_denitrification_pore_water_mixing_factor")]
    pub denitrification_pore_water_mixing_factor: f64,
    #[serde(default = "default_denitrification_activity_maturation_days")]
    pub denitrification_activity_maturation_days: f64,

    // Root-zone oxygenation (radial oxygen loss)
    #[serde(default = "default_rol_rate_cm_per_g")]
    pub rol_rate_cm_per_g: f64,

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
    pub plant_half_saturation_n_mg_n_per_l: f64,
    #[serde(default = "default_plant_half_sat_p")]
    pub plant_half_saturation_p_mg_p_per_l: f64,
    #[serde(default = "default_plant_half_sat_c")]
    pub plant_half_saturation_c_mg_c_per_l: f64,
    #[serde(default = "default_plant_half_sat_n_substrate")]
    pub plant_half_saturation_n_substrate_mg_n_per_m2: f64,
    #[serde(default = "default_plant_half_sat_p_substrate")]
    pub plant_half_saturation_p_substrate_mg_p_per_m2: f64,
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
    #[serde(
        default = "default_algae_half_sat_n",
        alias = "algae_half_saturation_n_mg_total"
    )]
    pub algae_half_saturation_n_mg_n_per_l: f64,
    #[serde(
        default = "default_algae_half_sat_p",
        alias = "algae_half_saturation_p_mg_total"
    )]
    pub algae_half_saturation_p_mg_p_per_l: f64,
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

    #[serde(default = "default_death_biomass_to_detritus_fraction")]
    pub death_biomass_to_detritus_fraction: f64,

    // -- Microfauna turnover --
    #[serde(default = "default_microfauna_mineralization_boost")]
    pub microfauna_mineralization_boost: f64,
    #[serde(default = "default_microfauna_periphyton_consumption")]
    pub microfauna_periphyton_consumption: f64,
    #[serde(default = "default_microfauna_population_smoothing")]
    pub microfauna_population_smoothing: f64,
    #[serde(default = "default_microfauna_shrimp_pressure_threshold")]
    pub microfauna_shrimp_pressure_threshold: f64,

    #[serde(default = "default_microfauna_assimilation_efficiency")]
    pub microfauna_assimilation_efficiency: f64,
    #[serde(default = "default_microfauna_respiration_fraction")]
    pub microfauna_respiration_fraction_of_assimilated: f64,
    #[serde(default = "default_microfauna_excretion_fraction")]
    pub microfauna_excretion_fraction_of_assimilated: f64,
    #[serde(default = "default_microfauna_growth_fraction")]
    pub microfauna_growth_fraction_of_assimilated: f64,

    pub provenance: Option<Provenance>,

    /// Per-parameter provenance metadata keyed by parameter name.
    /// Missing entries mean no provenance has been attached yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_meta: BTreeMap<String, ParamMeta>,
}

impl ProcessParamsPreset {
    fn unknown_param_meta_keys(&self) -> Vec<&str> {
        ParamMetaPreset::unknown_param_meta_keys(self)
    }

    /// Look up a parameter value by name. Returns `None` for unknown names.
    pub fn param_value(&self, name: &str) -> Option<f64> {
        ParamMetaPreset::lookup_param_value(self, name)
    }

    pub fn format_param(&self, name: &str) -> Option<String> {
        ParamMetaPreset::format_param(self, name)
    }

    /// Check all param_meta entries with valid_range against current values.
    /// Returns warnings for out-of-range values (never errors).
    pub fn check_ranges(&self) -> Vec<RangeWarning> {
        ParamMetaPreset::check_ranges(self)
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
    3.0
}
fn default_decomposer_k_do() -> f64 {
    0.5
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
    1.0
}
fn default_aob_k_do() -> f64 {
    0.5
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
    0.5
}
fn default_nob_k_do() -> f64 {
    0.8
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
    0.2
}
fn default_comammox_k_do() -> f64 {
    0.6
}
fn default_comammox_growth_yield() -> f64 {
    0.03
}
fn default_comammox_decay_rate() -> f64 {
    0.004
}
fn default_denitrification_vmax() -> f64 {
    0.1
}
fn default_denitrification_k_no3() -> f64 {
    2.0
}
fn default_denitrification_k_doc() -> f64 {
    5.0
}
fn default_denitrification_pore_water_mixing_factor() -> f64 {
    0.5
}
fn default_denitrification_activity_maturation_days() -> f64 {
    60.0
}
fn default_rol_rate_cm_per_g() -> f64 {
    0.15
}
fn default_o2_per_mg_n() -> f64 {
    4.57
}
fn default_alk_per_mg_n() -> f64 {
    NITRIFICATION_ALK_MEQ_PER_MG_N
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
    0.4
}
fn default_plant_half_sat_p() -> f64 {
    0.06
}
fn default_plant_half_sat_c() -> f64 {
    1.0
}
fn default_plant_half_sat_n_substrate() -> f64 {
    80.0
}
fn default_plant_half_sat_p_substrate() -> f64 {
    12.0
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
    0.25
}
fn default_algae_half_sat_p() -> f64 {
    0.04
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
fn default_death_biomass_to_detritus_fraction() -> f64 {
    1.0
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
fn default_microfauna_assimilation_efficiency() -> f64 {
    0.50
}
fn default_microfauna_respiration_fraction() -> f64 {
    0.70
}
fn default_microfauna_excretion_fraction() -> f64 {
    0.10
}
fn default_microfauna_growth_fraction() -> f64 {
    0.20
}

pub(crate) fn parse_process_params_preset(
    raw: &str,
) -> Result<ProcessParamsPreset, toml::de::Error> {
    let mut value: toml::Value = toml::from_str(raw)?;
    normalize_legacy_process_params_preset(&mut value);
    value.try_into()
}

fn normalize_legacy_process_params_preset(value: &mut toml::Value) {
    let Some(table) = value.as_table_mut() else {
        return;
    };

    let legacy_n = table.remove("plant_half_saturation_n_mg_total");
    if let Some(legacy_n) = legacy_n.as_ref() {
        if !table.contains_key("plant_half_saturation_n_mg_n_per_l") {
            insert_legacy_concentration_param_value(
                table,
                "plant_half_saturation_n_mg_n_per_l",
                legacy_n,
                legacy_total_param_to_mg_per_l,
            );
        }
        if !table.contains_key("plant_half_saturation_n_substrate_mg_n_per_m2") {
            if let Some(legacy_n) = toml_numeric_value(legacy_n) {
                table.insert(
                    "plant_half_saturation_n_substrate_mg_n_per_m2".to_string(),
                    toml::Value::from(legacy_total_param_to_mg_per_m2(legacy_n)),
                );
            }
        }
    }

    let legacy_p = table.remove("plant_half_saturation_p_mg_total");
    if let Some(legacy_p) = legacy_p.as_ref() {
        if !table.contains_key("plant_half_saturation_p_mg_p_per_l") {
            insert_legacy_concentration_param_value(
                table,
                "plant_half_saturation_p_mg_p_per_l",
                legacy_p,
                legacy_total_param_to_mg_per_l,
            );
        }
        if !table.contains_key("plant_half_saturation_p_substrate_mg_p_per_m2") {
            if let Some(legacy_p) = toml_numeric_value(legacy_p) {
                table.insert(
                    "plant_half_saturation_p_substrate_mg_p_per_m2".to_string(),
                    toml::Value::from(legacy_total_param_to_mg_per_m2(legacy_p)),
                );
            }
        }
    }

    if let Some(legacy_c) = table.remove("plant_half_saturation_c_mg_total").as_ref() {
        if !table.contains_key("plant_half_saturation_c_mg_c_per_l") {
            insert_legacy_concentration_param_value(
                table,
                "plant_half_saturation_c_mg_c_per_l",
                legacy_c,
                legacy_total_param_to_mg_per_l,
            );
        }
    }

    if let Some(legacy_n) = table.remove("algae_half_saturation_n_mg_total").as_ref() {
        if !table.contains_key("algae_half_saturation_n_mg_n_per_l") {
            insert_legacy_concentration_param_value(
                table,
                "algae_half_saturation_n_mg_n_per_l",
                legacy_n,
                legacy_total_param_to_mg_per_l,
            );
        }
    }

    if let Some(legacy_p) = table.remove("algae_half_saturation_p_mg_total").as_ref() {
        if !table.contains_key("algae_half_saturation_p_mg_p_per_l") {
            insert_legacy_concentration_param_value(
                table,
                "algae_half_saturation_p_mg_p_per_l",
                legacy_p,
                legacy_total_param_to_mg_per_l,
            );
        }
    }

    // Nitrogen-cycle Ks parameters: convert legacy total-style values
    // (scaled for 20L reference) to native concentration (mg/L).
    let nitrogen_legacy_renames: &[(&str, &str)] = &[
        ("decomposer_k_doc_mg", "decomposer_k_doc_mg_c_per_l"),
        ("decomposer_k_do_mg", "decomposer_k_do_mg_per_l"),
        ("aob_k_tan_mg", "aob_k_tan_mg_n_per_l"),
        ("aob_k_do_mg", "aob_k_do_mg_per_l"),
        ("nob_k_nitrite_mg", "nob_k_nitrite_mg_n_per_l"),
        ("nob_k_do_mg", "nob_k_do_mg_per_l"),
        ("comammox_k_tan_mg", "comammox_k_tan_mg_n_per_l"),
        ("comammox_k_do_mg", "comammox_k_do_mg_per_l"),
    ];
    for &(legacy, canonical) in nitrogen_legacy_renames {
        if let Some(legacy_val) = table.remove(legacy) {
            if !table.contains_key(canonical) {
                insert_legacy_concentration_param_value(
                    table,
                    canonical,
                    &legacy_val,
                    legacy_total_param_to_mg_per_l,
                );
            }
        }
    }

    if let Some(param_meta) = table
        .get_mut("param_meta")
        .and_then(toml::Value::as_table_mut)
    {
        move_legacy_process_param_meta_key(
            param_meta,
            "plant_half_saturation_n_mg_total",
            "plant_half_saturation_n_mg_n_per_l",
        );
        move_legacy_process_param_meta_key(
            param_meta,
            "plant_half_saturation_p_mg_total",
            "plant_half_saturation_p_mg_p_per_l",
        );
        move_legacy_process_param_meta_key(
            param_meta,
            "plant_half_saturation_c_mg_total",
            "plant_half_saturation_c_mg_c_per_l",
        );
        move_legacy_process_param_meta_key(
            param_meta,
            "algae_half_saturation_n_mg_total",
            "algae_half_saturation_n_mg_n_per_l",
        );
        move_legacy_process_param_meta_key(
            param_meta,
            "algae_half_saturation_p_mg_total",
            "algae_half_saturation_p_mg_p_per_l",
        );
        // Also move nitrogen-cycle param_meta keys.
        for &(legacy, canonical) in nitrogen_legacy_renames {
            move_legacy_process_param_meta_key(param_meta, legacy, canonical);
        }
    }
}

fn insert_legacy_concentration_param_value(
    table: &mut toml::map::Map<String, toml::Value>,
    canonical_key: &str,
    legacy_value: &toml::Value,
    transform: fn(f64) -> f64,
) {
    let canonical_value = toml_numeric_value(legacy_value)
        .map(transform)
        .map(toml::Value::from)
        .unwrap_or_else(|| legacy_value.clone());
    table.insert(canonical_key.to_string(), canonical_value);
}

fn toml_numeric_value(value: &toml::Value) -> Option<f64> {
    match value {
        toml::Value::Float(value) => Some(*value),
        toml::Value::Integer(value) => Some(*value as f64),
        _ => None,
    }
}

fn move_legacy_process_param_meta_key(
    table: &mut toml::map::Map<String, toml::Value>,
    legacy_key: &'static str,
    canonical_key: &'static str,
) {
    let Some(legacy_value) = table.remove(legacy_key) else {
        return;
    };
    table
        .entry(canonical_key.to_string())
        .or_insert(legacy_value);
}

/// Legacy process `_mg` / `_mg_total` keys are normalized onto canonical
/// concentration fields during preset parsing, so provenance display only
/// needs to surface the already-normalized canonical values.
fn normalize_process_param_for_provenance(_name: &str, value: f64, _unit: Option<&str>) -> f64 {
    value
}

impl ParamMetaPreset for ProcessParamsPreset {
    fn param_meta_map(&self) -> &BTreeMap<String, ParamMeta> {
        &self.param_meta
    }

    fn lookup_param_value(&self, name: &str) -> Option<f64> {
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
            "decomposer_k_doc_mg_c_per_l" => Some(self.decomposer_k_doc_mg_c_per_l),
            "decomposer_k_do_mg_per_l" => Some(self.decomposer_k_do_mg_per_l),
            "decomposer_growth_yield" => Some(self.decomposer_growth_yield),
            "decomposer_decay_rate_per_hour" => Some(self.decomposer_decay_rate_per_hour),
            "aob_vmax_mg_n_per_g_per_hour" => Some(self.aob_vmax_mg_n_per_g_per_hour),
            "aob_k_tan_mg_n_per_l" => Some(self.aob_k_tan_mg_n_per_l),
            "aob_k_do_mg_per_l" => Some(self.aob_k_do_mg_per_l),
            "aob_growth_yield" => Some(self.aob_growth_yield),
            "aob_decay_rate_per_hour" => Some(self.aob_decay_rate_per_hour),
            "nob_vmax_mg_n_per_g_per_hour" => Some(self.nob_vmax_mg_n_per_g_per_hour),
            "nob_k_nitrite_mg_n_per_l" => Some(self.nob_k_nitrite_mg_n_per_l),
            "nob_k_do_mg_per_l" => Some(self.nob_k_do_mg_per_l),
            "nob_growth_yield" => Some(self.nob_growth_yield),
            "nob_decay_rate_per_hour" => Some(self.nob_decay_rate_per_hour),
            "comammox_vmax_fraction" => Some(self.comammox_vmax_fraction),
            "comammox_k_tan_mg_n_per_l" => Some(self.comammox_k_tan_mg_n_per_l),
            "comammox_k_do_mg_per_l" => Some(self.comammox_k_do_mg_per_l),
            "comammox_growth_yield" => Some(self.comammox_growth_yield),
            "comammox_decay_rate_per_hour" => Some(self.comammox_decay_rate_per_hour),
            "denitrification_vmax_mg_n_per_l_per_hour" => {
                Some(self.denitrification_vmax_mg_n_per_l_per_hour)
            }
            "denitrification_k_no3_mg_n_per_l" => Some(self.denitrification_k_no3_mg_n_per_l),
            "denitrification_k_doc_mg_c_per_l" => Some(self.denitrification_k_doc_mg_c_per_l),
            "denitrification_pore_water_mixing_factor" => {
                Some(self.denitrification_pore_water_mixing_factor)
            }
            "denitrification_activity_maturation_days" => {
                Some(self.denitrification_activity_maturation_days)
            }
            "rol_rate_cm_per_g" => Some(self.rol_rate_cm_per_g),
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
            "plant_half_saturation_n_mg_n_per_l" => Some(self.plant_half_saturation_n_mg_n_per_l),
            "plant_half_saturation_p_mg_p_per_l" => Some(self.plant_half_saturation_p_mg_p_per_l),
            "plant_half_saturation_c_mg_c_per_l" => Some(self.plant_half_saturation_c_mg_c_per_l),
            "plant_half_saturation_n_substrate_mg_n_per_m2" => {
                Some(self.plant_half_saturation_n_substrate_mg_n_per_m2)
            }
            "plant_half_saturation_p_substrate_mg_p_per_m2" => {
                Some(self.plant_half_saturation_p_substrate_mg_p_per_m2)
            }
            "plant_light_half_saturation" => Some(self.plant_light_half_saturation),
            "plant_temp_optimum_c" => Some(self.plant_temp_optimum_c),
            "plant_temp_sigma_c" => Some(self.plant_temp_sigma_c),
            "plant_crowding_biomass_g_per_m2" => Some(self.plant_crowding_biomass_g_per_m2),
            "algae_max_growth_rate_per_day" => Some(self.algae_max_growth_rate_per_day),
            "periphyton_max_growth_rate_per_day" => Some(self.periphyton_max_growth_rate_per_day),
            "algae_respiration_fraction_per_day" => Some(self.algae_respiration_fraction_per_day),
            "algae_half_saturation_n_mg_n_per_l" => Some(self.algae_half_saturation_n_mg_n_per_l),
            "algae_half_saturation_p_mg_p_per_l" => Some(self.algae_half_saturation_p_mg_p_per_l),
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
            "death_biomass_to_detritus_fraction" => Some(self.death_biomass_to_detritus_fraction),
            "microfauna_mineralization_boost" => Some(self.microfauna_mineralization_boost),
            "microfauna_periphyton_consumption" => Some(self.microfauna_periphyton_consumption),
            "microfauna_population_smoothing" => Some(self.microfauna_population_smoothing),
            "microfauna_shrimp_pressure_threshold" => {
                Some(self.microfauna_shrimp_pressure_threshold)
            }
            "microfauna_assimilation_efficiency" => Some(self.microfauna_assimilation_efficiency),
            "microfauna_respiration_fraction_of_assimilated" => {
                Some(self.microfauna_respiration_fraction_of_assimilated)
            }
            "microfauna_excretion_fraction_of_assimilated" => {
                Some(self.microfauna_excretion_fraction_of_assimilated)
            }
            "microfauna_growth_fraction_of_assimilated" => {
                Some(self.microfauna_growth_fraction_of_assimilated)
            }
            _ => None,
        }
    }

    fn provenance_param_value(&self, name: &str) -> Option<f64> {
        let value = self.lookup_param_value(name)?;
        let unit = self
            .param_meta_map()
            .get(name)
            .and_then(|meta| meta.unit.as_deref());
        Some(normalize_process_param_for_provenance(name, value, unit))
    }
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
            (
                "respiration_dic_rate_mg_c_per_g_per_hour",
                self.respiration_dic_rate_mg_c_per_g_per_hour,
            ),
            (
                "photosynthesis_dic_rate_mg_c_per_g_per_hour",
                self.photosynthesis_dic_rate_mg_c_per_g_per_hour,
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
            (
                "decomposer_k_doc_mg_c_per_l",
                self.decomposer_k_doc_mg_c_per_l,
            ),
            ("decomposer_k_do_mg_per_l", self.decomposer_k_do_mg_per_l),
            ("decomposer_growth_yield", self.decomposer_growth_yield),
            (
                "decomposer_decay_rate_per_hour",
                self.decomposer_decay_rate_per_hour,
            ),
            (
                "aob_vmax_mg_n_per_g_per_hour",
                self.aob_vmax_mg_n_per_g_per_hour,
            ),
            ("aob_k_tan_mg_n_per_l", self.aob_k_tan_mg_n_per_l),
            ("aob_k_do_mg_per_l", self.aob_k_do_mg_per_l),
            ("aob_growth_yield", self.aob_growth_yield),
            ("aob_decay_rate_per_hour", self.aob_decay_rate_per_hour),
            (
                "nob_vmax_mg_n_per_g_per_hour",
                self.nob_vmax_mg_n_per_g_per_hour,
            ),
            ("nob_k_nitrite_mg_n_per_l", self.nob_k_nitrite_mg_n_per_l),
            ("nob_k_do_mg_per_l", self.nob_k_do_mg_per_l),
            ("nob_growth_yield", self.nob_growth_yield),
            ("nob_decay_rate_per_hour", self.nob_decay_rate_per_hour),
            ("comammox_vmax_fraction", self.comammox_vmax_fraction),
            ("comammox_k_tan_mg_n_per_l", self.comammox_k_tan_mg_n_per_l),
            ("comammox_k_do_mg_per_l", self.comammox_k_do_mg_per_l),
            ("comammox_growth_yield", self.comammox_growth_yield),
            (
                "comammox_decay_rate_per_hour",
                self.comammox_decay_rate_per_hour,
            ),
            (
                "denitrification_vmax_mg_n_per_l_per_hour",
                self.denitrification_vmax_mg_n_per_l_per_hour,
            ),
            (
                "denitrification_k_no3_mg_n_per_l",
                self.denitrification_k_no3_mg_n_per_l,
            ),
            (
                "denitrification_k_doc_mg_c_per_l",
                self.denitrification_k_doc_mg_c_per_l,
            ),
            (
                "denitrification_pore_water_mixing_factor",
                self.denitrification_pore_water_mixing_factor,
            ),
            (
                "denitrification_activity_maturation_days",
                self.denitrification_activity_maturation_days,
            ),
            ("rol_rate_cm_per_g", self.rol_rate_cm_per_g),
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
                "plant_half_saturation_n_mg_n_per_l",
                self.plant_half_saturation_n_mg_n_per_l,
            ),
            (
                "plant_half_saturation_p_mg_p_per_l",
                self.plant_half_saturation_p_mg_p_per_l,
            ),
            (
                "plant_half_saturation_c_mg_c_per_l",
                self.plant_half_saturation_c_mg_c_per_l,
            ),
            (
                "plant_half_saturation_n_substrate_mg_n_per_m2",
                self.plant_half_saturation_n_substrate_mg_n_per_m2,
            ),
            (
                "plant_half_saturation_p_substrate_mg_p_per_m2",
                self.plant_half_saturation_p_substrate_mg_p_per_m2,
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
                "algae_half_saturation_n_mg_n_per_l",
                self.algae_half_saturation_n_mg_n_per_l,
            ),
            (
                "algae_half_saturation_p_mg_p_per_l",
                self.algae_half_saturation_p_mg_p_per_l,
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
            (
                "death_biomass_to_detritus_fraction",
                self.death_biomass_to_detritus_fraction,
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
        if self.shrimp_juvenile_maturation_days <= 0.0 {
            return Err(format!(
                "shrimp_juvenile_maturation_days must be > 0.0, got {}",
                self.shrimp_juvenile_maturation_days
            ));
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
        if (self.death_biomass_to_detritus_fraction - 1.0).abs()
            > CLOSED_LOOP_DEATH_FRACTION_TOLERANCE
        {
            return Err(format!(
                "death_biomass_to_detritus_fraction must remain 1.0 until explicit export accounting exists, got {}",
                self.death_biomass_to_detritus_fraction
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
            (
                "microfauna_assimilation_efficiency",
                self.microfauna_assimilation_efficiency,
            ),
            (
                "microfauna_respiration_fraction_of_assimilated",
                self.microfauna_respiration_fraction_of_assimilated,
            ),
            (
                "microfauna_excretion_fraction_of_assimilated",
                self.microfauna_excretion_fraction_of_assimilated,
            ),
            (
                "microfauna_growth_fraction_of_assimilated",
                self.microfauna_growth_fraction_of_assimilated,
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
        if self.microfauna_assimilation_efficiency <= 0.0
            || self.microfauna_assimilation_efficiency >= 1.0
        {
            return Err(format!(
                "microfauna_assimilation_efficiency must be in (0, 1), got {}",
                self.microfauna_assimilation_efficiency
            ));
        }
        let microfauna_partition_sum = self.microfauna_respiration_fraction_of_assimilated
            + self.microfauna_excretion_fraction_of_assimilated
            + self.microfauna_growth_fraction_of_assimilated;
        if (microfauna_partition_sum - 1.0).abs() > SHRIMP_ROUTE_SUM_TOLERANCE {
            return Err(format!(
                "microfauna assimilated partition sum must equal 1.0, got {}",
                microfauna_partition_sum
            ));
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
    /// Optional scenario-authored biomedia area (cm²). When absent, startup
    /// materialization scales the default hardware media area with footprint.
    #[serde(default)]
    pub filter_media_area_cm2: Option<f64>,
    pub provenance: Option<Provenance>,
}

#[cfg(test)]
mod tests {
    use super::{
        parse_process_params_preset, PlantPreset, ProcessParamsPreset, ShrimpPreset,
        SourceWaterPreset, SubstratePreset,
    };
    use tank_core::types::provenance::{
        check_param_range, format_param, ConfidenceLevel, ParamMeta,
    };
    use tank_core::{
        legacy_total_param_to_mg_per_l, legacy_total_param_to_mg_per_m2,
        NITRIFICATION_ALK_MEQ_PER_MG_N,
    };

    fn default_process_preset() -> ProcessParamsPreset {
        parse_process_params_preset(include_str!("../data/process/default.toml"))
            .expect("default process preset should parse")
    }

    fn source_preset(contents: &str) -> SourceWaterPreset {
        toml::from_str(contents).expect("source preset should parse")
    }

    fn shrimp_preset(contents: &str) -> ShrimpPreset {
        toml::from_str(contents).expect("shrimp preset should parse")
    }

    fn plant_preset(contents: &str) -> PlantPreset {
        toml::from_str(contents).expect("plant preset should parse")
    }

    fn substrate_preset(contents: &str) -> SubstratePreset {
        toml::from_str(contents).expect("substrate preset should parse")
    }

    // ---- Provenance acceptance-criteria tests ----

    #[test]
    fn test_source_water_param_meta_roundtrip_and_range_warning(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
temperature_c = 24.0
ammonia_mg_n_per_l = 0.0
nitrite_mg_n_per_l = 0.0
nitrate_mg_n_per_l = 5.0
phosphate_mg_p_per_l = 0.1
dic_mg_c_per_l = 18.0
doc_mg_c_per_l = 0.0
don_mg_n_per_l = 0.0
alkalinity_meq_per_l = 1.249
calcium_mg_per_l = 25.0
magnesium_mg_per_l = 10.0
sodium_mg_per_l = 12.0
potassium_mg_per_l = 3.0
bicarbonate_mg_per_l = 76.2
chloride_mg_per_l = 18.0
sulfate_mg_per_l = 15.8

[param_meta.nitrate_mg_n_per_l]
unit = "mg N/L"
source = "Municipal water report"
confidence = "literature"
valid_range = [0.0, 2.0]
notes = "Winter baseline"
"#;
        let preset: SourceWaterPreset = toml::from_str(toml_str)?;
        preset.validate().expect("source preset should validate");

        let meta = preset
            .param_meta
            .get("nitrate_mg_n_per_l")
            .expect("metadata should exist");
        assert_eq!(meta.unit.as_deref(), Some("mg N/L"));
        assert_eq!(meta.confidence, Some(ConfidenceLevel::Literature));

        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].param_name, "nitrate_mg_n_per_l");
        assert_eq!(warnings[0].value, 5.0);

        let display = preset
            .format_param("nitrate_mg_n_per_l")
            .expect("parameter should format");
        assert!(display.contains("Municipal water report"));
        assert!(display.contains("range [0.0, 2.0]"));

        let serialized = toml::to_string(&preset)?;
        let roundtrip: SourceWaterPreset = toml::from_str(&serialized)?;
        assert_eq!(roundtrip.param_meta, preset.param_meta);
        Ok(())
    }

    #[test]
    fn test_source_water_unknown_param_meta_keys_are_rejected(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test"
temperature_c = 24.0
ammonia_mg_n_per_l = 0.0
nitrite_mg_n_per_l = 0.0
nitrate_mg_n_per_l = 5.0
phosphate_mg_p_per_l = 0.1
dic_mg_c_per_l = 18.0
doc_mg_c_per_l = 0.0
don_mg_n_per_l = 0.0
alkalinity_meq_per_l = 1.249
calcium_mg_per_l = 25.0
magnesium_mg_per_l = 10.0
sodium_mg_per_l = 12.0
potassium_mg_per_l = 3.0
bicarbonate_mg_per_l = 76.2
chloride_mg_per_l = 18.0
sulfate_mg_per_l = 15.8

[param_meta.nitrate_typo]
unit = "mg N/L"
"#;
        let preset: SourceWaterPreset = toml::from_str(toml_str)?;
        let err = preset
            .validate()
            .expect_err("unknown source-water param_meta key should fail");
        assert!(err.contains("unknown param_meta entries"));
        assert!(err.contains("nitrate_typo"));
        Ok(())
    }

    #[test]
    fn test_substrate_param_meta_roundtrip_and_range_warning(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test_substrate"
name = "Test Substrate"
depth_cm = 5.0
cation_exchange_capacity_index = 0.9
detritus_trapping_index = 0.6
colonizable_area_factor = 0.8
low_oxygen_tendency_index = 0.4
grazing_surface_index = 0.6
nutrient_charge_mg_n_total = 60.0
nutrient_charge_mg_p_total = 18.0

[param_meta.nutrient_charge_mg_n_total]
unit = "mg N total"
source = "Starter preset pack"
confidence = "expert"
valid_range = [0.0, 40.0]
notes = "Charged substrate can exceed inert baselines"
"#;
        let preset = substrate_preset(toml_str);
        preset.validate().expect("substrate preset should validate");

        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].param_name, "nutrient_charge_mg_n_total");
        assert_eq!(warnings[0].value, 60.0);

        let display = preset
            .format_param("nutrient_charge_mg_n_total")
            .expect("parameter should format");
        assert!(display.contains("Starter preset pack"));
        assert!(display.contains("mg N total"));

        let serialized = toml::to_string(&preset)?;
        let roundtrip: SubstratePreset = toml::from_str(&serialized)?;
        assert_eq!(roundtrip.param_meta, preset.param_meta);
        Ok(())
    }

    #[test]
    fn test_substrate_unknown_param_meta_keys_are_rejected(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test_substrate"
name = "Test Substrate"
depth_cm = 5.0
cation_exchange_capacity_index = 0.9
detritus_trapping_index = 0.6
colonizable_area_factor = 0.8
low_oxygen_tendency_index = 0.4
grazing_surface_index = 0.6
nutrient_charge_mg_n_total = 60.0
nutrient_charge_mg_p_total = 18.0

[param_meta.nutrient_charge_typo]
unit = "mg N total"
"#;
        let preset = substrate_preset(toml_str);
        let err = preset
            .validate()
            .expect_err("unknown substrate param_meta key should fail");
        assert!(err.contains("unknown param_meta entries"));
        assert!(err.contains("nutrient_charge_typo"));
        Ok(())
    }

    #[test]
    fn test_plant_param_meta_roundtrip_and_range_warning() -> Result<(), Box<dyn std::error::Error>>
    {
        let toml_str = r#"
id = "test_plant"
name = "Test Plant"
guild = "FastStem"
growth_rate_index = 0.9
water_column_uptake_bias = 0.9
substrate_uptake_bias = 0.2

[param_meta.water_column_uptake_bias]
unit = "relative weight"
source = "Ecology tuning note"
confidence = "heuristic"
valid_range = [0.0, 0.7]
notes = "Fast stems lean heavily on water-column nutrients"
"#;
        let preset = plant_preset(toml_str);
        preset.validate().expect("plant preset should validate");

        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].param_name, "water_column_uptake_bias");
        assert_eq!(warnings[0].value, 0.9);

        let display = preset
            .format_param("water_column_uptake_bias")
            .expect("parameter should format");
        assert!(display.contains("Ecology tuning note"));
        assert!(display.contains("relative weight"));

        let serialized = toml::to_string(&preset)?;
        let roundtrip: PlantPreset = toml::from_str(&serialized)?;
        assert_eq!(roundtrip.param_meta, preset.param_meta);
        Ok(())
    }

    #[test]
    fn test_plant_unknown_param_meta_keys_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test_plant"
name = "Test Plant"
guild = "FastStem"
growth_rate_index = 0.9
water_column_uptake_bias = 0.9
substrate_uptake_bias = 0.2

[param_meta.uptake_bias_typo]
unit = "relative weight"
"#;
        let preset = plant_preset(toml_str);
        let err = preset
            .validate()
            .expect_err("unknown plant param_meta key should fail");
        assert!(err.contains("unknown param_meta entries"));
        assert!(err.contains("uptake_bias_typo"));
        Ok(())
    }

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

[param_meta.aob_k_tan_mg_n_per_l]
unit = "mg N/L"
source = "EPA 2013 ammonia criteria"
confidence = "literature"
valid_range = [0.1, 5.0]
notes = "K_s for AOB in biofilter context; may differ for free-living AOB"
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;
        let meta = preset
            .param_meta
            .get("aob_k_tan_mg_n_per_l")
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
            roundtrip.param_meta.get("aob_k_tan_mg_n_per_l"),
            preset.param_meta.get("aob_k_tan_mg_n_per_l")
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

[param_meta.aob_k_tan_mg_n_per_l]
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

[param_meta.aob_k_tan_mg_n_per_l]
confidence = "medium"
"#;
        let result: Result<ProcessParamsPreset, _> = toml::from_str(bad_toml);
        assert!(result.is_err(), "unknown confidence 'medium' should fail");
        Ok(())
    }

    #[test]
    fn test_preset_level_provenance_confidence_is_freeform() {
        let preset = default_process_preset();
        assert_eq!(
            preset
                .provenance
                .as_ref()
                .and_then(|provenance| provenance.confidence.as_deref()),
            Some("medium")
        );
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
aob_k_tan_mg_n_per_l = 200.0

[param_meta.aob_k_tan_mg_n_per_l]
unit = "mg N/L"
valid_range = [0.1, 5.0]
"#;
        let preset: ProcessParamsPreset = toml::from_str(toml_str)?;

        // Validation should still succeed (range check is a warning path).
        preset.validate().expect("validate should pass");

        // But range check should produce a warning.
        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].param_name, "aob_k_tan_mg_n_per_l");
        assert_eq!(warnings[0].value, 200.0);
        assert_eq!(warnings[0].range, [0.1, 5.0]);

        // In-range value → no warning.
        let meta = ParamMeta {
            unit: None,
            source: None,
            confidence: None,
            valid_range: Some([0.1, 5.0]),
            notes: None,
        };
        assert!(check_param_range("aob_k_tan_mg_n_per_l", 0.5, &meta).is_none());
        Ok(())
    }

    #[test]
    fn test_valid_range_warns_for_invalid_range_or_non_finite_value() {
        let mut preset = default_process_preset();
        preset.aob_k_tan_mg_n_per_l = f64::INFINITY;
        preset.param_meta.insert(
            "aob_k_tan_mg_n_per_l".to_string(),
            ParamMeta {
                unit: Some("mg N/L".into()),
                source: None,
                confidence: None,
                valid_range: Some([5.0, 0.1]),
                notes: None,
            },
        );

        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].to_string().contains("invalid valid_range"));

        preset
            .param_meta
            .get_mut("aob_k_tan_mg_n_per_l")
            .expect("param_meta entry should exist")
            .valid_range = Some([0.1, 5.0]);
        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].value.is_infinite());
        assert!(warnings[0].to_string().contains("non-finite"));
    }

    /// AC 5: Existing TOML data files WITHOUT provenance metadata still load.
    #[test]
    fn test_backward_compatible_loading() -> Result<(), Box<dyn std::error::Error>> {
        // The shipped default.toml now carries param_meta (added in 6e5.7.2).
        // Verify it loads and validates successfully with the metadata present.
        let preset = default_process_preset();
        assert!(
            !preset.param_meta.is_empty(),
            "shipped default.toml should carry param_meta entries"
        );
        preset
            .validate()
            .expect("default file with param_meta should still validate");

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
        assert_eq!(preset.death_biomass_to_detritus_fraction, 1.0);
        assert_eq!(preset.microfauna_assimilation_efficiency, 0.50);
        assert_eq!(preset.microfauna_respiration_fraction_of_assimilated, 0.70);
        assert_eq!(preset.microfauna_excretion_fraction_of_assimilated, 0.10);
        assert_eq!(preset.microfauna_growth_fraction_of_assimilated, 0.20);
        Ok(())
    }

    #[test]
    fn default_process_preset_alkalinity_matches_core_constant() {
        let preset = default_process_preset();
        assert!(
            (preset.alkalinity_meq_per_mg_n_nitrified - NITRIFICATION_ALK_MEQ_PER_MG_N).abs()
                < 1e-12,
            "default process preset should stay synchronized with the core nitrification alkalinity constant"
        );
    }

    #[test]
    fn test_process_preset_parser_migrates_legacy_plant_half_saturation_keys(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let legacy_n_total = 13.0;
        let legacy_p_total = 2.8;
        let legacy_c_total = 34.0;
        let toml_str = format!(
            r#"
id = "legacy"
name = "Legacy"
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
plant_half_saturation_n_mg_total = {legacy_n_total}
plant_half_saturation_p_mg_total = {legacy_p_total}
plant_half_saturation_c_mg_total = {legacy_c_total}
"#
        );

        let preset = parse_process_params_preset(&toml_str)?;
        assert_eq!(
            preset.plant_half_saturation_n_mg_n_per_l,
            legacy_total_param_to_mg_per_l(legacy_n_total)
        );
        assert_eq!(
            preset.plant_half_saturation_p_mg_p_per_l,
            legacy_total_param_to_mg_per_l(legacy_p_total)
        );
        assert_eq!(
            preset.plant_half_saturation_c_mg_c_per_l,
            legacy_total_param_to_mg_per_l(legacy_c_total)
        );
        assert_eq!(
            preset.plant_half_saturation_n_substrate_mg_n_per_m2,
            legacy_total_param_to_mg_per_m2(legacy_n_total)
        );
        assert_eq!(
            preset.plant_half_saturation_p_substrate_mg_p_per_m2,
            legacy_total_param_to_mg_per_m2(legacy_p_total)
        );

        Ok(())
    }

    #[test]
    fn test_process_preset_parser_migrates_legacy_algae_half_saturation_keys(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let legacy_n_total = 5.0;
        let legacy_p_total = 0.8;
        let toml_str = format!(
            r#"
id = "legacy"
name = "Legacy"
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
algae_half_saturation_n_mg_total = {legacy_n_total}
algae_half_saturation_p_mg_total = {legacy_p_total}
"#
        );

        let preset = parse_process_params_preset(&toml_str)?;
        assert_eq!(
            preset.algae_half_saturation_n_mg_n_per_l,
            legacy_total_param_to_mg_per_l(legacy_n_total)
        );
        assert_eq!(
            preset.algae_half_saturation_p_mg_p_per_l,
            legacy_total_param_to_mg_per_l(legacy_p_total)
        );

        Ok(())
    }

    #[test]
    fn test_process_preset_parser_migrates_legacy_algae_half_saturation_param_meta_keys(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let legacy_n_total = 5.0;
        let legacy_p_total = 0.8;
        let toml_str = format!(
            r#"
id = "legacy"
name = "Legacy"
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
algae_half_saturation_n_mg_total = {legacy_n_total}
algae_half_saturation_p_mg_total = {legacy_p_total}

[param_meta.algae_half_saturation_n_mg_total]
unit = "mg N total"
"#
        );

        let preset = parse_process_params_preset(&toml_str)?;
        assert_eq!(
            preset.algae_half_saturation_n_mg_n_per_l,
            legacy_total_param_to_mg_per_l(legacy_n_total)
        );
        assert_eq!(
            preset.algae_half_saturation_p_mg_p_per_l,
            legacy_total_param_to_mg_per_l(legacy_p_total)
        );
        assert!(preset
            .param_meta
            .contains_key("algae_half_saturation_n_mg_n_per_l"));
        assert!(!preset
            .param_meta
            .contains_key("algae_half_saturation_n_mg_total"));

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
    fn test_process_preset_format_param_shows_concentration_value() {
        let mut preset = default_process_preset();
        preset.param_meta.insert(
            "aob_k_tan_mg_n_per_l".to_string(),
            ParamMeta {
                unit: Some("mg N/L".into()),
                source: Some("EPA 2013".into()),
                confidence: Some(ConfidenceLevel::Literature),
                valid_range: Some([0.1, 5.0]),
                notes: None,
            },
        );

        let display = preset
            .format_param("aob_k_tan_mg_n_per_l")
            .expect("parameter should format");
        assert!(display.starts_with("aob_k_tan_mg_n_per_l: 1 "));
        assert!(display.contains("mg N/L"));
        assert!(display.contains("range [0.1, 5.0]"));
    }

    #[test]
    fn test_process_preset_format_param_without_meta_shows_raw_value() {
        let preset = default_process_preset();

        // aob_k_tan_mg_n_per_l now carries param_meta in shipped default.toml,
        // so use a parameter that does NOT have param_meta for this test.
        let display = preset
            .format_param("feed_leach_rate_per_hour")
            .expect("parameter should format");
        assert_eq!(display, "feed_leach_rate_per_hour: 0.12");
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
        let err = preset
            .validate()
            .expect_err("unknown param_meta key should fail");
        assert!(err.contains("unknown param_meta entries"));
        assert!(err.contains("aob_k_tan_typo"));
        Ok(())
    }

    #[test]
    fn test_shrimp_param_meta_supports_defaulted_and_optional_fields(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = r#"
id = "test"
name = "Test shrimp"
species = "Neocaridina davidi"
optimal_temp_min_c = 22.0
optimal_temp_max_c = 26.0
gh_min_d = 5.0
gh_max_d = 10.0
egg_duration_days = 21
hatch_success_base = 0.7
juvenile_sensitivity = 1.5
high_temp_repro_penalty_start_c = 28.0
high_temp_repro_penalty_full_c = 33.0
base_molt_interval_days = 50.0

[param_meta.base_spawn_rate]
unit = "per day"
confidence = "heuristic"
notes = "Serde default should still be provenance-visible"

[param_meta.base_clutch_size]
unit = "eggs"
confidence = "expert"
valid_range = [1.0, 20.0]
notes = "Optional field should inherit the runtime default"

[param_meta.juvenile_to_subadult_days]
unit = "days"
confidence = "expert"
valid_range = [10.0, 20.0]
notes = "Optional field should inherit the runtime default"

[param_meta.base_molt_interval_days]
unit = "days"
confidence = "expert"
valid_range = [20.0, 40.0]
"#;
        let preset = shrimp_preset(toml_str);
        preset.validate().expect("shrimp preset should validate");

        assert_eq!(preset.base_spawn_rate, 0.15);
        let spawn_display = preset
            .format_param("base_spawn_rate")
            .expect("defaulted field should format");
        assert!(spawn_display.contains("base_spawn_rate: 0.15"));
        assert!(spawn_display.contains("heuristic"));

        let clutch_display = preset
            .format_param("base_clutch_size")
            .expect("default-backed optional integer should format");
        assert!(clutch_display.contains("base_clutch_size: 25"));
        assert!(clutch_display.contains("runtime default"));

        let maturation_display = preset
            .format_param("juvenile_to_subadult_days")
            .expect("default-backed optional float should format");
        assert!(maturation_display.contains("juvenile_to_subadult_days: 30"));

        let warnings = preset.check_ranges();
        assert_eq!(warnings.len(), 3);
        assert_eq!(warnings[0].param_name, "base_clutch_size");
        assert_eq!(warnings[0].value, 25.0);
        assert_eq!(warnings[1].param_name, "base_molt_interval_days");
        assert_eq!(warnings[1].value, 50.0);
        assert_eq!(warnings[2].param_name, "juvenile_to_subadult_days");
        assert_eq!(warnings[2].value, 30.0);
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
    fn process_preset_rejects_invalid_dic_rates() {
        let mut preset = default_process_preset();
        preset.respiration_dic_rate_mg_c_per_g_per_hour = -0.01;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("respiration_dic_rate_mg_c_per_g_per_hour"));

        let mut preset = default_process_preset();
        preset.photosynthesis_dic_rate_mg_c_per_g_per_hour = f64::NAN;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("photosynthesis_dic_rate_mg_c_per_g_per_hour"));
    }

    #[test]
    fn process_preset_rejects_non_positive_shrimp_juvenile_maturation_days() {
        let mut preset = default_process_preset();
        preset.shrimp_juvenile_maturation_days = 0.0;

        let err = preset.validate().expect_err("preset should be rejected");
        assert!(err.contains("shrimp_juvenile_maturation_days"));
        assert!(err.contains("> 0.0"));
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
        assert!(ro_ph < soft_ph, "ro-like should stay below soft acidic");
        assert!(
            (6.5..=7.0).contains(&soft_ph),
            "soft acidic pH {soft_ph:.3} out of band"
        );
        assert!(
            (7.0..=7.5).contains(&moderate_ph),
            "moderate pH {moderate_ph:.3} out of band"
        );
        assert!(
            (7.5..=8.0).contains(&hard_ph),
            "hard shrimp pH {hard_ph:.3} out of band"
        );
        assert!(
            (5.5..=6.5).contains(&ro_ph),
            "ro-like pH {ro_ph:.3} out of band"
        );
    }

    #[test]
    fn shipped_source_water_priority_metadata_matches_summary() {
        let moderate = source_preset(include_str!("../data/source_water/moderate.toml"));
        let hard = source_preset(include_str!("../data/source_water/hard_shrimp.toml"));

        for preset in [&moderate, &hard] {
            preset.validate().expect("shipped preset should validate");
        }

        for name in [
            "dic_mg_c_per_l",
            "alkalinity_meq_per_l",
            "bicarbonate_mg_per_l",
            "chloride_mg_per_l",
        ] {
            assert_eq!(
                moderate
                    .param_meta
                    .get(name)
                    .and_then(|meta| meta.confidence),
                Some(ConfidenceLevel::Heuristic),
                "moderate.{name} should stay heuristic"
            );
            assert_eq!(
                hard.param_meta.get(name).and_then(|meta| meta.confidence),
                Some(ConfidenceLevel::Heuristic),
                "hard_shrimp.{name} should stay heuristic"
            );
        }

        for name in ["calcium_mg_per_l", "magnesium_mg_per_l"] {
            assert_eq!(
                moderate
                    .param_meta
                    .get(name)
                    .and_then(|meta| meta.confidence),
                Some(ConfidenceLevel::Expert),
                "moderate.{name} should be expert-curated"
            );
            assert_eq!(
                hard.param_meta.get(name).and_then(|meta| meta.confidence),
                Some(ConfidenceLevel::Expert),
                "hard_shrimp.{name} should be expert-curated"
            );
        }
    }

    #[test]
    fn shipped_substrate_presets_expose_colonizable_area_metadata() {
        let active = substrate_preset(include_str!("../data/substrate/active_planted.toml"));
        let porous = substrate_preset(include_str!("../data/substrate/coarse_porous.toml"));

        for preset in [&active, &porous] {
            preset.validate().expect("shipped preset should validate");
            let meta = preset
                .param_meta
                .get("colonizable_area_factor")
                .expect("colonizable area metadata should exist");
            assert_eq!(meta.confidence, Some(ConfidenceLevel::Heuristic));
            assert!(meta.source.is_some(), "source should be documented");
            assert!(meta.notes.is_some(), "notes should be documented");
        }
    }

    #[test]
    fn shipped_process_priority_metadata_covers_denitrification_fields() {
        let preset = default_process_preset();

        preset
            .validate()
            .expect("shipped process preset should validate");

        for name in [
            "denitrification_vmax_mg_n_per_l_per_hour",
            "denitrification_k_no3_mg_n_per_l",
            "denitrification_k_doc_mg_c_per_l",
        ] {
            let meta = preset
                .param_meta
                .get(name)
                .unwrap_or_else(|| panic!("{name} metadata should exist"));
            assert_eq!(meta.confidence, Some(ConfidenceLevel::Literature));
            assert!(meta.source.is_some(), "{name} should document a source");
        }

        assert_eq!(
            preset
                .param_meta
                .get("denitrification_pore_water_mixing_factor")
                .and_then(|meta| meta.confidence),
            Some(ConfidenceLevel::Heuristic)
        );
        assert_eq!(
            preset
                .param_meta
                .get("denitrification_activity_maturation_days")
                .and_then(|meta| meta.confidence),
            Some(ConfidenceLevel::Expert)
        );
    }

    #[test]
    fn shipped_shrimp_priority_metadata_covers_repro_suppression_curve_endpoints() {
        let preset = shrimp_preset(include_str!("../data/shrimp/neocaridina_davidi.toml"));

        preset
            .validate()
            .expect("shipped shrimp preset should validate");

        for name in [
            "tan_repro_full_suppression_mg_n_per_l",
            "no2_repro_full_suppression_mg_n_per_l",
        ] {
            let meta = preset
                .param_meta
                .get(name)
                .unwrap_or_else(|| panic!("{name} metadata should exist"));
            assert_eq!(meta.confidence, Some(ConfidenceLevel::Heuristic));
            assert!(meta.notes.is_some(), "{name} should explain the derivation");
        }
    }
}
