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

    /// Default porosity (void fraction) for the substrate kind.
    ///
    /// Porosity controls pore water volume and O₂ diffusion tortuosity.
    /// Values represent typical void fractions for each grain class:
    /// - Fine sand: tightly packed, low porosity
    /// - Gravel: moderate interstitial space
    /// - Active planted (e.g., aqua-soil): high porosity, granular structure
    /// - Coarse porous (lava rock, pumice): very high porosity
    pub fn default_porosity(&self) -> f64 {
        match self {
            Self::InertSand => 0.35,
            Self::InertGravel => 0.40,
            Self::ActivePlanted => 0.50,
            Self::CoarsePorous => 0.55,
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
    /// Void fraction of the substrate bed (0–1). Controls pore water volume
    /// and O₂ diffusion tortuosity. Falls back to `kind.default_porosity()`
    /// when deserialized as 0 from legacy saves.
    #[serde(default)]
    pub porosity: f64,
    /// Dynamic O₂ penetration depth (cm) computed each tick from the
    /// Bouldin (1968) steady-state diffusion model. Substrate above this
    /// depth is oxic (maps to `SubstrateSurface` habitat); substrate below
    /// is suboxic (maps to `SubstrateDeep` habitat).
    #[serde(default)]
    pub o2_penetration_depth_cm: f64,
}

impl SubstrateLayerState {
    pub fn resolved_colonizable_area_factor(&self) -> f64 {
        if self.colonizable_area_factor.is_finite() && self.colonizable_area_factor > 0.0 {
            self.colonizable_area_factor
        } else {
            self.kind.default_colonizable_area_factor()
        }
    }

    /// Resolved porosity, falling back to the substrate kind default when the
    /// serialized value is zero or invalid (legacy saves).
    pub fn resolved_porosity(&self) -> f64 {
        if self.porosity.is_finite() && self.porosity > 0.0 {
            self.porosity.min(1.0)
        } else {
            self.kind.default_porosity()
        }
    }

    /// Derived internal colonizable area for this layer from the current tank
    /// footprint, serialized colonizable-area factor, and actual layer depth.
    pub fn derived_colonizable_area_cm2(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0)
            * self.depth_cm.max(0.0)
            * self.resolved_colonizable_area_factor()
    }

    /// O₂ penetration depth clamped to this layer's depth. Returns `depth_cm`
    /// when the penetration depth has not yet been computed (zero from
    /// legacy deserialization).
    pub fn effective_o2_penetration_depth_cm(&self) -> f64 {
        if self.o2_penetration_depth_cm <= 0.0 {
            // Not yet computed or legacy save — assume fully oxic.
            self.depth_cm.max(0.0)
        } else {
            self.o2_penetration_depth_cm.min(self.depth_cm.max(0.0))
        }
    }

    /// Depth of the oxic zone within this layer (cm).
    pub fn oxic_depth_cm(&self) -> f64 {
        self.effective_o2_penetration_depth_cm()
    }

    /// Depth of the suboxic zone within this layer (cm).
    pub fn suboxic_depth_cm(&self) -> f64 {
        (self.depth_cm.max(0.0) - self.oxic_depth_cm()).max(0.0)
    }

    /// Pore water volume in the oxic zone (cm³).
    pub fn oxic_pore_volume_cm3(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.oxic_depth_cm() * self.resolved_porosity()
    }

    /// Pore water volume in the suboxic zone (cm³).
    pub fn suboxic_pore_volume_cm3(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.suboxic_depth_cm() * self.resolved_porosity()
    }

    /// Total pore water volume for this layer (cm³).
    pub fn total_pore_volume_cm3(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.depth_cm.max(0.0) * self.resolved_porosity()
    }

    /// Interstitial colonizable area within the oxic zone (cm²).
    pub fn oxic_colonizable_area_cm2(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.oxic_depth_cm() * self.resolved_colonizable_area_factor()
    }

    /// Interstitial colonizable area within the suboxic zone (cm²).
    pub fn suboxic_colonizable_area_cm2(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0)
            * self.suboxic_depth_cm()
            * self.resolved_colonizable_area_factor()
    }
}

impl Default for SubstrateLayerState {
    fn default() -> Self {
        let kind = SubstrateKind::InertSand;
        let depth_cm = 3.0;
        let mut layer = Self {
            kind,
            depth_cm,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.3,
            colonizable_area_factor: 0.0,
            colonizable_area_cm2: 0.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.4,
            porosity: kind.default_porosity(),
            o2_penetration_depth_cm: depth_cm,
        };
        layer.colonizable_area_cm2 =
            layer.derived_colonizable_area_cm2(TankGeometry::default().footprint_area_cm2());
        layer
    }
}
