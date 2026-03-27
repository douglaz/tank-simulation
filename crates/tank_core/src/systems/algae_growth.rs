use crate::{
    systems::events,
    types::{total_colonizable_area_cm2, TankState},
};

const ALGAE_N_MG_PER_G_GROWTH: f64 = 35.0;
const ALGAE_P_MG_PER_G_GROWTH: f64 = 5.0;

pub fn step_daily_algae(state: &mut TankState) {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let previous_nuisance_index = state.algae.nuisance_index;
    let light_factor = algae_light_factor(state);
    let temp_factor = gaussian_response(
        state.water.temperature_c,
        state.process_params.algae_temp_optimum_c,
        state.process_params.algae_temp_sigma_c,
    );
    let plant_shading = (1.0
        - state
            .plant_guilds
            .iter()
            .map(|plant| plant.crowding_index)
            .sum::<f64>()
            / state.plant_guilds.len().max(1) as f64
            * 0.7)
        .clamp(0.2, 1.0);
    let nutrient_factor = half_saturation(
        state.water.ammonia_total_mg_n_total + state.water.nitrate_mg_n_total,
        state.process_params.algae_half_saturation_n_mg_total,
    )
    .min(half_saturation(
        state.water.phosphate_mg_p_total,
        state.process_params.algae_half_saturation_p_mg_total,
    ));
    let microfauna_grazing = (state.microfauna.population_index
        * (0.3 + 0.7 * state.microfauna.grazing_pressure_index))
        .clamp(0.0, 1.0);

    let suspended_seed_g = state.algae.suspended_biomass_g.max(0.02 * volume_l / 10.0);
    let suspended_gross_growth_g = suspended_seed_g
        * state.process_params.algae_max_growth_rate_per_day
        * light_factor
        * plant_shading
        * temp_factor
        * nutrient_factor;
    let suspended_respiration_g =
        state.algae.suspended_biomass_g * state.process_params.algae_respiration_fraction_per_day;
    let suspended_grazing_g = state.algae.suspended_biomass_g * 0.03 * microfauna_grazing;
    let (susp_nh3_removed, susp_no3_removed, susp_p_removed) = consume_algae_nutrients(
        state,
        suspended_gross_growth_g * ALGAE_N_MG_PER_G_GROWTH,
        suspended_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH,
    );
    let susp_n_removed = susp_nh3_removed + susp_no3_removed;
    let susp_n_demand = suspended_gross_growth_g * ALGAE_N_MG_PER_G_GROWTH;
    let susp_p_demand = suspended_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH;
    let susp_cap_frac = if suspended_gross_growth_g > f64::EPSILON {
        let n_frac = if susp_n_demand > f64::EPSILON {
            susp_n_removed / susp_n_demand
        } else {
            1.0
        };
        let p_frac = if susp_p_demand > f64::EPSILON {
            susp_p_removed / susp_p_demand
        } else {
            1.0
        };
        n_frac.min(p_frac)
    } else {
        0.0
    };
    let susp_cap = suspended_gross_growth_g * susp_cap_frac;
    // Refund only the non-limiting nutrient's excess.
    let susp_n_used = susp_n_demand * susp_cap_frac;
    let susp_p_used = susp_p_demand * susp_cap_frac;
    let susp_n_refund = (susp_n_removed - susp_n_used).max(0.0);
    let susp_p_refund = (susp_p_removed - susp_p_used).max(0.0);
    // Split N refund proportionally across ammonia/nitrate.
    let susp_n_frac = if susp_n_removed > f64::EPSILON {
        susp_n_refund / susp_n_removed
    } else {
        0.0
    };
    refund_algae_nutrients(
        state,
        susp_nh3_removed * susp_n_frac,
        susp_no3_removed * susp_n_frac,
        susp_p_refund,
    );
    let suspended_new_g = (state.algae.suspended_biomass_g + susp_cap
        - suspended_respiration_g
        - suspended_grazing_g)
        .max(0.0);
    state.algae.suspended_biomass_g = suspended_new_g;

    let colonizable_area_m2 =
        total_colonizable_area_cm2(&state.substrate_layers, state.geometry.wall_area_cm2())
            / 10_000.0;
    let grazing_surface_factor =
        0.7 + (0.3 * state.avg_substrate_index(|layer| layer.grazing_surface_index));
    let periphyton_capacity_g = colonizable_area_m2.max(0.0)
        * state.process_params.periphyton_capacity_g_per_m2
        * grazing_surface_factor;
    let surface_cap_factor = if periphyton_capacity_g <= f64::EPSILON {
        0.0
    } else {
        (1.0 - (state.algae.periphyton_biomass_g / periphyton_capacity_g)).clamp(0.0, 1.0)
    };
    let periphyton_seed_g = state.algae.periphyton_biomass_g.max(0.05);
    let periphyton_gross_growth_g = periphyton_seed_g
        * state.process_params.periphyton_max_growth_rate_per_day
        * light_factor
        * plant_shading
        * temp_factor
        * nutrient_factor
        * surface_cap_factor;
    let periphyton_respiration_g =
        state.algae.periphyton_biomass_g * state.process_params.algae_respiration_fraction_per_day;
    let periphyton_grazing_g = state.algae.periphyton_biomass_g * 0.06 * microfauna_grazing;
    let (peri_nh3_removed, peri_no3_removed, peri_p_removed) = consume_algae_nutrients(
        state,
        periphyton_gross_growth_g * ALGAE_N_MG_PER_G_GROWTH,
        periphyton_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH,
    );
    let peri_n_removed = peri_nh3_removed + peri_no3_removed;
    let peri_n_demand = periphyton_gross_growth_g * ALGAE_N_MG_PER_G_GROWTH;
    let peri_p_demand = periphyton_gross_growth_g * ALGAE_P_MG_PER_G_GROWTH;
    let peri_cap_frac = if periphyton_gross_growth_g > f64::EPSILON {
        let n_frac = if peri_n_demand > f64::EPSILON {
            peri_n_removed / peri_n_demand
        } else {
            1.0
        };
        let p_frac = if peri_p_demand > f64::EPSILON {
            peri_p_removed / peri_p_demand
        } else {
            1.0
        };
        n_frac.min(p_frac)
    } else {
        0.0
    };
    let peri_cap = periphyton_gross_growth_g * peri_cap_frac;
    let peri_n_used = peri_n_demand * peri_cap_frac;
    let peri_p_used = peri_p_demand * peri_cap_frac;
    let peri_n_refund = (peri_n_removed - peri_n_used).max(0.0);
    let peri_p_refund = (peri_p_removed - peri_p_used).max(0.0);
    let peri_n_frac = if peri_n_removed > f64::EPSILON {
        peri_n_refund / peri_n_removed
    } else {
        0.0
    };
    refund_algae_nutrients(
        state,
        peri_nh3_removed * peri_n_frac,
        peri_no3_removed * peri_n_frac,
        peri_p_refund,
    );
    let periphyton_new_g = (state.algae.periphyton_biomass_g + peri_cap
        - periphyton_respiration_g
        - periphyton_grazing_g)
        .max(0.0)
        .min(periphyton_capacity_g.max(0.0));
    state.algae.periphyton_biomass_g = periphyton_new_g;

    let suspended_pressure = (state.algae.suspended_biomass_g / volume_l)
        / state
            .process_params
            .algae_bloom_threshold_g_per_l
            .max(f64::MIN_POSITIVE);
    let periphyton_pressure = if periphyton_capacity_g <= f64::EPSILON {
        0.0
    } else {
        state.algae.periphyton_biomass_g / periphyton_capacity_g
    };
    state.algae.nuisance_index =
        (0.55 * suspended_pressure + 0.45 * periphyton_pressure).clamp(0.0, 1.0);

    events::emit_daily_algae_events(state, previous_nuisance_index, periphyton_capacity_g);
}

