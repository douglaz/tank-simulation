use crate::{
    systems::{chemistry::compute_ph_from_totals, temperature::do_sat_mg_l},
    types::{SimError, SourceWaterProfile, TankState},
};

pub fn validate_source_profile(state: &TankState, source_profile_id: &str) -> Result<(), SimError> {
    match state.source_water_catalog.get(source_profile_id) {
        None => Err(SimError::UnknownSourceProfile {
            id: source_profile_id.to_string(),
        }),
        Some(profile) => profile.validate(source_profile_id),
    }
}

/// Validates that all water change actions in the queue reference known and valid source profiles.
/// Returns the first error found, or Ok(()) if all are valid.
/// Checks both catalog membership and runtime validity (finite, non-negative chemistry).
pub fn validate_water_changes(
    state: &TankState,
    actions: &[crate::types::PlayerAction],
) -> Result<(), SimError> {
    for action in actions {
        if let crate::types::PlayerAction::WaterChangePercent {
            source_profile_id, ..
        } = action
        {
            validate_source_profile(state, source_profile_id)?;
        }
    }
    Ok(())
}

/// Applies a water change: removes `percent/100` of each dissolved total,
/// then adds replacement mass from the source-water profile using exchanged liters.
/// Also mixes water temperature proportionally by exchanged volume.
///
/// A 0% water change is a no-op.
pub fn apply_water_change(state: &mut TankState, percent: f64, source: &SourceWaterProfile) {
    if percent <= 0.0 {
        return;
    }

    let volume_l = state.geometry.water_volume_l();
    let fraction = percent / 100.0;
    let retention = 1.0 - fraction;
    let exchanged_l = volume_l * fraction;

    // Remove fraction of each dissolved total
    state.water.ammonia_total_mg_n_total *= retention;
    state.water.nitrite_mg_n_total *= retention;
    state.water.nitrate_mg_n_total *= retention;
    state.water.phosphate_mg_p_total *= retention;
    state.water.dissolved_inorganic_carbon_mg_c_total *= retention;
    state.water.dissolved_organic_carbon_mg_c_total *= retention;
    state.water.dissolved_organic_nitrogen_mg_n_total *= retention;
    state.water.dissolved_oxygen_mg_total *= retention;
    state.water.alkalinity_meq_total *= retention;
    state.water.calcium_mg_total *= retention;
    state.water.magnesium_mg_total *= retention;
    state.water.sodium_mg_total *= retention;
    state.water.potassium_mg_total *= retention;
    state.water.bicarbonate_mg_total *= retention;
    state.water.chloride_mg_total *= retention;
    state.water.sulfate_mg_total *= retention;

    // Add replacement mass from source water
    state.water.ammonia_total_mg_n_total += source.ammonia_mg_n_per_l * exchanged_l;
    state.water.nitrite_mg_n_total += source.nitrite_mg_n_per_l * exchanged_l;
    state.water.nitrate_mg_n_total += source.nitrate_mg_n_per_l * exchanged_l;
    state.water.phosphate_mg_p_total += source.phosphate_mg_p_per_l * exchanged_l;
    state.water.dissolved_inorganic_carbon_mg_c_total += source.dic_mg_c_per_l * exchanged_l;
    state.water.dissolved_organic_carbon_mg_c_total += source.doc_mg_c_per_l * exchanged_l;
    state.water.dissolved_organic_nitrogen_mg_n_total += source.don_mg_n_per_l * exchanged_l;
    state.water.dissolved_oxygen_mg_total += do_sat_mg_l(source.temperature_c) * exchanged_l;
    state.water.alkalinity_meq_total += source.alkalinity_meq_per_l * exchanged_l;
    state.water.calcium_mg_total += source.calcium_mg_per_l * exchanged_l;
    state.water.magnesium_mg_total += source.magnesium_mg_per_l * exchanged_l;
    state.water.sodium_mg_total += source.sodium_mg_per_l * exchanged_l;
    state.water.potassium_mg_total += source.potassium_mg_per_l * exchanged_l;
    state.water.bicarbonate_mg_total += source.bicarbonate_mg_per_l * exchanged_l;
    state.water.chloride_mg_total += source.chloride_mg_per_l * exchanged_l;
    state.water.sulfate_mg_total += source.sulfate_mg_per_l * exchanged_l;

    // Mix temperature proportionally by exchanged volume
    state.water.temperature_c =
        state.water.temperature_c * retention + source.temperature_c * fraction;

    // Recompute pH from the new alkalinity/DIC so downstream systems in the
    // same tick (e.g. nitrification) use the post-change value.
    state.water.ph = compute_ph_from_totals(
        state.water.alkalinity_meq_total,
        state.water.dissolved_inorganic_carbon_mg_c_total,
        volume_l,
    );
}
