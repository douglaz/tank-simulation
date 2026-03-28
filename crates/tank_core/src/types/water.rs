use serde::{Deserialize, Serialize};

use super::{SourceWaterProfile, SubstrateLayerState, TankGeometry};

const MG_CACO3_PER_MEQ: f64 = 50.0;
const MG_CACO3_PER_DEGREE: f64 = 17.848;

fn default_ph() -> f64 {
    7.0
}

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
    #[serde(default = "default_ph")]
    pub ph: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ConcentrationView<'a> {
    water: &'a WaterState,
    volume_l: f64,
}

impl WaterState {
    pub fn default_for_volume_l(volume_l: f64) -> Self {
        let volume_l = volume_l.max(0.0);
        let mut state = Self {
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
            ph: default_ph(),
        };
        crate::systems::chemistry::resolve_carbonate_state(&mut state, volume_l);
        state
    }

    pub fn default_for_geometry(geometry: &TankGeometry, substrate_depth_cm: f64) -> Self {
        Self::default_for_volume_l(geometry.water_volume_l_with_substrate_depth(substrate_depth_cm))
    }

    /// Creates initial water state from a source-water profile and tank geometry.
    /// All dissolved totals are the profile's per-liter values multiplied by volume.
    pub fn from_source_profile(
        profile: &SourceWaterProfile,
        geometry: &TankGeometry,
        substrate_depth_cm: f64,
    ) -> Self {
        Self::from_source_profile_for_volume_l(
            profile,
            geometry.water_volume_l_with_substrate_depth(substrate_depth_cm),
        )
    }

    /// Creates initial water state from a source-water profile and explicit volume.
    /// All dissolved totals are the profile's per-liter values multiplied by volume.
    pub fn from_source_profile_for_volume_l(profile: &SourceWaterProfile, volume_l: f64) -> Self {
        let volume_l = volume_l.max(0.0);
        let do_sat = crate::systems::temperature::do_sat_mg_l(profile.temperature_c);
        let mut state = Self {
            temperature_c: profile.temperature_c,
            ammonia_total_mg_n_total: profile.ammonia_mg_n_per_l * volume_l,
            nitrite_mg_n_total: profile.nitrite_mg_n_per_l * volume_l,
            nitrate_mg_n_total: profile.nitrate_mg_n_per_l * volume_l,
            phosphate_mg_p_total: profile.phosphate_mg_p_per_l * volume_l,
            dissolved_oxygen_mg_total: do_sat * volume_l,
            dissolved_inorganic_carbon_mg_c_total: profile.dic_mg_c_per_l * volume_l,
            dissolved_organic_carbon_mg_c_total: profile.doc_mg_c_per_l * volume_l,
            dissolved_organic_nitrogen_mg_n_total: profile.don_mg_n_per_l * volume_l,
            alkalinity_meq_total: profile.alkalinity_meq_per_l * volume_l,
            calcium_mg_total: profile.calcium_mg_per_l * volume_l,
            magnesium_mg_total: profile.magnesium_mg_per_l * volume_l,
            sodium_mg_total: profile.sodium_mg_per_l * volume_l,
            potassium_mg_total: profile.potassium_mg_per_l * volume_l,
            bicarbonate_mg_total: profile.bicarbonate_mg_per_l * volume_l,
            chloride_mg_total: profile.chloride_mg_per_l * volume_l,
            sulfate_mg_total: profile.sulfate_mg_per_l * volume_l,
            ph: default_ph(),
        };
        crate::systems::chemistry::resolve_carbonate_state(&mut state, volume_l);
        state
    }

