use serde::{Deserialize, Serialize};

use super::{SourceWaterProfile, SubstrateLayerState, TankGeometry};
use crate::systems::chemistry::{
    bicarbonate_mg_total_from_mmol_per_l, solve_carbonate_equilibrium, CarbonateEquilibrium,
};

/// Equivalent weight of CaCO3 on the conventional alkalinity/hardness scale, in mg/meq.
const MG_CACO3_PER_MEQ: f64 = 50.0;
/// One German degree of hardness, expressed as mg/L CaCO3.
const MG_CACO3_PER_DEGREE: f64 = 17.848;
/// Molar mass of CaCO3, used to express Ca2+/Mg2+ hardness on a CaCO3 basis.
const MOLAR_MASS_CACO3_G_PER_MOL: f64 = 100.0869;
/// Atomic mass of calcium for Ca2+ -> CaCO3 hardness conversion.
const MOLAR_MASS_CALCIUM_G_PER_MOL: f64 = 40.078;
/// Atomic mass of magnesium for Mg2+ -> CaCO3 hardness conversion.
const MOLAR_MASS_MAGNESIUM_G_PER_MOL: f64 = 24.305;
/// Converts mg/L Ca2+ to mg/L as CaCO3 for GH reporting.
const CALCIUM_AS_CACO3_FACTOR: f64 = MOLAR_MASS_CACO3_G_PER_MOL / MOLAR_MASS_CALCIUM_G_PER_MOL;
/// Converts mg/L Mg2+ to mg/L as CaCO3 for GH reporting.
const MAGNESIUM_AS_CACO3_FACTOR: f64 = MOLAR_MASS_CACO3_G_PER_MOL / MOLAR_MASS_MAGNESIUM_G_PER_MOL;
/// Empirical freshwater TDS-to-conductivity divisor for the current 7-ion proxy.
///
/// This is an approximate ppm-to-uS/cm conversion used for display only; it is
/// not a physical conductivity solver and should be interpreted as an estimate.
pub(crate) const ESTIMATED_TDS_TO_CONDUCTIVITY_DIVISOR: f64 = 0.65;

/// Major ions currently counted by the TDS/conductivity display estimate.
pub const ESTIMATED_TDS_TRACKED_MAJOR_IONS: [&str; 7] =
    ["Ca", "Mg", "Na", "K", "HCO3", "Cl", "SO4"];

/// Important contributors intentionally omitted from the current 7-ion estimate.
pub const ESTIMATED_TDS_OMITTED_CONTRIBUTORS: [&str; 3] = [
    "tracked TAN/NH3, nitrite, nitrate, and phosphate species are excluded from this estimate",
    "trace ions and micronutrients",
    "dissolved organics and other untracked solutes",
];

