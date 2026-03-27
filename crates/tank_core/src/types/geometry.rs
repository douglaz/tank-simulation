use serde::{Deserialize, Serialize};

const DEFAULT_LID_EXCHANGE_FACTOR: f64 = 0.25;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TankGeometry {
    pub length_cm: f64,
    pub width_cm: f64,
    pub height_cm: f64,
    pub fill_height_cm: f64,
    pub glass_thickness_mm: f64,
    pub open_top: bool,
    #[serde(default = "default_lid_exchange_factor")]
    pub lid_exchange_factor: f64,
}

impl TankGeometry {
    /// Gross filled-prism volume before substrate displacement.
    pub fn water_volume_l(&self) -> f64 {
        self.length_cm * self.width_cm * self.fill_height_cm / 1000.0
    }

    pub fn substrate_displacement_l(&self, substrate_depth_cm: f64) -> f64 {
        if !substrate_depth_cm.is_finite() {
            return 0.0;
        }

        let capped_depth_cm = substrate_depth_cm.clamp(0.0, self.fill_height_cm.max(0.0));
        self.footprint_area_cm2() * capped_depth_cm / 1000.0
    }

    pub fn water_volume_l_with_substrate_depth(&self, substrate_depth_cm: f64) -> f64 {
        (self.water_volume_l() - self.substrate_displacement_l(substrate_depth_cm)).max(0.0)
    }

    pub fn surface_area_cm2(&self) -> f64 {
        self.length_cm * self.width_cm
    }

    pub fn footprint_area_cm2(&self) -> f64 {
        self.length_cm * self.width_cm
    }

    pub fn footprint_area_m2(&self) -> f64 {
        (self.footprint_area_cm2() / 10_000.0).max(0.0)
    }

    pub fn wall_area_cm2(&self) -> f64 {
        2.0 * self.fill_height_cm * (self.length_cm + self.width_cm)
    }

    pub fn mean_depth_cm(&self) -> f64 {
        self.fill_height_cm
    }

    pub fn surface_area_to_volume_ratio(&self) -> f64 {
        let volume_l = self.water_volume_l();
        if volume_l <= f64::EPSILON {
            0.0
        } else {
            self.surface_area_cm2() / volume_l
        }
    }

    pub fn wall_area_to_volume_ratio(&self) -> f64 {
        let volume_l = self.water_volume_l();
        if volume_l <= f64::EPSILON {
            0.0
        } else {
            self.wall_area_cm2() / volume_l
        }
    }

    pub fn top_exchange_factor(&self) -> f64 {
        if self.open_top {
            1.0
        } else {
            self.lid_exchange_factor.clamp(0.0, 1.0)
        }
    }
}

impl Default for TankGeometry {
    fn default() -> Self {
        Self {
            length_cm: 40.0,
            width_cm: 25.0,
            height_cm: 25.0,
            fill_height_cm: 22.0,
            glass_thickness_mm: 5.0,
            open_top: true,
            lid_exchange_factor: DEFAULT_LID_EXCHANGE_FACTOR,
        }
    }
}

fn default_lid_exchange_factor() -> f64 {
    DEFAULT_LID_EXCHANGE_FACTOR
}
