use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubstrateKind {
    InertSand,
    InertGravel,
    ActivePlanted,
    CoarsePorous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubstrateLayerState {
    pub kind: SubstrateKind,
    pub depth_cm: f64,
    pub nutrient_store_mg_n_total: f64,
    pub nutrient_store_mg_p_total: f64,
    pub cation_exchange_capacity_index: f64,
    pub detritus_trapping_index: f64,
    pub colonizable_area_cm2: f64,
    pub low_oxygen_tendency_index: f64,
    pub grazing_surface_index: f64,
}

impl Default for SubstrateLayerState {
    fn default() -> Self {
        Self {
            kind: SubstrateKind::InertSand,
            depth_cm: 3.0,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.3,
            colonizable_area_cm2: 100.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.4,
        }
    }
}
