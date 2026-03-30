use serde::{Deserialize, Serialize};

use super::TankGeometry;

const UNCOMPUTED_O2_PENETRATION_DEPTH_CM: f64 = -1.0;

fn default_uncomputed_o2_penetration_depth_cm() -> f64 {
    UNCOMPUTED_O2_PENETRATION_DEPTH_CM
}

/// Canonical redox zones within the substrate bed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubstrateZone {
    Oxic,
    Suboxic,
}

/// Aggregate geometry for one substrate redox zone across the full bed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SubstrateZoneGeometry {
    pub depth_cm: f64,
    pub volume_cm3: f64,
    pub pore_volume_cm3: f64,
    pub colonizable_area_cm2: f64,
}

/// First-pass nutrient availability metrics for one substrate redox zone.
///
/// Nutrients are partitioned uniformly by depth within each substrate layer.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SubstrateZoneNutrientAvailability {
    pub nitrogen_mg_total: f64,
    pub phosphorus_mg_total: f64,
    pub nitrogen_mg_per_m2: f64,
    pub phosphorus_mg_per_m2: f64,
}

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
    /// Dynamic O₂ penetration boundary depth (cm below the substrate
    /// surface) computed each tick from the Bouldin (1968) steady-state
    /// diffusion model. The same shared stack boundary is serialized onto
    /// each layer so save/load can round-trip the zone state without a
    /// separate top-level substrate-zone payload.
    #[serde(default = "default_uncomputed_o2_penetration_depth_cm")]
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

    pub fn has_computed_o2_penetration_depth(&self) -> bool {
        self.o2_penetration_depth_cm.is_finite() && self.o2_penetration_depth_cm >= 0.0
    }

    /// O₂ penetration depth resolved against the total stacked substrate
    /// depth. Legacy/uncomputed payloads deserialize as a negative sentinel
    /// and resolve to full-depth penetration until the next zone update.
    pub fn resolved_o2_penetration_depth_cm(&self, total_substrate_depth_cm: f64) -> f64 {
        let total_depth_cm = total_substrate_depth_cm.max(0.0);
        if self.has_computed_o2_penetration_depth() {
            self.o2_penetration_depth_cm.clamp(0.0, total_depth_cm)
        } else {
            total_depth_cm
        }
    }

    /// Depth of the requested redox zone within this layer (cm), given the
    /// cumulative layer top depth and the shared substrate O₂ boundary.
    pub fn zone_depth_cm(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        zone: SubstrateZone,
    ) -> f64 {
        let layer_depth_cm = self.depth_cm.max(0.0);
        let layer_top_depth_cm = layer_top_depth_cm.max(0.0);
        let layer_bottom_depth_cm = layer_top_depth_cm + layer_depth_cm;
        let oxic_depth_cm = (o2_penetration_depth_cm.clamp(0.0, layer_bottom_depth_cm)
            - layer_top_depth_cm)
            .clamp(0.0, layer_depth_cm);

        match zone {
            SubstrateZone::Oxic => oxic_depth_cm,
            SubstrateZone::Suboxic => (layer_depth_cm - oxic_depth_cm).max(0.0),
        }
    }

    /// Depth of the oxic zone within this layer (cm).
    pub fn oxic_depth_cm(&self, layer_top_depth_cm: f64, o2_penetration_depth_cm: f64) -> f64 {
        self.zone_depth_cm(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            SubstrateZone::Oxic,
        )
    }

    /// Depth of the suboxic zone within this layer (cm).
    pub fn suboxic_depth_cm(&self, layer_top_depth_cm: f64, o2_penetration_depth_cm: f64) -> f64 {
        self.zone_depth_cm(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            SubstrateZone::Suboxic,
        )
    }

    /// Bulk substrate volume in the requested redox zone (cm³).
    pub fn zone_volume_cm3(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
        zone: SubstrateZone,
    ) -> f64 {
        footprint_area_cm2.max(0.0)
            * self.zone_depth_cm(layer_top_depth_cm, o2_penetration_depth_cm, zone)
    }

    /// Pore water volume in the requested redox zone (cm³).
    pub fn zone_pore_volume_cm3(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
        zone: SubstrateZone,
    ) -> f64 {
        self.zone_volume_cm3(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            footprint_area_cm2,
            zone,
        ) * self.resolved_porosity()
    }

    /// Pore water volume in the oxic zone (cm³).
    pub fn oxic_pore_volume_cm3(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
    ) -> f64 {
        self.zone_pore_volume_cm3(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            footprint_area_cm2,
            SubstrateZone::Oxic,
        )
    }

    /// Pore water volume in the suboxic zone (cm³).
    pub fn suboxic_pore_volume_cm3(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
    ) -> f64 {
        self.zone_pore_volume_cm3(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            footprint_area_cm2,
            SubstrateZone::Suboxic,
        )
    }

    /// Total pore water volume for this layer (cm³).
    pub fn total_pore_volume_cm3(&self, footprint_area_cm2: f64) -> f64 {
        footprint_area_cm2.max(0.0) * self.depth_cm.max(0.0) * self.resolved_porosity()
    }

    /// Interstitial colonizable area within the requested redox zone (cm²).
    pub fn zone_colonizable_area_cm2(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
        zone: SubstrateZone,
    ) -> f64 {
        footprint_area_cm2.max(0.0)
            * self.zone_depth_cm(layer_top_depth_cm, o2_penetration_depth_cm, zone)
            * self.resolved_colonizable_area_factor()
    }

    /// Interstitial colonizable area within the oxic zone (cm²).
    pub fn oxic_colonizable_area_cm2(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
    ) -> f64 {
        self.zone_colonizable_area_cm2(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            footprint_area_cm2,
            SubstrateZone::Oxic,
        )
    }

    /// Interstitial colonizable area within the suboxic zone (cm²).
    pub fn suboxic_colonizable_area_cm2(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_cm2: f64,
    ) -> f64 {
        self.zone_colonizable_area_cm2(
            layer_top_depth_cm,
            o2_penetration_depth_cm,
            footprint_area_cm2,
            SubstrateZone::Suboxic,
        )
    }

    fn zone_fraction(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        zone: SubstrateZone,
    ) -> f64 {
        let layer_depth_cm = self.depth_cm.max(0.0);
        if layer_depth_cm <= f64::EPSILON {
            0.0
        } else {
            self.zone_depth_cm(layer_top_depth_cm, o2_penetration_depth_cm, zone) / layer_depth_cm
        }
    }

    pub fn zone_nutrient_store_mg_n_total(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        zone: SubstrateZone,
    ) -> f64 {
        self.nutrient_store_mg_n_total
            * self.zone_fraction(layer_top_depth_cm, o2_penetration_depth_cm, zone)
    }

    pub fn zone_nutrient_store_mg_p_total(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        zone: SubstrateZone,
    ) -> f64 {
        self.nutrient_store_mg_p_total
            * self.zone_fraction(layer_top_depth_cm, o2_penetration_depth_cm, zone)
    }

    pub fn zone_nutrient_availability(
        &self,
        layer_top_depth_cm: f64,
        o2_penetration_depth_cm: f64,
        footprint_area_m2: f64,
        zone: SubstrateZone,
    ) -> SubstrateZoneNutrientAvailability {
        let nitrogen_mg_total =
            self.zone_nutrient_store_mg_n_total(layer_top_depth_cm, o2_penetration_depth_cm, zone);
        let phosphorus_mg_total =
            self.zone_nutrient_store_mg_p_total(layer_top_depth_cm, o2_penetration_depth_cm, zone);
        let area_m2 = footprint_area_m2.max(0.0);

        SubstrateZoneNutrientAvailability {
            nitrogen_mg_total,
            phosphorus_mg_total,
            nitrogen_mg_per_m2: if area_m2 <= f64::EPSILON {
                0.0
            } else {
                nitrogen_mg_total / area_m2
            },
            phosphorus_mg_per_m2: if area_m2 <= f64::EPSILON {
                0.0
            } else {
                phosphorus_mg_total / area_m2
            },
        }
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