/// Short TUI-ready summary lines describing the 7-ion estimate scope.
pub const ESTIMATED_TDS_SCOPE_LINES: [&str; 3] = [
    "7 ions: Ca Mg Na K HCO3 Cl SO4",
    "Omits TAN/NH3 NO2 NO3 PO4",
    "Omits organics + trace ions",
];

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EstimatedDissolvedSolids {
    pub(crate) bicarbonate_mg_total: f64,
    pub(crate) tds_mg_per_l: f64,
    pub(crate) conductivity_us_cm: f64,
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
            bicarbonate_mg_total: 0.0, // solver-derived; set by resolve_carbonate_state below
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
            bicarbonate_mg_total: 0.0, // solver-derived; set by resolve_carbonate_state below
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

    pub fn chloride_mg_per_l(&self, volume_l: f64) -> f64 {
        concentration_from_total(self.chloride_mg_total, volume_l)
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

        (((CALCIUM_AS_CACO3_FACTOR * calcium_mg_per_l)
            + (MAGNESIUM_AS_CACO3_FACTOR * magnesium_mg_per_l))
            / MG_CACO3_PER_DEGREE)
            .max(0.0)
    }

    pub fn kh_d(&self, volume_l: f64) -> f64 {
        (self.alkalinity_meq_per_l(volume_l) * MG_CACO3_PER_MEQ / MG_CACO3_PER_DEGREE).max(0.0)
    }

    pub fn tds_mg_per_l(&self, volume_l: f64) -> f64 {
        self.estimated_dissolved_solids(volume_l).tds_mg_per_l
    }

    pub fn conductivity_us_cm(&self, volume_l: f64) -> f64 {
        self.estimated_dissolved_solids(volume_l).conductivity_us_cm
    }

    pub(crate) fn projected_carbonate_equilibrium(&self, volume_l: f64) -> CarbonateEquilibrium {
        solve_carbonate_equilibrium(
            self.dissolved_inorganic_carbon_mg_c_total,
            self.alkalinity_meq_total,
            self.temperature_c,
            volume_l,
        )
    }

    pub(crate) fn estimated_dissolved_solids(&self, volume_l: f64) -> EstimatedDissolvedSolids {
        self.estimated_dissolved_solids_with_carbonate_equilibrium(
            volume_l,
            self.projected_carbonate_equilibrium(volume_l),
        )
    }

    pub(crate) fn estimated_dissolved_solids_with_carbonate_equilibrium(
        &self,
        volume_l: f64,
        carbonate_eq: CarbonateEquilibrium,
    ) -> EstimatedDissolvedSolids {
        let bicarbonate_mg_total =
            bicarbonate_mg_total_from_mmol_per_l(carbonate_eq.hco3_mmol_per_l, volume_l);
        let tds_mg_per_l = self.tds_mg_per_l_with_bicarbonate_total(volume_l, bicarbonate_mg_total);
        let conductivity_us_cm =
            self.conductivity_us_cm_with_bicarbonate_total(volume_l, bicarbonate_mg_total);

        EstimatedDissolvedSolids {
            bicarbonate_mg_total,
            tds_mg_per_l,
            conductivity_us_cm,
        }
    }

    /// Computes the estimated 7-ion TDS using an explicitly supplied
    /// bicarbonate total instead of the cached `bicarbonate_mg_total`.
    ///
    /// This lets callers reuse a fresh carbonate solve before the cache has
    /// been synchronized back onto `WaterState`.
    pub(crate) fn tds_mg_per_l_with_bicarbonate_total(
        &self,
        volume_l: f64,
        bicarbonate_mg_total: f64,
    ) -> f64 {
        concentration_from_total(
            self.total_tracked_ions_mg_with_bicarbonate_total(bicarbonate_mg_total),
            volume_l,
        )
    }

    /// Computes the estimated conductivity from the same 7-ion proxy while
    /// allowing callers to override the bicarbonate contribution.
    pub(crate) fn conductivity_us_cm_with_bicarbonate_total(
        &self,
        volume_l: f64,
        bicarbonate_mg_total: f64,
    ) -> f64 {
        (self.tds_mg_per_l_with_bicarbonate_total(volume_l, bicarbonate_mg_total)
            / ESTIMATED_TDS_TO_CONDUCTIVITY_DIVISOR)
            .max(0.0)
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

    /// Returns the cached 7-ion total based on the stored bicarbonate field.
    ///
    /// This is useful for inspecting persisted solver state, but the public
    /// TDS/conductivity accessors recompute a fresh carbonate projection so
    /// they stay consistent with snapshot output even when the cache is stale.
    pub fn total_tracked_ions_mg(&self) -> f64 {
        self.total_tracked_ions_mg_with_bicarbonate_total(self.bicarbonate_mg_total)
    }

    pub(crate) fn total_tracked_ions_mg_with_bicarbonate_total(
        &self,
        bicarbonate_mg_total: f64,
    ) -> f64 {
        self.calcium_mg_total
            + self.magnesium_mg_total
            + self.sodium_mg_total
            + self.potassium_mg_total
            + bicarbonate_mg_total.max(0.0)
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

    pub fn chloride_mg_per_l(&self) -> f64 {
        self.water.chloride_mg_per_l(self.volume_l)
    }
}

pub(crate) fn concentration_from_total(total: f64, volume_l: f64) -> f64 {
    if !total.is_finite() || !volume_l.is_finite() || volume_l <= f64::EPSILON {
        0.0
    } else {
        (total / volume_l).max(0.0)
    }
}
