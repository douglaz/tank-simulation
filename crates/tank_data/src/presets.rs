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
    pub provenance: Option<Provenance>,
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
    pub provenance: Option<Provenance>,
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
