use crate::systems::chemistry::resolve_carbonate_state;
use crate::types::{
    legacy_total_param_to_mg_per_l, legacy_total_param_to_mg_per_m2, plant_carbon_mg,
    plant_nitrogen_mg, PlantGuild, TankState, PLANT_N_MG_PER_G_BIOMASS,
};

const PLANT_P_MG_PER_G_GROWTH: f64 = 4.0;

pub fn step_daily_plants(state: &mut TankState) {
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let dic_mg_c_per_l = state.concentrations().dic_mg_c_per_l();
    let plant_half_saturation_n_mg_n_per_l =
        legacy_total_param_to_mg_per_l(state.process_params.plant_half_saturation_n_mg_total);
    let plant_half_saturation_p_mg_p_per_l =
        legacy_total_param_to_mg_per_l(state.process_params.plant_half_saturation_p_mg_total);
    let plant_half_saturation_c_mg_c_per_l =
        legacy_total_param_to_mg_per_l(state.process_params.plant_half_saturation_c_mg_total);
    let plant_half_saturation_n_mg_n_per_m2 =
        legacy_total_param_to_mg_per_m2(state.process_params.plant_half_saturation_n_mg_total);
    let plant_half_saturation_p_mg_p_per_m2 =
        legacy_total_param_to_mg_per_m2(state.process_params.plant_half_saturation_p_mg_total);
    let surface_area_m2 = state.geometry.footprint_area_m2().max(f64::MIN_POSITIVE);
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
    let f_c = half_saturation(dic_mg_c_per_l, plant_half_saturation_c_mg_c_per_l);

    for index in 0..state.plant_guilds.len() {
        let guild = state.plant_guilds[index].guild;
        let biomass_g = state.plant_guilds[index].biomass_g;
        let health_index = state.plant_guilds[index].health_index;
        let habitat_index = habitat_factor(state, guild);

        state.plant_guilds[index].crowding_index = crowding_index;
        state.plant_guilds[index].habitat_index = habitat_index;

        let substrate_n = state.substrate_n_mg_n_per_m2();
        let substrate_p = state.substrate_p_mg_p_per_m2();
        let water_bias = state.plant_guilds[index].water_column_uptake_bias();
        let substrate_bias = state.plant_guilds[index].substrate_uptake_bias();
        let (tan_mg_n_per_l, nitrate_mg_n_per_l, phosphate_mg_p_per_l) = {
            let chemistry = state.concentrations();
            (
                chemistry.tan_mg_n_per_l(),
                chemistry.nitrate_mg_n_per_l(),
                chemistry.phosphate_mg_p_per_l(),
            )
        };
        // Normalize biases for the accessibility calculation so presets that
        // sum above 1.0 don't inflate the apparent nutrient availability.
        let bias_sum = (water_bias + substrate_bias).max(f64::MIN_POSITIVE);
        let w_norm = water_bias / bias_sum;
        let s_norm = substrate_bias / bias_sum;
        let water_n_factor = half_saturation(
            tan_mg_n_per_l + nitrate_mg_n_per_l,
            plant_half_saturation_n_mg_n_per_l,
        );
        let water_p_factor =
            half_saturation(phosphate_mg_p_per_l, plant_half_saturation_p_mg_p_per_l);
        let substrate_n_factor = half_saturation(substrate_n, plant_half_saturation_n_mg_n_per_m2);
        let substrate_p_factor = half_saturation(substrate_p, plant_half_saturation_p_mg_p_per_m2);
        let f_n = weighted_limitation_factor(water_n_factor, substrate_n_factor, w_norm, s_norm);
        let f_p = weighted_limitation_factor(water_p_factor, substrate_p_factor, w_norm, s_norm);
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

        let n_demand = realized_growth_g * PLANT_N_MG_PER_G_BIOMASS;
        let p_demand = realized_growth_g * PLANT_P_MG_PER_G_GROWTH;
        let c_demand = plant_carbon_mg(realized_growth_g, n_to_c_ratio);
        let (nh3_removed, no3_removed, sub_n_removed, p_removed) =
            remove_plant_nutrients(state, n_demand, p_demand, water_bias, substrate_bias);
        let n_removed = nh3_removed + no3_removed + sub_n_removed;
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
            let c_frac = if c_demand > f64::EPSILON {
                state.water.dissolved_inorganic_carbon_mg_c_total / c_demand
            } else {
                1.0
            };
            n_frac.min(p_frac).min(c_frac).min(1.0)
        } else {
            0.0
        };
        let nutrient_cap_g = realized_growth_g * cap_frac;

        // Refund the over-removed non-limiting nutrient only.
        // n_used/p_used = what the capped growth actually consumed.
        let n_used = n_demand * cap_frac;
        let p_used = p_demand * cap_frac;
        let n_refund = (n_removed - n_used).max(0.0);
        let p_refund = (p_removed - p_used).max(0.0);
        // Split N refund proportionally across the pools it was removed from.
        let n_refund_frac = if n_removed > f64::EPSILON {
            n_refund / n_removed
        } else {
            0.0
        };
        refund_plant_nutrients(
            state,
            n_refund_frac,
            nh3_removed,
            no3_removed,
            sub_n_removed,
            p_refund,
        );
        let c_used = plant_carbon_mg(nutrient_cap_g, n_to_c_ratio);
        state.water.dissolved_inorganic_carbon_mg_c_total =
            (state.water.dissolved_inorganic_carbon_mg_c_total - c_used).max(0.0);

        let biomass_after_growth_g = (biomass_g + nutrient_cap_g).max(0.0);
        let (realized_respiration_g, realized_senescence_g) =
            clamp_partitioned_losses(biomass_after_growth_g, respiration_g, senescence_g);
        let new_biomass_g =
            (biomass_after_growth_g - realized_respiration_g - realized_senescence_g).max(0.0);
        state.plant_guilds[index].biomass_g = new_biomass_g;
        route_plant_loss_to_dissolved_organics(state, realized_respiration_g, n_to_c_ratio);
        state.detritus.fine_detritus_g_total +=
            plant_detrital_mass_g(realized_senescence_g, n_to_c_ratio);

        let stress_driver = nutrient_limitation.min(f_light).min(habitat_index);
        let poor_conditions = stress_driver < 0.55;
        let new_health = if poor_conditions {
            health_index - state.process_params.plant_health_decline_per_day * (1.0 - stress_driver)
        } else {
            health_index + state.process_params.plant_health_recovery_per_day * (1.0 - health_index)
        };
        state.plant_guilds[index].health_index = new_health.clamp(0.0, 1.0);
    }

    let volume_l = state.water_volume_l();
    resolve_carbonate_state(&mut state.water, volume_l);
}

