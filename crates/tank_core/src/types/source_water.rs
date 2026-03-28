use serde::{Deserialize, Serialize};

use super::SimError;
use crate::systems::chemistry::{
    validate_source_water_carbonate_profile, SourceWaterCarbonateValidationError,
};

/// Runtime source-water profile with explicit per-liter chemistry.
/// Stored in `TankState.source_water_catalog` for deterministic continuation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceWaterProfile {
    pub temperature_c: f64,
    pub ammonia_mg_n_per_l: f64,
    pub nitrite_mg_n_per_l: f64,
    pub nitrate_mg_n_per_l: f64,
    pub phosphate_mg_p_per_l: f64,
    pub dic_mg_c_per_l: f64,
    pub doc_mg_c_per_l: f64,
    pub don_mg_n_per_l: f64,
    pub alkalinity_meq_per_l: f64,
    pub calcium_mg_per_l: f64,
    pub magnesium_mg_per_l: f64,
    pub sodium_mg_per_l: f64,
    pub potassium_mg_per_l: f64,
    /// Display/documentation field only. Runtime carbonate chemistry uses DIC,
    /// alkalinity, and temperature as the authoritative inputs and immediately
    /// re-derives bicarbonate when materializing water state or snapshots.
    pub bicarbonate_mg_per_l: f64,
    pub chloride_mg_per_l: f64,
    pub sulfate_mg_per_l: f64,
}

impl SourceWaterProfile {
    /// Validates that all chemistry fields are finite and non-negative,
    /// and temperature is finite and above 0.0 C.
    /// Returns `Ok(())` if the profile is valid for use in water changes.
    pub fn validate(&self, profile_id: &str) -> Result<(), SimError> {
        let fields: &[(&str, f64)] = &[
            ("ammonia_mg_n_per_l", self.ammonia_mg_n_per_l),
            ("nitrite_mg_n_per_l", self.nitrite_mg_n_per_l),
            ("nitrate_mg_n_per_l", self.nitrate_mg_n_per_l),
            ("phosphate_mg_p_per_l", self.phosphate_mg_p_per_l),
            ("dic_mg_c_per_l", self.dic_mg_c_per_l),
            ("doc_mg_c_per_l", self.doc_mg_c_per_l),
            ("don_mg_n_per_l", self.don_mg_n_per_l),
            ("alkalinity_meq_per_l", self.alkalinity_meq_per_l),
            ("calcium_mg_per_l", self.calcium_mg_per_l),
            ("magnesium_mg_per_l", self.magnesium_mg_per_l),
            ("sodium_mg_per_l", self.sodium_mg_per_l),
            ("potassium_mg_per_l", self.potassium_mg_per_l),
            ("bicarbonate_mg_per_l", self.bicarbonate_mg_per_l),
            ("chloride_mg_per_l", self.chloride_mg_per_l),
            ("sulfate_mg_per_l", self.sulfate_mg_per_l),
        ];
        for &(field, value) in fields {
            if !value.is_finite() || value < 0.0 {
                return Err(SimError::InvalidSourceProfile {
                    id: profile_id.to_string(),
                    field,
                    value,
                });
            }
        }
        if !self.temperature_c.is_finite() || self.temperature_c <= 0.0 {
            return Err(SimError::InvalidSourceProfile {
                id: profile_id.to_string(),
                field: "temperature_c",
                value: self.temperature_c,
            });
        }
        if let Err(err) = validate_source_water_carbonate_profile(
            self.dic_mg_c_per_l,
            self.alkalinity_meq_per_l,
            self.temperature_c,
        ) {
            return Err(SimError::InvalidSourceProfile {
                id: profile_id.to_string(),
                field: "carbonate_derived_ph",
                value: match err {
                    SourceWaterCarbonateValidationError::OutOfRangePh(ph) => ph,
                    SourceWaterCarbonateValidationError::NonFiniteNeutralFallback => f64::NAN,
                },
            });
        }
        Ok(())
    }

    /// Creates a zero-nutrient profile (equivalent to pure RO water).
    pub fn zero() -> Self {
        Self {
            temperature_c: 23.0,
            ammonia_mg_n_per_l: 0.0,
            nitrite_mg_n_per_l: 0.0,
            nitrate_mg_n_per_l: 0.0,
            phosphate_mg_p_per_l: 0.0,
            dic_mg_c_per_l: 0.0,
            doc_mg_c_per_l: 0.0,
            don_mg_n_per_l: 0.0,
            alkalinity_meq_per_l: 0.0,
            calcium_mg_per_l: 0.0,
            magnesium_mg_per_l: 0.0,
            sodium_mg_per_l: 0.0,
            potassium_mg_per_l: 0.0,
            bicarbonate_mg_per_l: 0.0,
            chloride_mg_per_l: 0.0,
            sulfate_mg_per_l: 0.0,
        }
    }
}
