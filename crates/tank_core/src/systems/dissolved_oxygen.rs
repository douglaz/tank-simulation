use crate::{
    systems::{
        chemistry::{photosynthetic_biomass_g, respiring_biomass_g},
        temperature::do_sat_mg_l,
    },
    types::{BudgetDelta, ElementBudget, TankState},
};

/// Reference surface-area-to-volume ratio (cm²/L) matching the default tank
/// geometry (40×25 cm footprint, 22 L gross − 3 cm substrate ≈ 19 L net).
///
/// The base K_LA process parameters (`reaeration_kla_base`, `aeration_kla_boost`)
/// are calibrated for this ratio.  `compute_o2_kla` normalises by the actual
/// tank's SA/V so that *surface flux* (mg exchanged at the air–water interface)
/// scales with exposed area rather than water volume.
const REFERENCE_SA_V_CM2_PER_L: f64 = 1000.0 / 19.0;

/// Computes the volumetric gas-transfer coefficient K_LA (h⁻¹) for O₂
/// based on surface exchange, aeration, filter agitation, and the tank's
/// surface-area-to-volume ratio.
///
/// The hardware-derived base coefficient is normalised by the ratio of the
/// tank's actual SA/V to the reference SA/V so that total gas flux depends on
/// exposed surface area, not water volume.  Two tanks with the same footprint
/// and hardware but different fill heights exchange the same mass per hour;
/// the shallower tank simply sees a faster concentration change.
///
/// Callers should cap the returned value (e.g., `.min(1.0)`) before using it
/// in an explicit Euler step to prevent overshooting equilibrium.
pub fn compute_o2_kla(state: &TankState) -> f64 {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return 0.0;
    }

    let aeration_intensity = if state.hardware.aeration.enabled {
        state.hardware.aeration.intensity
    } else {
        0.0
    };
    let filter_kla_boost = if state.hardware.filter.enabled {
        0.02 * (state.hardware.filter.flow_lph / 200.0).min(2.0)
    } else {
        0.0
    };
    let raw_kla = (state.process_params.reaeration_kla_base * state.geometry.top_exchange_factor())
        + (state.process_params.aeration_kla_boost * aeration_intensity)
        + filter_kla_boost;

    // Normalise by SA/V so surface flux is area-proportional, not
    // volume-proportional.  For the default geometry this ratio is 1.0.
    let sa_v = state.geometry.surface_area_cm2() / volume_l;
    raw_kla * (sa_v / REFERENCE_SA_V_CM2_PER_L)
}

pub fn step_dissolved_oxygen(state: &mut TankState, light_on: bool) {
    let Some(terms) = dissolved_oxygen_terms(state, light_on) else {
        return;
    };

    apply_dissolved_oxygen_terms(state, terms);
}

pub fn step_dissolved_oxygen_with_budget(state: &mut TankState, light_on: bool) -> BudgetDelta {
    let Some(terms) = dissolved_oxygen_terms(state, light_on) else {
        return BudgetDelta::default();
    };

    BudgetDelta {
        oxygen: apply_dissolved_oxygen_terms(state, terms),
        ..BudgetDelta::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct DissolvedOxygenTerms {
    reaeration_mg: f64,
    photosynthesis_mg: f64,
    respiration_mg: f64,
}

fn dissolved_oxygen_terms(state: &TankState, light_on: bool) -> Option<DissolvedOxygenTerms> {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return None;
    }

    let do_sat_mg_l = do_sat_mg_l(state.water.temperature_c);
    let do_mg_l = state.do_mg_per_l();
    let k_la = compute_o2_kla(state);
    // Cap k_la at 1.0 so the explicit Euler step cannot overshoot saturation.
    // With k_la <= 1.0, gas exchange moves DO at most 100% toward the
    // saturation target per hour, asymptotically approaching but never crossing.
    let delta_do_reaeration_mg = k_la.min(1.0) * (do_sat_mg_l - do_mg_l) * volume_l;

    let background_bod_mg = state
        .process_params
        .background_bod_mg_o2_per_g_biomass_per_hour
        * respiring_biomass_g(state);
    let photosynthetic_o2_mg = if light_on {
        state
            .process_params
            .plant_photosynthesis_o2_mg_per_g_per_hour
            * photosynthetic_biomass_g(state)
            * state.hardware.light.intensity_index
    } else {
        0.0
    };

    Some(DissolvedOxygenTerms {
        reaeration_mg: delta_do_reaeration_mg,
        photosynthesis_mg: photosynthetic_o2_mg,
        respiration_mg: background_bod_mg,
    })
}

fn apply_dissolved_oxygen_terms(
    state: &mut TankState,
    terms: DissolvedOxygenTerms,
) -> ElementBudget {
    let oxygen_before_mg = state.water.dissolved_oxygen_mg_total.max(0.0);
    let oxygen_in_mg = terms.photosynthesis_mg + terms.reaeration_mg.max(0.0);
    let oxygen_out_candidate_mg = terms.respiration_mg + (-terms.reaeration_mg).max(0.0);
    let oxygen_out_mg = oxygen_out_candidate_mg.min(oxygen_before_mg + oxygen_in_mg);

    state.water.dissolved_oxygen_mg_total =
        (oxygen_before_mg + oxygen_in_mg - oxygen_out_mg).max(0.0);

    ElementBudget {
        in_mg: oxygen_in_mg,
        out_mg: oxygen_out_mg,
    }
}
