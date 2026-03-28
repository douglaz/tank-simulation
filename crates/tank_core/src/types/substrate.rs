use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubstrateKind {
    InertSand,
    InertGravel,
    ActivePlanted,
    CoarsePorous,
}

impl SubstrateKind {
    /// Effective interstitial colonizable-area multiplier per cm of substrate
    /// depth for the coarse substrate classes the engine currently supports.
    /// Keep these values aligned with the substrate preset pack until the
    /// factor is promoted into first-class serialized layer metadata.
    pub fn colonizable_area_factor(&self) -> f64 {
        match self {
            Self::InertSand => 0.5,
            Self::InertGravel => 0.6,
            Self::ActivePlanted => 0.8,
            Self::CoarsePorous => 0.9,
        }
    }
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

impl SubstrateLayerState {
    /// Derived internal colonizable area for this layer from the current tank
    /// footprint, substrate kind, and actual layer depth.
    pub fn derived_colonizable_area_cm2(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.depth_cm.max(0.0) * self.kind.colonizable_area_factor()
    }
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