fn algae_light_factor(state: &TankState) -> f64 {
    if !state.hardware.light.enabled {
        return 0.0;
    }

    let photoperiod_factor = (state.hardware.light.photoperiod_hours / 9.0).clamp(0.0, 1.0);
    half_saturation(
        state.hardware.light.intensity_index * photoperiod_factor,
        state.process_params.algae_light_half_saturation,
    )
}

fn refund_algae_nutrients(
    state: &mut TankState,
    ammonia_refund_mg: f64,
    nitrate_refund_mg: f64,
    p_refund_mg: f64,
) {
    if ammonia_refund_mg > f64::EPSILON {
        state.water.ammonia_total_mg_n_total += ammonia_refund_mg;
    }
    if nitrate_refund_mg > f64::EPSILON {
        state.water.nitrate_mg_n_total += nitrate_refund_mg;
    }
    if p_refund_mg > f64::EPSILON {
        state.water.phosphate_mg_p_total += p_refund_mg;
    }
}

/// Returns (ammonia_removed, nitrate_removed, phosphate_removed) so
/// refunds can go back to the correct nitrogen pool.
fn consume_algae_nutrients(
    state: &mut TankState,
    n_demand_mg: f64,
    p_demand_mg: f64,
) -> (f64, f64, f64) {
    let ammonia_removed = state.water.ammonia_total_mg_n_total.min(n_demand_mg * 0.6);
    state.water.ammonia_total_mg_n_total -= ammonia_removed;

    let remaining_n = (n_demand_mg - ammonia_removed).max(0.0);
    let nitrate_removed = state.water.nitrate_mg_n_total.min(remaining_n);
    state.water.nitrate_mg_n_total -= nitrate_removed;

    let phosphate_removed = state.water.phosphate_mg_p_total.min(p_demand_mg);
    state.water.phosphate_mg_p_total -= phosphate_removed;

    (ammonia_removed, nitrate_removed, phosphate_removed)
}

fn half_saturation(value: f64, half_sat: f64) -> f64 {
    let half_sat = half_sat.max(f64::MIN_POSITIVE);
    (value / (value + half_sat)).clamp(0.0, 1.0)
}

fn gaussian_response(value: f64, optimum: f64, sigma: f64) -> f64 {
    let sigma = sigma.max(0.1);
    let diff = value - optimum;
    (-(diff * diff) / (2.0 * sigma * sigma))
        .exp()
        .clamp(0.0, 1.0)
}
