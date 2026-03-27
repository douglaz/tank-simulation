use serde::{Deserialize, Serialize};

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
        ];
        for (name, value) in shrimp_fields {
            if !value.is_finite() {
                return Err(format!("field `{name}` must be finite, got {value}"));
            }
            if *value < 0.0 {
                return Err(format!("field `{name}` must be non-negative, got {value}"));
            }
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
