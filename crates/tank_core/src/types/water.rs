use serde::{Deserialize, Serialize};

use super::TankGeometry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaterState {
    pub temperature_c: f64,
    pub ammonia_total_mg_n_total: f64,
    pub nitrite_mg_n_total: f64,
    pub nitrate_mg_n_total: f64,
    pub phosphate_mg_p_total: f64,
    pub dissolved_oxygen_mg_total: f64,
    pub dissolved_inorganic_carbon_mg_c_total: f64,
    pub dissolved_organic_carbon_mg_c_total: f64,
    pub dissolved_organic_nitrogen_mg_n_total: f64,
    pub alkalinity_meq_total: f64,
    pub calcium_mg_total: f64,
    pub magnesium_mg_total: f64,
    pub sodium_mg_total: f64,
    pub potassium_mg_total: f64,
    pub bicarbonate_mg_total: f64,
    pub chloride_mg_total: f64,
    pub sulfate_mg_total: f64,
}

impl WaterState {
    pub fn default_for_geometry(geometry: &TankGeometry) -> Self {
        let volume_l = geometry.water_volume_l();
        Self {
            temperature_c: 24.0,
            ammonia_total_mg_n_total: 0.0,
            nitrite_mg_n_total: 0.0,
            nitrate_mg_n_total: 0.0,
            phosphate_mg_p_total: 0.0,
            dissolved_oxygen_mg_total: 8.0 * volume_l,
            dissolved_inorganic_carbon_mg_c_total: 20.0 * volume_l,
            dissolved_organic_carbon_mg_c_total: 0.0,
            dissolved_organic_nitrogen_mg_n_total: 0.0,
            alkalinity_meq_total: 1.5 * volume_l,
            calcium_mg_total: 20.0 * volume_l,
            magnesium_mg_total: 5.0 * volume_l,
            sodium_mg_total: 10.0 * volume_l,
            potassium_mg_total: 3.0 * volume_l,
            bicarbonate_mg_total: 70.0 * volume_l,
            chloride_mg_total: 12.0 * volume_l,
            sulfate_mg_total: 8.0 * volume_l,
        }
    }

    pub fn total_tracked_ions_mg(&self) -> f64 {
        self.calcium_mg_total
            + self.magnesium_mg_total
            + self.sodium_mg_total
            + self.potassium_mg_total
            + self.bicarbonate_mg_total
            + self.chloride_mg_total
            + self.sulfate_mg_total
    }
}

impl Default for WaterState {
    fn default() -> Self {
        Self::default_for_geometry(&TankGeometry::default())
    }
}
