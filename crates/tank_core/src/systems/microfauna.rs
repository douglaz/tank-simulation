use crate::types::{
    algae_carbon_mg, algae_nitrogen_mg, detritus_carbon_mg, detritus_nitrogen_mg, TankState,
};

/// Stoichiometric O2:C for organic matter oxidation (32/12 ≈ 2.67).
const O2_PER_MG_C_RESPIRED: f64 = 2.67;

/// Molar mass of nitrogen for alkalinity coupling (mg N per meq NH4+).
const MG_N_PER_MEQ_AMMONIA: f64 = 14.007;

/// Daily microfauna turnover: updates population_index and grazing_pressure_index
/// from detritus/periphyton resource availability and shrimp grazing pressure.
///
/// Microfauna effects:
/// - Modestly improves mineralization efficiency (applied in nitrogen_cycle)
/// - Consumes some periphyton/bacterial resource
/// - Declines under heavy shrimp grazing
///
/// Consumed material follows the consumer routing contract:
/// - feces → fine_detritus_g_total
/// - excretion → TAN (N) + DOC (C)
/// - respiration → DIC (C) + O2 demand
/// - retained → microfauna.reserve_g
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
    let periphyton_consumed = state.algae.periphyton_biomass_g * consumption_fraction;
    state.algae.periphyton_biomass_g =
        (state.algae.periphyton_biomass_g - periphyton_consumed).max(0.0);

    // Microfauna also process some fine detritus (modest)
    let detritus_consumed =
        state.detritus.fine_detritus_g_total * 0.01 * state.microfauna.population_index;
    state.detritus.fine_detritus_g_total =
        (state.detritus.fine_detritus_g_total - detritus_consumed).max(0.0);

    // Convert consumed biomass to elemental mg for routing.
    // Periphyton uses algae stoichiometry; detritus uses organic-matter stoichiometry.
    let consumed_n_mg = algae_nitrogen_mg(periphyton_consumed)
        + detritus_nitrogen_mg(detritus_consumed, n_to_c_ratio);
    let consumed_c_mg = algae_carbon_mg(periphyton_consumed, n_to_c_ratio)
        + detritus_carbon_mg(detritus_consumed, n_to_c_ratio);

    route_consumed_food(state, consumed_n_mg, consumed_c_mg);
}

/// Route consumed food through the consumer routing contract.
///
/// Destinations:
/// - feces → fine_detritus_g_total (as organic matter grams)
/// - excreted N → ammonia_total_mg_n_total (TAN)
/// - excreted C → dissolved_organic_carbon_mg_c_total (DOC)
/// - respired C → dissolved_inorganic_carbon_mg_c_total (DIC)
/// - respired → O2 demand (dissolved_oxygen_mg_total)
/// - retained → microfauna.reserve_g (organic matter grams)
fn route_consumed_food(state: &mut TankState, consumed_n_mg: f64, consumed_c_mg: f64) {
    if consumed_n_mg <= f64::EPSILON && consumed_c_mg <= f64::EPSILON {
        return;
    }

    let params = &state.process_params;
    let ae = params.microfauna_assimilation_efficiency;
    let fecal_fraction = 1.0 - ae;
    let resp_frac = params.microfauna_respiration_fraction_of_assimilated;
    let excr_frac = params.microfauna_excretion_fraction_of_assimilated;
    let growth_frac = params.microfauna_growth_fraction_of_assimilated;

    // ── Feces: unassimilated share → fine detritus ──
    let fecal_n_mg = consumed_n_mg * fecal_fraction;
    let fecal_c_mg = consumed_c_mg * fecal_fraction;
    let fecal_mass_g = (fecal_n_mg + fecal_c_mg) / 1000.0;
    state.detritus.fine_detritus_g_total += fecal_mass_g;

    // ── Assimilated share ──
    let assimilated_n_mg = consumed_n_mg * ae;
    let assimilated_c_mg = consumed_c_mg * ae;

    // ── Excretion: TAN + DOC ──
    let excreted_n_mg = assimilated_n_mg * excr_frac;
    state.water.ammonia_total_mg_n_total += excreted_n_mg;
    state.water.alkalinity_meq_total += excreted_n_mg / MG_N_PER_MEQ_AMMONIA;

    let excreted_c_mg = assimilated_c_mg * excr_frac;
    state.water.dissolved_organic_carbon_mg_c_total += excreted_c_mg;

    // ── Respiration: O2 demand + DIC ──
    // O2-limited: only oxidize the share that available O2 can support.
    let target_respired_c_mg = assimilated_c_mg * resp_frac;
    let o2_demand_mg = target_respired_c_mg * O2_PER_MG_C_RESPIRED;
    let actual_o2_consumed_mg = state
        .water
        .dissolved_oxygen_mg_total
        .max(0.0)
        .min(o2_demand_mg);
    let respiration_scale = if o2_demand_mg > f64::EPSILON {
        actual_o2_consumed_mg / o2_demand_mg
    } else {
        1.0
    };
    let respired_c_mg = target_respired_c_mg * respiration_scale;
    state.water.dissolved_inorganic_carbon_mg_c_total += respired_c_mg;
    state.water.dissolved_oxygen_mg_total =
        (state.water.dissolved_oxygen_mg_total - actual_o2_consumed_mg).max(0.0);

    let target_respired_n_mg = assimilated_n_mg * resp_frac;
    let respired_n_mg = target_respired_n_mg * respiration_scale;
    state.water.ammonia_total_mg_n_total += respired_n_mg;
    state.water.alkalinity_meq_total += respired_n_mg / MG_N_PER_MEQ_AMMONIA;

    // ── Retained: microfauna reserve ──
    let retained_n_mg = assimilated_n_mg * growth_frac + (target_respired_n_mg - respired_n_mg);
    let retained_c_mg = assimilated_c_mg * growth_frac + (target_respired_c_mg - respired_c_mg);
    let retained_mass_g = (retained_n_mg + retained_c_mg) / 1000.0;
    state.microfauna.reserve_g += retained_mass_g;
}
