use crate::types::{PlantGuild, TankState};

const PLANT_N_MG_PER_G_GROWTH: f64 = 28.0;
const PLANT_P_MG_PER_G_GROWTH: f64 = 4.0;

pub fn step_daily_plants(state: &mut TankState) {
    let surface_area_m2 = (state.geometry.surface_area_cm2() / 10_000.0).max(f64::MIN_POSITIVE);
    let total_plant_biomass_g: f64 = state.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
    let crowding_index = (total_plant_biomass_g
        / (surface_area_m2
            * state
                .process_params
                .plant_crowding_biomass_g_per_m2
                .max(1.0)))
    .clamp(0.0, 1.0);
    let f_light = plant_light_factor(state);
    let f_temp = gaussian_response(
        state.water.temperature_c,
        state.process_params.plant_temp_optimum_c,
        state.process_params.plant_temp_sigma_c,
    );
    let f_c = half_saturation(
        state.water.dissolved_inorganic_carbon_mg_c_total,
        state.process_params.plant_half_saturation_c_mg_total,
    );

    for index in 0..state.plant_guilds.len() {
        let guild = state.plant_guilds[index].guild;
        let biomass_g = state.plant_guilds[index].biomass_g;
        let health_index = state.plant_guilds[index].health_index;
        let habitat_index = habitat_factor(state, guild);

        state.plant_guilds[index].crowding_index = crowding_index;
        state.plant_guilds[index].habitat_index = habitat_index;

        let water_n = state.water.ammonia_total_mg_n_total + state.water.nitrate_mg_n_total;
        let water_p = state.water.phosphate_mg_p_total;
        let substrate_n = total_substrate_n(state);
        let substrate_p = total_substrate_p(state);
        let water_bias = state.plant_guilds[index].water_column_uptake_bias();
        let substrate_bias = state.plant_guilds[index].substrate_uptake_bias();
        let accessible_n = (water_n * water_bias) + (substrate_n * substrate_bias);
        let accessible_p = (water_p * water_bias) + (substrate_p * substrate_bias);
        let f_n = half_saturation(
            accessible_n,
            state.process_params.plant_half_saturation_n_mg_total,
        );
        let f_p = half_saturation(
            accessible_p,
            state.process_params.plant_half_saturation_p_mg_total,
        );
        let nutrient_limitation = f_n.min(f_p).min(f_c);
        let max_rate = match guild {
            PlantGuild::FastStem => state.process_params.plant_max_growth_rate_fast_stem_per_day,
            PlantGuild::RootFeedingRosette => {
                state
                    .process_params
                    .plant_max_growth_rate_root_rosette_per_day
            }
        };
        let gross_growth_g = biomass_g
            * max_rate
            * f_light
            * f_temp
            * nutrient_limitation
            * habitat_index
            * (1.0 - crowding_index);
        let respiration_g = biomass_g * state.process_params.plant_respiration_fraction_per_day;
        let senescence_g = biomass_g
            * state.process_params.plant_senescence_fraction_per_day
            * (1.0 + 0.5 * (1.0 - health_index));
        let realized_growth_g = gross_growth_g.max(0.0);

        let n_demand = realized_growth_g * PLANT_N_MG_PER_G_GROWTH;
        let p_demand = realized_growth_g * PLANT_P_MG_PER_G_GROWTH;
        let (n_removed, p_removed) =
            remove_plant_nutrients(state, n_demand, p_demand, water_bias, substrate_bias);
        // Cap growth by what nutrients were actually available, and refund
        // the non-limiting nutrient so mass balance is maintained.
        let cap_frac = if realized_growth_g > f64::EPSILON {
            let n_frac = if n_demand > f64::EPSILON {
                n_removed / n_demand
            } else {
                1.0
            };
            let p_frac = if p_demand > f64::EPSILON {
                p_removed / p_demand
            } else {
                1.0
            };
            n_frac.min(p_frac).min(1.0)
        } else {
            0.0
        };
        let nutrient_cap_g = realized_growth_g * cap_frac;

        // Refund the over-removed portion of the non-limiting nutrient.
        let n_used = n_removed * cap_frac;
        let p_used = p_removed * cap_frac;
        refund_plant_nutrients(state, n_removed - n_used, p_removed - p_used, water_bias);

        let capped_net = nutrient_cap_g - respiration_g - senescence_g;

        let new_biomass_g = (biomass_g + capped_net).max(0.0);
        state.plant_guilds[index].biomass_g = new_biomass_g;
        state.detritus.fine_detritus_g_total += senescence_g.max(0.0);

        let stress_driver = nutrient_limitation.min(f_light).min(habitat_index);
        let poor_conditions = stress_driver < 0.55;
        let new_health = if poor_conditions {
            health_index - state.process_params.plant_health_decline_per_day * (1.0 - stress_driver)
        } else {
            health_index + state.process_params.plant_health_recovery_per_day * (1.0 - health_index)
        };
        state.plant_guilds[index].health_index = new_health.clamp(0.0, 1.0);
    }
}

