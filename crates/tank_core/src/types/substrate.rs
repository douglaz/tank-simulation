use serde::{Deserialize, Serialize};

use super::TankGeometry;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubstrateKind {
    InertSand,
    InertGravel,
    ActivePlanted,
    CoarsePorous,
}

impl SubstrateKind {
    /// Default interstitial colonizable-area multiplier per cm of substrate
    /// depth for the coarse substrate classes the engine currently supports.
    pub fn default_colonizable_area_factor(&self) -> f64 {
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
    /// Serialized source-of-truth multiplier for interstitial colonizable
    /// area. Scenario presets and custom state should persist this so habitat
    /// refreshes do not fall back to hard-coded kind defaults.
    pub colonizable_area_factor: f64,
    pub colonizable_area_cm2: f64,
    pub low_oxygen_tendency_index: f64,
    pub grazing_surface_index: f64,
}

impl SubstrateLayerState {
    pub fn resolved_colonizable_area_factor(&self) -> f64 {
        if self.colonizable_area_factor.is_finite() && self.colonizable_area_factor > 0.0 {
            self.colonizable_area_factor
        } else {
            self.kind.default_colonizable_area_factor()
        }
    }

    /// Derived internal colonizable area for this layer from the current tank
    /// footprint, serialized colonizable-area factor, and actual layer depth.
    pub fn derived_colonizable_area_cm2(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0)
            * self.depth_cm.max(0.0)
            * self.resolved_colonizable_area_factor()
    }
}

impl Default for SubstrateLayerState {
    fn default() -> Self {
        let mut layer = Self {
            kind: SubstrateKind::InertSand,
            depth_cm: 3.0,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.3,
            colonizable_area_factor: 0.0,
            colonizable_area_cm2: 0.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.4,
        };
        layer.colonizable_area_cm2 =
            layer.derived_colonizable_area_cm2(TankGeometry::default().footprint_area_cm2());
        layer
    }
}