    pub fn tan_mg_n_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.ammonia_total_mg_n_total, volume_l)
    }

    pub fn nitrite_mg_n_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.nitrite_mg_n_total, volume_l)
    }

    pub fn nitrate_mg_n_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.nitrate_mg_n_total, volume_l)
    }

    pub fn don_mg_n_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.dissolved_organic_nitrogen_mg_n_total, volume_l)
    }

    pub fn doc_mg_c_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.dissolved_organic_carbon_mg_c_total, volume_l)
    }

    pub fn dic_mg_c_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.dissolved_inorganic_carbon_mg_c_total, volume_l)
    }

    pub fn do_mg_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.dissolved_oxygen_mg_total, volume_l)
    }

    pub fn phosphate_mg_p_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.phosphate_mg_p_total, volume_l)
    }

    pub fn alkalinity_meq_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.alkalinity_meq_total, volume_l)
    }

    pub fn calcium_mg_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.calcium_mg_total, volume_l)
    }

    pub fn magnesium_mg_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.magnesium_mg_total, volume_l)
    }

    pub fn gh_d(&self, volume_l: f64) -> f64 {
        let calcium_mg_per_l = self.calcium_mg_per_l(volume_l);
        let magnesium_mg_per_l = self.magnesium_mg_per_l(volume_l);

        (((2.497 * calcium_mg_per_l) + (4.118 * magnesium_mg_per_l)) / MG_CACO3_PER_DEGREE).max(0.0)
    }

    pub fn kh_d(&self, volume_l: f64) -> f64 {
        (self.alkalinity_meq_per_l(volume_l) * MG_CACO3_PER_MEQ / MG_CACO3_PER_DEGREE).max(0.0)
    }

    pub fn tds_mg_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.total_tracked_ions_mg(), volume_l)
    }

    pub fn conductivity_us_cm(&self, volume_l: f64) -> f64 {
        (self.tds_mg_per_l(volume_l) / 0.65).max(0.0)
    }

    pub fn concentration_view(&self, volume_l: f64) -> ConcentrationView<'_> {
        ConcentrationView {
            water: self,
            volume_l: volume_l.max(0.0),
        }
    }

    pub fn rescale_totals_for_volume(&mut self, old_volume_l: f64, new_volume_l: f64) {
        let scale = if old_volume_l > f64::EPSILON && old_volume_l.is_finite() {
            (new_volume_l.max(0.0) / old_volume_l).max(0.0)
        } else {
            0.0
        };

        self.ammonia_total_mg_n_total *= scale;
        self.nitrite_mg_n_total *= scale;
        self.nitrate_mg_n_total *= scale;
        self.phosphate_mg_p_total *= scale;
        self.dissolved_oxygen_mg_total *= scale;
        self.dissolved_inorganic_carbon_mg_c_total *= scale;
        self.dissolved_organic_carbon_mg_c_total *= scale;
        self.dissolved_organic_nitrogen_mg_n_total *= scale;
        self.alkalinity_meq_total *= scale;
        self.calcium_mg_total *= scale;
        self.magnesium_mg_total *= scale;
        self.sodium_mg_total *= scale;
        self.potassium_mg_total *= scale;
        self.chloride_mg_total *= scale;
        self.sulfate_mg_total *= scale;

        crate::systems::chemistry::resolve_carbonate_state(self, new_volume_l.max(0.0));
    }

    /// Returns the ion-total cache used for TDS/conductivity.
    ///
    /// `bicarbonate_mg_total` is solver-derived, so callers must ensure
    /// `resolve_carbonate_state()` has run after the most recent DIC or
    /// alkalinity mutation before reading this aggregate.
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
        let geometry = TankGeometry::default();
        Self::default_for_geometry(&geometry, SubstrateLayerState::default().depth_cm)
    }
}

impl<'a> ConcentrationView<'a> {
    pub fn volume_l(&self) -> f64 {
        self.volume_l
    }

    pub fn tan_mg_n_per_l(&self) -> f64 {
        self.water.tan_mg_n_per_l(self.volume_l)
    }

    pub fn nitrite_mg_n_per_l(&self) -> f64 {
        self.water.nitrite_mg_n_per_l(self.volume_l)
    }

    pub fn nitrate_mg_n_per_l(&self) -> f64 {
        self.water.nitrate_mg_n_per_l(self.volume_l)
    }

    pub fn don_mg_n_per_l(&self) -> f64 {
        self.water.don_mg_n_per_l(self.volume_l)
    }

    pub fn doc_mg_c_per_l(&self) -> f64 {
        self.water.doc_mg_c_per_l(self.volume_l)
    }

    pub fn dic_mg_c_per_l(&self) -> f64 {
        self.water.dic_mg_c_per_l(self.volume_l)
    }

    pub fn do_mg_per_l(&self) -> f64 {
        self.water.do_mg_per_l(self.volume_l)
    }

    pub fn phosphate_mg_p_per_l(&self) -> f64 {
        self.water.phosphate_mg_p_per_l(self.volume_l)
    }

    pub fn alkalinity_meq_per_l(&self) -> f64 {
        self.water.alkalinity_meq_per_l(self.volume_l)
    }

    pub fn calcium_mg_per_l(&self) -> f64 {
        self.water.calcium_mg_per_l(self.volume_l)
    }

    pub fn magnesium_mg_per_l(&self) -> f64 {
        self.water.magnesium_mg_per_l(self.volume_l)
    }

    pub fn gh_d(&self) -> f64 {
        self.water.gh_d(self.volume_l)
    }

    pub fn kh_d(&self) -> f64 {
        self.water.kh_d(self.volume_l)
    }

    pub fn tds_mg_per_l(&self) -> f64 {
        self.water.tds_mg_per_l(self.volume_l)
    }

    pub fn conductivity_us_cm(&self) -> f64 {
        self.water.conductivity_us_cm(self.volume_l)
    }
}

pub(crate) fn concentration_from_total(total: f64, volume_l: f64) -> f64 {
    if !total.is_finite() || !volume_l.is_finite() || volume_l <= f64::EPSILON {
        0.0
    } else {
        (total / volume_l).max(0.0)
    }
}