fn weighted_limitation_factor(
    water_factor: f64,
    substrate_factor: f64,
    water_weight: f64,
    substrate_weight: f64,
) -> f64 {
    ((water_factor * water_weight) + (substrate_factor * substrate_weight)).clamp(0.0, 1.0)
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

/// Return excess nutrients proportionally to the pools they were removed from.
/// Substrate N refunds go to water-column nitrate (simplification) since
/// tracking per-layer substrate refunds adds complexity for minimal accuracy gain.
fn refund_plant_nutrients(
    state: &mut TankState,
    refund_frac: f64,
    ammonia_removed: f64,
    nitrate_removed: f64,
    substrate_n_removed: f64,
    p_refund_mg: f64,
) {
    if refund_frac > f64::EPSILON {
        let nh3_refund = ammonia_removed * refund_frac;
        let no3_refund = nitrate_removed * refund_frac;
        let sub_n_refund = substrate_n_removed * refund_frac;
        if nh3_refund > f64::EPSILON {
            state.water.ammonia_total_mg_n_total += nh3_refund;
        }
        // Nitrate + substrate N both refunded to water-column nitrate.
        let nitrate_total_refund = no3_refund + sub_n_refund;
        if nitrate_total_refund > f64::EPSILON {
            state.water.nitrate_mg_n_total += nitrate_total_refund;
        }
    }
    if p_refund_mg > f64::EPSILON {
        state.water.phosphate_mg_p_total += p_refund_mg;
    }
}

/// Returns (ammonia_removed, nitrate_removed, substrate_n_removed, total_p_removed)
/// so refunds can go back to the correct pools.
fn remove_plant_nutrients(
    state: &mut TankState,
    n_demand_mg: f64,
    p_demand_mg: f64,
    water_bias: f64,
    substrate_bias: f64,
) -> (f64, f64, f64, f64) {
    // Normalize biases so they sum to 1.0, preventing over-removal when
    // preset biases sum above 1.0 (e.g. fast_stem 0.9+0.2 = 1.1).
    let bias_sum = (water_bias + substrate_bias).max(f64::MIN_POSITIVE);
    let w = water_bias / bias_sum;
    let s = substrate_bias / bias_sum;

    let water_n_target = n_demand_mg * w;
    let substrate_n_target = n_demand_mg * s;
    let water_p_target = p_demand_mg * w;
    let substrate_p_target = p_demand_mg * s;

    let (ammonia_removed, nitrate_removed) = remove_water_n(state, water_n_target);
    let water_n_removed = ammonia_removed + nitrate_removed;
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
        ammonia_removed,
        nitrate_removed,
        substrate_n_removed,
        water_p_removed + substrate_p_removed,
    )
}

