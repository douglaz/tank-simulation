use crate::{
    systems::{
        chemistry::{photosynthetic_biomass_g, respiring_biomass_g},
        temperature::do_sat_mg_l,
    },
    types::TankState,
};

pub fn step_dissolved_oxygen(state: &mut TankState, light_on: bool) {
    let volume_l = state.geometry.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let do_sat_mg_l = do_sat_mg_l(state.water.temperature_c);
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;
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
    let k_la = (state.process_params.reaeration_kla_base * state.geometry.top_exchange_factor())
        + (state.process_params.aeration_kla_boost * aeration_intensity)
        + filter_kla_boost;
    let delta_do_reaeration_mg = k_la * (do_sat_mg_l - do_mg_l) * volume_l;

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

    state.water.dissolved_oxygen_mg_total +=
        delta_do_reaeration_mg + photosynthetic_o2_mg - background_bod_mg;
    // Floor at zero so downstream stress/event paths never see negative DO.
    state.water.dissolved_oxygen_mg_total = state.water.dissolved_oxygen_mg_total.max(0.0);
}