fn plant_light_factor(state: &TankState) -> f64 {
    if !state.hardware.light.enabled {
        return 0.0;
    }

    let photoperiod_factor = (state.hardware.light.photoperiod_hours / 10.0).clamp(0.0, 1.0);
    half_saturation(
        state.hardware.light.intensity_index * photoperiod_factor,
        state.process_params.plant_light_half_saturation,
    )
}

fn habitat_factor(state: &TankState, guild: PlantGuild) -> f64 {
    match guild {
        PlantGuild::FastStem => 0.95,
        PlantGuild::RootFeedingRosette => {
            if state.substrate_layers.is_empty() {
                return 0.2;
            }

            let total_depth_cm: f64 = state
                .substrate_layers
                .iter()
                .map(|layer| layer.depth_cm)
                .sum();
            let depth_factor = (total_depth_cm / 5.0).clamp(0.0, 1.0);
            let weighted_cec = state
                .substrate_layers
                .iter()
                .map(|layer| layer.cation_exchange_capacity_index * layer.depth_cm.max(0.1))
                .sum::<f64>()
                / total_depth_cm.max(0.1);
            let active_bonus = if state
                .substrate_layers
                .iter()
                .any(|layer| matches!(layer.kind, crate::types::SubstrateKind::ActivePlanted))
            {
                0.15
            } else {
                0.0
            };

            (0.2 + (0.35 * depth_factor) + (0.3 * weighted_cec) + active_bonus).clamp(0.0, 1.0)
        }
    }
}

/// Return excess nutrients to the water column when growth was capped by the
/// limiting nutrient. Refunds go to the water column (simplification) since
/// tracking per-layer substrate refunds adds complexity for minimal accuracy gain.
fn refund_plant_nutrients(
    state: &mut TankState,
    n_refund_mg: f64,
    p_refund_mg: f64,
    water_bias: f64,
) {
    if n_refund_mg > f64::EPSILON {
        // Refund N preferentially to ammonia (reverse of uptake order).
        let ammonia_share = n_refund_mg * water_bias;
        state.water.ammonia_total_mg_n_total += ammonia_share;
        // Remainder goes to nitrate via substrate proxy; simplify to water.
        state.water.nitrate_mg_n_total += (n_refund_mg - ammonia_share).max(0.0);
    }
    if p_refund_mg > f64::EPSILON {
        state.water.phosphate_mg_p_total += p_refund_mg;
    }
}

