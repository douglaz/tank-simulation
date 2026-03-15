use serde::{Deserialize, Serialize};

/// Runtime source-water profile with explicit per-liter chemistry.
/// Stored in `TankState.source_water_catalog` for deterministic continuation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceWaterProfile {
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
}

impl SourceWaterProfile {
    /// Creates a zero-nutrient profile (equivalent to pure RO water).
    pub fn zero() -> Self {
        Self {
            temperature_c: 23.0,
            ammonia_mg_n_per_l: 0.0,
            nitrite_mg_n_per_l: 0.0,
            nitrate_mg_n_per_l: 0.0,
            phosphate_mg_p_per_l: 0.0,
            dic_mg_c_per_l: 0.0,
            doc_mg_c_per_l: 0.0,
            don_mg_n_per_l: 0.0,
            alkalinity_meq_per_l: 0.0,
            calcium_mg_per_l: 0.0,
            magnesium_mg_per_l: 0.0,
            sodium_mg_per_l: 0.0,
            potassium_mg_per_l: 0.0,
            bicarbonate_mg_per_l: 0.0,
            chloride_mg_per_l: 0.0,
            sulfate_mg_per_l: 0.0,
        }
    }
}
