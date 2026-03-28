use crate::{
    systems::{chemistry::resolve_carbonate_state, temperature::do_sat_mg_l},
    types::{
        algae_carbon_mg, algae_nitrogen_mg, BudgetDelta, ElementBudget, SimError,
        SourceWaterProfile, TankState,
    },
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

    let volume_l = state.water_volume_l();
    let fraction = percent / 100.0;
    let retention = 1.0 - fraction;
    let exchanged_l = volume_l * fraction;

    // Remove fraction of each dissolved total and suspended biomass
    state.algae.suspended_biomass_g *= retention;
    state.water.ammonia_total_mg_n_total *= retention;
    state.water.nitrite_mg_n_total *= retention;
    state.water.nitrate_mg_n_total *= retention;
    state.water.phosphate_mg_p_total *= retention;
    state.water.dissolved_inorganic_carbon_mg_c_total *= retention;
    state.water.dissolved_organic_carbon_mg_c_total *= retention;
    state.water.dissolved_organic_nitrogen_mg_n_total *= retention;
    // This is only a shadow counter that mirrors DOC/DON-origin residue mass.
    // The real exported C/N is already accounted through the dissolved pools,
    // so this bookkeeping adjustment must stay out of the explicit budget delta.
    state.detritus.dissolved_feed_residue_g_total *= retention;
    state.water.dissolved_oxygen_mg_total *= retention;
    state.water.alkalinity_meq_total *= retention;
    state.water.calcium_mg_total *= retention;
    state.water.magnesium_mg_total *= retention;
    state.water.sodium_mg_total *= retention;
    state.water.potassium_mg_total *= retention;
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
    state.water.chloride_mg_total += source.chloride_mg_per_l * exchanged_l;
    state.water.sulfate_mg_total += source.sulfate_mg_per_l * exchanged_l;

    // Mix temperature proportionally by exchanged volume
    state.water.temperature_c =
        state.water.temperature_c * retention + source.temperature_c * fraction;

    // Resolve carbonate equilibrium so downstream systems in the same tick
    // (e.g. nitrification) use the post-change pH and bicarbonate values.
    resolve_carbonate_state(&mut state.water, volume_l);
}

pub fn apply_water_change_with_budget(
    state: &mut TankState,
    percent: f64,
    source: &SourceWaterProfile,
) -> BudgetDelta {
    if percent <= 0.0 {
        return BudgetDelta::default();
    }

    let fraction = percent / 100.0;
    let exchanged_l = state.water_volume_l() * fraction;
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;

    let nitrogen_out_mg = fraction
        * (state.water.ammonia_total_mg_n_total
            + state.water.nitrite_mg_n_total
            + state.water.nitrate_mg_n_total
            + state.water.dissolved_organic_nitrogen_mg_n_total
            + algae_nitrogen_mg(state.algae.suspended_biomass_g));
    let carbon_out_mg = fraction
        * (state.water.dissolved_inorganic_carbon_mg_c_total
            + state.water.dissolved_organic_carbon_mg_c_total
            + algae_carbon_mg(state.algae.suspended_biomass_g, n_to_c_ratio));
    let oxygen_out_mg = fraction * state.water.dissolved_oxygen_mg_total.max(0.0);

    let nitrogen_in_mg = exchanged_l
        * (source.ammonia_mg_n_per_l
            + source.nitrite_mg_n_per_l
            + source.nitrate_mg_n_per_l
            + source.don_mg_n_per_l);
    let carbon_in_mg = exchanged_l * (source.dic_mg_c_per_l + source.doc_mg_c_per_l);
    let oxygen_in_mg = exchanged_l * do_sat_mg_l(source.temperature_c);

    apply_water_change(state, percent, source);

    BudgetDelta {
        nitrogen: ElementBudget {
            in_mg: nitrogen_in_mg,
            out_mg: nitrogen_out_mg,
        },
        carbon: ElementBudget {
            in_mg: carbon_in_mg,
            out_mg: carbon_out_mg,
        },
        oxygen: ElementBudget {
            in_mg: oxygen_in_mg,
            out_mg: oxygen_out_mg,
        },
    }
}