/// Returns (ammonia_removed, nitrate_removed) so callers can track pool sources.
fn remove_water_n(state: &mut TankState, target_mg: f64) -> (f64, f64) {
    let ammonia_removed = state.water.ammonia_total_mg_n_total.min(target_mg);
    state.water.ammonia_total_mg_n_total -= ammonia_removed;

    let remaining = (target_mg - ammonia_removed).max(0.0);
    let nitrate_removed = state.water.nitrate_mg_n_total.min(remaining);
    state.water.nitrate_mg_n_total -= nitrate_removed;

    (ammonia_removed, nitrate_removed)
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

fn route_plant_loss_to_dissolved_organics(
    state: &mut TankState,
    biomass_g: f64,
    n_to_c_ratio: f64,
) {
    if biomass_g <= f64::EPSILON {
        return;
    }

    state.water.dissolved_organic_nitrogen_mg_n_total += plant_nitrogen_mg(biomass_g);
    state.water.dissolved_organic_carbon_mg_c_total += plant_carbon_mg(biomass_g, n_to_c_ratio);
}

fn clamp_partitioned_losses(
    available_biomass_g: f64,
    primary_loss_g: f64,
    secondary_loss_g: f64,
) -> (f64, f64) {
    let available_biomass_g = available_biomass_g.max(0.0);
    let primary_loss_g = primary_loss_g.max(0.0);
    let secondary_loss_g = secondary_loss_g.max(0.0);
    let requested_total_loss_g = primary_loss_g + secondary_loss_g;
    if available_biomass_g <= f64::EPSILON || requested_total_loss_g <= f64::EPSILON {
        return (0.0, 0.0);
    }

    let realized_total_loss_g = requested_total_loss_g.min(available_biomass_g);
    let realized_primary_loss_g = realized_total_loss_g * (primary_loss_g / requested_total_loss_g);
    let realized_secondary_loss_g = realized_total_loss_g - realized_primary_loss_g;
    (realized_primary_loss_g, realized_secondary_loss_g)
}

pub(crate) fn plant_detrital_mass_g(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    if biomass_g <= f64::EPSILON {
        return 0.0;
    }

    (plant_nitrogen_mg(biomass_g) + plant_carbon_mg(biomass_g, n_to_c_ratio)) / 1000.0
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