fn remove_plant_nutrients(
    state: &mut TankState,
    n_demand_mg: f64,
    p_demand_mg: f64,
    water_bias: f64,
    substrate_bias: f64,
) -> (f64, f64) {
    // Normalize biases so they sum to 1.0, preventing over-removal when
    // preset biases sum above 1.0 (e.g. fast_stem 0.9+0.2 = 1.1).
    let bias_sum = (water_bias + substrate_bias).max(f64::MIN_POSITIVE);
    let w = water_bias / bias_sum;
    let s = substrate_bias / bias_sum;

    let water_n_target = n_demand_mg * w;
    let substrate_n_target = n_demand_mg * s;
    let water_p_target = p_demand_mg * w;
    let substrate_p_target = p_demand_mg * s;

    let water_n_removed = remove_water_n(state, water_n_target);
    let substrate_n_removed = remove_substrate_n(
        state,
        substrate_n_target + (water_n_target - water_n_removed).max(0.0),
    );

    let water_p_removed = remove_water_p(state, water_p_target);
    let substrate_p_removed = remove_substrate_p(
        state,
        substrate_p_target + (water_p_target - water_p_removed).max(0.0),
    );

    (
        water_n_removed + substrate_n_removed,
        water_p_removed + substrate_p_removed,
    )
}

fn remove_water_n(state: &mut TankState, target_mg: f64) -> f64 {
    let ammonia_removed = state.water.ammonia_total_mg_n_total.min(target_mg);
    state.water.ammonia_total_mg_n_total -= ammonia_removed;

    let remaining = (target_mg - ammonia_removed).max(0.0);
    let nitrate_removed = state.water.nitrate_mg_n_total.min(remaining);
    state.water.nitrate_mg_n_total -= nitrate_removed;

    ammonia_removed + nitrate_removed
}

fn remove_water_p(state: &mut TankState, target_mg: f64) -> f64 {
    let removed = state.water.phosphate_mg_p_total.min(target_mg);
    state.water.phosphate_mg_p_total -= removed;
    removed
}

fn remove_substrate_n(state: &mut TankState, target_mg: f64) -> f64 {
    remove_substrate_pool(state, target_mg, true)
}

fn remove_substrate_p(state: &mut TankState, target_mg: f64) -> f64 {
    remove_substrate_pool(state, target_mg, false)
}

fn remove_substrate_pool(state: &mut TankState, target_mg: f64, is_nitrogen: bool) -> f64 {
    if target_mg <= f64::EPSILON || state.substrate_layers.is_empty() {
        return 0.0;
    }

    let weights: Vec<f64> = state
        .substrate_layers
        .iter()
        .map(|layer| {
            let store = if is_nitrogen {
                layer.nutrient_store_mg_n_total
            } else {
                layer.nutrient_store_mg_p_total
            };
            store * (0.4 + (0.6 * layer.cation_exchange_capacity_index))
        })
        .collect();
    let total_weight: f64 = weights.iter().sum();
    if total_weight <= f64::EPSILON {
        return 0.0;
    }

    let mut removed_total = 0.0;
    for (layer, weight) in state.substrate_layers.iter_mut().zip(weights.iter()) {
        let share = target_mg * (*weight / total_weight);
        let pool = if is_nitrogen {
            &mut layer.nutrient_store_mg_n_total
        } else {
            &mut layer.nutrient_store_mg_p_total
        };
        let removed = (*pool).min(share);
        *pool -= removed;
        removed_total += removed;
    }

    let mut remainder = (target_mg - removed_total).max(0.0);
    if remainder > f64::EPSILON {
        for layer in &mut state.substrate_layers {
            if remainder <= f64::EPSILON {
                break;
            }
            let pool = if is_nitrogen {
                &mut layer.nutrient_store_mg_n_total
            } else {
                &mut layer.nutrient_store_mg_p_total
            };
            let removed = (*pool).min(remainder);
            *pool -= removed;
            remainder -= removed;
            removed_total += removed;
        }
    }

    removed_total
}

fn total_substrate_n(state: &TankState) -> f64 {
    state
        .substrate_layers
        .iter()
        .map(|layer| layer.nutrient_store_mg_n_total)
        .sum()
}

fn total_substrate_p(state: &TankState) -> f64 {
    state
        .substrate_layers
        .iter()
        .map(|layer| layer.nutrient_store_mg_p_total)
        .sum()
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
