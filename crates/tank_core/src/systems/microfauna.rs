use crate::types::{
    algae_carbon_mg, algae_nitrogen_mg, detritus_carbon_mg, detritus_nitrogen_mg, TankState,
};

/// Daily microfauna turnover: updates population_index and grazing_pressure_index
/// from detritus/periphyton resource availability and shrimp grazing pressure.
///
/// Microfauna effects:
/// - Modestly improves mineralization efficiency (applied in nitrogen_cycle)
/// - Consumes some periphyton/bacterial resource
/// - Declines under heavy shrimp grazing
pub fn step_daily_microfauna(state: &mut TankState) {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let pp = &state.process_params;
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;

    // Resource availability (detritus + periphyton as food for microfauna)
    // Reference densities are generous so microfauna don't over-respond to
    // short-term feeding pulses.
    let detritus_density = state.detritus.fine_detritus_g_total / volume_l;
    let detritus_resource = (detritus_density / 2.0).clamp(0.0, 1.0);
    let periphyton_density = state.algae.periphyton_biomass_g / volume_l;
    let periphyton_resource = (periphyton_density / 1.0).clamp(0.0, 1.0);
    let resource_availability =
        (0.5 * detritus_resource + 0.5 * periphyton_resource).clamp(0.0, 1.0);

    // Shrimp grazing pressure on microfauna
    let total_shrimp = (state.animal.adults_count + state.animal.juveniles_count) as f64;
    let shrimp_density = total_shrimp / volume_l;
    let shrimp_pressure =
        (shrimp_density / pp.microfauna_shrimp_pressure_threshold.max(0.01)).clamp(0.0, 1.0);

    // Population target: grows with resources, declines with shrimp pressure
    let growth_target = resource_availability * (1.0 - 0.6 * shrimp_pressure);
    let smoothing = pp.microfauna_population_smoothing;
    state.microfauna.population_index = (state.microfauna.population_index
        + smoothing * (growth_target - state.microfauna.population_index))
        .clamp(0.0, 1.0);

    // Grazing pressure reflects population and available food
    state.microfauna.grazing_pressure_index =
        (state.microfauna.population_index * resource_availability.sqrt()).clamp(0.0, 1.0);

    // Microfauna consume some periphyton (without driving it negative)
    let consumption_fraction =
        pp.microfauna_periphyton_consumption * state.microfauna.population_index;
    let consumed = state.algae.periphyton_biomass_g * consumption_fraction;
    state.algae.periphyton_biomass_g = (state.algae.periphyton_biomass_g - consumed).max(0.0);
    route_algae_loss_to_dissolved_organics(state, consumed, n_to_c_ratio);

    // Microfauna also process some fine detritus (modest)
    let detritus_consumed =
        state.detritus.fine_detritus_g_total * 0.01 * state.microfauna.population_index;
    state.detritus.fine_detritus_g_total =
        (state.detritus.fine_detritus_g_total - detritus_consumed).max(0.0);
    route_detritus_loss_to_dissolved_organics(state, detritus_consumed, n_to_c_ratio);
}

fn route_algae_loss_to_dissolved_organics(
    state: &mut TankState,
    biomass_g: f64,
    n_to_c_ratio: f64,
) {
    if biomass_g <= f64::EPSILON {
        return;
    }

    state.water.dissolved_organic_nitrogen_mg_n_total += algae_nitrogen_mg(biomass_g);
    state.water.dissolved_organic_carbon_mg_c_total += algae_carbon_mg(biomass_g, n_to_c_ratio);
}

fn route_detritus_loss_to_dissolved_organics(
    state: &mut TankState,
    mass_g: f64,
    n_to_c_ratio: f64,
) {
    if mass_g <= f64::EPSILON {
        return;
    }

    state.water.dissolved_organic_nitrogen_mg_n_total += detritus_nitrogen_mg(mass_g, n_to_c_ratio);
    state.water.dissolved_organic_carbon_mg_c_total += detritus_carbon_mg(mass_g, n_to_c_ratio);
}
