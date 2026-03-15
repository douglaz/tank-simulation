use serde::{Deserialize, Serialize};

/// Runtime process parameters for heat-transfer coefficients and later chemistry systems.
/// Stored in `TankState` for deterministic continuation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessParams {
    pub mineralization_rate_per_day: f64,
    pub nitrification_vmax: f64,
    pub reaeration_kla: f64,
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
            reaeration_kla: 0.4,
            k_surface_w_per_m2_k: 10.0,
            k_wall_w_per_m2_k: 5.0,
        }
    }
}
