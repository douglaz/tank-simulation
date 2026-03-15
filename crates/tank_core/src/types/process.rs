use serde::{Deserialize, Serialize};

/// Runtime process parameters for heat-transfer coefficients and later chemistry systems.
/// Stored in `TankState` for deterministic continuation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProcessParams {
    pub mineralization_rate_per_day: f64,
    pub nitrification_vmax: f64,
    pub reaeration_kla_base: f64,
    pub aeration_kla_boost: f64,
    pub background_bod_mg_o2_per_g_biomass_per_hour: f64,
    pub plant_photosynthesis_o2_mg_per_g_per_hour: f64,
    pub respiration_dic_rate_mg_c_per_g_per_hour: f64,
    pub photosynthesis_dic_rate_mg_c_per_g_per_hour: f64,
    /// Surface (top) heat-transfer coefficient in W/(m²·K).
    pub k_surface_w_per_m2_k: f64,
    /// Wall heat-transfer coefficient in W/(m²·K).
    pub k_wall_w_per_m2_k: f64,
}

impl Default for ProcessParams {
    fn default() -> Self {
        Self {
            mineralization_rate_per_day: 0.15,
            nitrification_vmax: 0.08,
            reaeration_kla_base: 0.35,
            aeration_kla_boost: 0.9,
            background_bod_mg_o2_per_g_biomass_per_hour: 0.05,
            plant_photosynthesis_o2_mg_per_g_per_hour: 0.2,
            respiration_dic_rate_mg_c_per_g_per_hour: 0.0,
            photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.0,
            k_surface_w_per_m2_k: 10.0,
            k_wall_w_per_m2_k: 5.0,
        }
    }
}
