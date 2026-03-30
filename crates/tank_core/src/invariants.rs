use crate::systems::chemistry::{CARBONATE_PH_MAX, CARBONATE_PH_MIN};
use crate::types::{ShrimpRuntimeParams, SimError, TankState};

const SHRIMP_ROUTE_SUM_TOLERANCE: f64 = 1e-9;

pub fn validate_invariants(state: &TankState) -> Result<(), SimError> {
    validate_invariants_inner(state)
}

pub fn enforce_invariants(state: &mut TankState) -> Result<(), SimError> {
    validate_invariants_inner(state)?;

    state.water.dissolved_oxygen_mg_total =
        normalized_non_negative_if_finite(state.water.dissolved_oxygen_mg_total);
    state.water.ph = state.water.ph.clamp(CARBONATE_PH_MIN, CARBONATE_PH_MAX);
    state.geometry.lid_exchange_factor = state.geometry.lid_exchange_factor.clamp(0.0, 1.0);
    state.hardware.light.intensity_index = state.hardware.light.intensity_index.clamp(0.0, 1.0);
    state.hardware.filter.cleanliness_index =
        state.hardware.filter.cleanliness_index.clamp(0.0, 1.0);
    state.hardware.aeration.intensity = state.hardware.aeration.intensity.clamp(0.0, 1.0);
    state.filter_state.biofilter_maturity_index =
        state.filter_state.biofilter_maturity_index.clamp(0.0, 1.0);
    state.filter_state.clogging_index = state.filter_state.clogging_index.clamp(0.0, 1.0);
    state.filter_state.seeded_biomass_index =
        state.filter_state.seeded_biomass_index.clamp(0.0, 1.0);
    state.algae.nuisance_index = state.algae.nuisance_index.clamp(0.0, 1.0);
    state.microbe.maturity_index = state.microbe.maturity_index.clamp(0.0, 1.0);
    state.microfauna.population_index = state.microfauna.population_index.clamp(0.0, 1.0);
    state.microfauna.grazing_pressure_index =
        state.microfauna.grazing_pressure_index.clamp(0.0, 1.0);
    state.animal.adult.condition_index = state.animal.adult.condition_index.clamp(0.0, 1.0);
    state.animal.sub_adult.condition_index = state.animal.sub_adult.condition_index.clamp(0.0, 1.0);
    state.animal.juvenile.condition_index = state.animal.juvenile.condition_index.clamp(0.0, 1.0);
    state.animal.molt_stress_index = state.animal.molt_stress_index.clamp(0.0, 1.0);
    state.animal.reproductive_readiness_index =
        state.animal.reproductive_readiness_index.clamp(0.0, 1.0);
    state.animal.molt_readiness = state.animal.molt_readiness.clamp(0.0, 1.0);
    state.animal.failed_molt_accum = state.animal.failed_molt_accum.clamp(0.0, 1.0);
    state.animal.daily_food_consumed_g = state.animal.daily_food_consumed_g.max(0.0);
    state.animal.spawn_progress_accum = state.animal.spawn_progress_accum.clamp(-1.0, 1.0);
    state.animal.hatch_success_carry = state.animal.hatch_success_carry.clamp(-1.0, 1.0);
    state.animal.clamp_berried_to_adults();
    state.animal.egg_progress_days = state.animal.egg_progress_days.max(0.0);
    state.animal.egg_cohorts.retain(|c| c.count > 0);
    state.animal.adult.clamp_maturation_accum_to_count();
    state.animal.sub_adult.clamp_maturation_accum_to_count();
    state.animal.juvenile.clamp_maturation_accum_to_count();
    state.stability_tracker.instability_index =
        state.stability_tracker.instability_index.clamp(0.0, 1.0);
    state.microbe.denitrifier_activity_index =
        state.microbe.denitrifier_activity_index.clamp(0.0, 1.0);

    for plant in &mut state.plant_guilds {
        plant.health_index = plant.health_index.clamp(0.0, 1.0);
        plant.crowding_index = plant.crowding_index.clamp(0.0, 1.0);
        plant.habitat_index = plant.habitat_index.clamp(0.0, 1.0);
    }
    for layer in &mut state.substrate_layers {
        layer.cation_exchange_capacity_index = layer.cation_exchange_capacity_index.clamp(0.0, 1.0);
        layer.detritus_trapping_index = layer.detritus_trapping_index.clamp(0.0, 1.0);
        layer.low_oxygen_tendency_index = layer.low_oxygen_tendency_index.clamp(0.0, 1.0);
        layer.grazing_surface_index = layer.grazing_surface_index.clamp(0.0, 1.0);
    }
    for habitat in &mut state.habitat_registry {
        habitat.colonizable_area_cm2 = habitat.colonizable_area_cm2.max(0.0);
        habitat.flow_exposure = habitat.flow_exposure.clamp(0.0, 1.0);
        habitat.oxygen_exposure = habitat.oxygen_exposure.clamp(0.0, 1.0);
        habitat.light_exposure = habitat.light_exposure.clamp(0.0, 1.0);
    }

    if state.event_log.len() > 200 {
        let keep_from = state.event_log.len() - 200;
        state.event_log.drain(0..keep_from);
    }

    Ok(())
}

fn validate_invariants_inner(state: &TankState) -> Result<(), SimError> {
    check_non_negative(
        "ammonia_total_mg_n_total",
        state.water.ammonia_total_mg_n_total,
    )?;
    check_non_negative("nitrite_mg_n_total", state.water.nitrite_mg_n_total)?;
    check_non_negative("nitrate_mg_n_total", state.water.nitrate_mg_n_total)?;
    check_non_negative("phosphate_mg_p_total", state.water.phosphate_mg_p_total)?;
    check_non_negative(
        "dissolved_oxygen_mg_total",
        normalized_non_negative_if_finite(state.water.dissolved_oxygen_mg_total),
    )?;
    check_non_negative(
        "dissolved_inorganic_carbon_mg_c_total",
        state.water.dissolved_inorganic_carbon_mg_c_total,
    )?;
    check_non_negative(
        "dissolved_organic_carbon_mg_c_total",
        state.water.dissolved_organic_carbon_mg_c_total,
    )?;
    check_non_negative(
        "dissolved_organic_nitrogen_mg_n_total",
        state.water.dissolved_organic_nitrogen_mg_n_total,
    )?;
    check_non_negative("alkalinity_meq_total", state.water.alkalinity_meq_total)?;
    check_non_negative("calcium_mg_total", state.water.calcium_mg_total)?;
    check_non_negative("magnesium_mg_total", state.water.magnesium_mg_total)?;
    check_non_negative("sodium_mg_total", state.water.sodium_mg_total)?;
    check_non_negative("potassium_mg_total", state.water.potassium_mg_total)?;
    check_non_negative("bicarbonate_mg_total", state.water.bicarbonate_mg_total)?;
    check_non_negative("chloride_mg_total", state.water.chloride_mg_total)?;
    check_non_negative("sulfate_mg_total", state.water.sulfate_mg_total)?;
    check_non_negative(
        "particulate_organics_g_total",
        state.detritus.particulate_organics_g_total,
    )?;
    check_non_negative(
        "fine_detritus_g_total",
        state.detritus.fine_detritus_g_total,
    )?;
    check_non_negative(
        "dissolved_feed_residue_g_total",
        state.detritus.dissolved_feed_residue_g_total,
    )?;
    check_non_negative("decomposer_biomass_g", state.microbe.decomposer_biomass_g)?;
    check_non_negative(
        "ammonia_oxidizer_biomass_g",
        state.microbe.ammonia_oxidizer_biomass_g,
    )?;
    check_non_negative(
        "nitrite_oxidizer_biomass_g",
        state.microbe.nitrite_oxidizer_biomass_g,
    )?;
    check_non_negative("comammox_biomass_g", state.microbe.comammox_biomass_g)?;
    check_non_negative("cumulative_n2_export_mg_n", state.cumulative_n2_export_mg_n)?;
    for plant in &state.plant_guilds {
        check_non_negative("plant.biomass_g", plant.biomass_g)?;
    }
    check_non_negative("algae.suspended_biomass_g", state.algae.suspended_biomass_g)?;
    check_non_negative(
        "algae.periphyton_biomass_g",
        state.algae.periphyton_biomass_g,
    )?;
    for biomass in state.algae.periphyton_by_habitat.values() {
        check_non_negative("algae.periphyton_by_habitat[*]", *biomass)?;
    }
    for biomass in state.microbe.decomposer_by_habitat.values() {
        check_non_negative("microbe.decomposer_by_habitat[*]", *biomass)?;
    }
    for layer in &state.substrate_layers {
        check_non_negative("substrate.depth_cm", layer.depth_cm)?;
        check_non_negative(
            "substrate.nutrient_store_mg_n_total",
            layer.nutrient_store_mg_n_total,
        )?;
        check_non_negative(
            "substrate.nutrient_store_mg_p_total",
            layer.nutrient_store_mg_p_total,
        )?;
        check_non_negative(
            "substrate.colonizable_area_factor",
            layer.colonizable_area_factor,
        )?;
        check_non_negative("substrate.colonizable_area_cm2", layer.colonizable_area_cm2)?;
    }
    check_non_negative("water.temperature_c", state.water.temperature_c)?;
    if state.water.temperature_c <= 0.0 {
        return Err(SimError::TemperatureTooLow {
            field: "water.temperature_c",
            value: state.water.temperature_c,
        });
    }
    if !state.water.ph.is_finite() {
        return Err(SimError::InvariantViolation {
            field: "water.ph",
            value: state.water.ph,
        });
    }

    // Geometry: reject non-positive dimensions and impossible fill levels
    check_positive("geometry.length_cm", state.geometry.length_cm)?;
    check_positive("geometry.width_cm", state.geometry.width_cm)?;
    check_positive("geometry.height_cm", state.geometry.height_cm)?;
    check_positive("geometry.fill_height_cm", state.geometry.fill_height_cm)?;
    if state.geometry.fill_height_cm > state.geometry.height_cm {
        return Err(SimError::InvariantViolation {
            field: "geometry.fill_height_cm",
            value: state.geometry.fill_height_cm,
        });
    }
    check_non_negative("animal.adult.reserve_g", state.animal.adult.reserve_g)?;
    check_non_negative(
        "animal.adult.molt_timer_days",
        state.animal.adult.molt_timer_days,
    )?;
    check_non_negative(
        "animal.sub_adult.reserve_g",
        state.animal.sub_adult.reserve_g,
    )?;
    check_non_negative(
        "animal.sub_adult.molt_timer_days",
        state.animal.sub_adult.molt_timer_days,
    )?;
    check_non_negative("animal.juvenile.reserve_g", state.animal.juvenile.reserve_g)?;
    check_non_negative(
        "animal.juvenile.molt_timer_days",
        state.animal.juvenile.molt_timer_days,
    )?;
    if state.animal.berried_females_count > state.animal.adult.count {
        return Err(SimError::InvariantViolation {
            field: "animal.berried_females_count",
            value: f64::from(state.animal.berried_females_count),
        });
    }
    let egg_cohort_total = state.animal.egg_cohort_count_total();
    if egg_cohort_total != state.animal.berried_females_count {
        return Err(SimError::InvariantViolation {
            field: "animal.egg_cohort_count_total",
            value: f64::from(egg_cohort_total),
        });
    }

    // Process parameters: reject negative coefficients that would produce
    // nonsensical physics (negative heat transfer, negative reaeration, etc.).
    let pp = &state.process_params;
    check_non_negative("process.reaeration_kla_base", pp.reaeration_kla_base)?;
    check_non_negative("process.aeration_kla_boost", pp.aeration_kla_boost)?;
    check_non_negative("process.k_surface_w_per_m2_k", pp.k_surface_w_per_m2_k)?;
    check_non_negative("process.k_wall_w_per_m2_k", pp.k_wall_w_per_m2_k)?;
    check_non_negative(
        "process.background_bod_mg_o2_per_g_biomass_per_hour",
        pp.background_bod_mg_o2_per_g_biomass_per_hour,
    )?;
    check_non_negative(
        "process.decomposer_vmax_per_hour",
        pp.decomposer_vmax_per_hour,
    )?;
    check_non_negative(
        "process.aob_vmax_mg_n_per_g_per_hour",
        pp.aob_vmax_mg_n_per_g_per_hour,
    )?;
    check_non_negative(
        "process.nob_vmax_mg_n_per_g_per_hour",
        pp.nob_vmax_mg_n_per_g_per_hour,
    )?;
    check_non_negative(
        "process.respiration_dic_rate_mg_c_per_g_per_hour",
        pp.respiration_dic_rate_mg_c_per_g_per_hour,
    )?;
    check_non_negative(
        "process.photosynthesis_dic_rate_mg_c_per_g_per_hour",
        pp.photosynthesis_dic_rate_mg_c_per_g_per_hour,
    )?;
    check_non_negative(
        "process.plant_photosynthesis_o2_mg_per_g_per_hour",
        pp.plant_photosynthesis_o2_mg_per_g_per_hour,
    )?;
    check_non_negative(
        "process.denitrification_vmax_mg_n_per_l_per_hour",
        pp.denitrification_vmax_mg_n_per_l_per_hour,
    )?;
    check_non_negative(
        "process.denitrification_k_no3_mg_n_per_l",
        pp.denitrification_k_no3_mg_n_per_l,
    )?;
    check_non_negative(
        "process.denitrification_k_doc_mg_c_per_l",
        pp.denitrification_k_doc_mg_c_per_l,
    )?;
    check_non_negative(
        "process.denitrification_pore_water_mixing_factor",
        pp.denitrification_pore_water_mixing_factor,
    )?;
    check_non_negative(
        "process.denitrification_activity_maturation_days",
        pp.denitrification_activity_maturation_days,
    )?;
    check_non_negative("process.rol_rate_cm_per_g", pp.rol_rate_cm_per_g)?;
    check_open_unit_interval(
        "process.shrimp_assimilation_efficiency",
        pp.shrimp_assimilation_efficiency,
    )?;
    check_unit_interval(
        "process.shrimp_respiration_fraction_of_assimilated",
        pp.shrimp_respiration_fraction_of_assimilated,
    )?;
    check_unit_interval(
        "process.shrimp_excretion_fraction_of_assimilated",
        pp.shrimp_excretion_fraction_of_assimilated,
    )?;
    check_unit_interval(
        "process.shrimp_growth_fraction_of_assimilated",
        pp.shrimp_growth_fraction_of_assimilated,
    )?;
    check_unit_interval(
        "process.death_biomass_to_detritus_fraction",
        pp.death_biomass_to_detritus_fraction,
    )?;
    // Mortality routing stays closed-loop until the engine has an explicit
    // export destination for carcass removal.
    check_sum_close_to_one(
        "process.death_biomass_to_detritus_fraction",
        pp.death_biomass_to_detritus_fraction,
    )?;
    check_positive(
        "process.shrimp_o2_per_mg_c_respired",
        pp.shrimp_o2_per_mg_c_respired,
    )?;
    check_sum_close_to_one(
        "process.shrimp_assimilated_partition_sum",
        pp.shrimp_respiration_fraction_of_assimilated
            + pp.shrimp_excretion_fraction_of_assimilated
            + pp.shrimp_growth_fraction_of_assimilated,
    )?;
    validate_shrimp_runtime_params(&state.shrimp_params)?;

    Ok(())
}

fn validate_shrimp_runtime_params(params: &ShrimpRuntimeParams) -> Result<(), SimError> {
    check_positive(
        "shrimp_params.optimal_temp_min_c",
        params.optimal_temp_min_c,
    )?;
    check_positive(
        "shrimp_params.optimal_temp_max_c",
        params.optimal_temp_max_c,
    )?;
    check_strictly_increasing(
        "shrimp_params.optimal_temp_min_c",
        params.optimal_temp_min_c,
        "shrimp_params.optimal_temp_max_c",
        params.optimal_temp_max_c,
    )?;
    check_non_negative("shrimp_params.gh_min_d", params.gh_min_d)?;
    check_non_negative("shrimp_params.gh_max_d", params.gh_max_d)?;
    check_strictly_increasing(
        "shrimp_params.gh_min_d",
        params.gh_min_d,
        "shrimp_params.gh_max_d",
        params.gh_max_d,
    )?;
    check_unit_interval("shrimp_params.base_spawn_rate", params.base_spawn_rate)?;
    if params.egg_duration_days == 0 {
        return Err(SimError::InvariantViolation {
            field: "shrimp_params.egg_duration_days",
            value: 0.0,
        });
    }
    check_unit_interval(
        "shrimp_params.hatch_success_base",
        params.hatch_success_base,
    )?;
    check_non_negative(
        "shrimp_params.juvenile_sensitivity",
        params.juvenile_sensitivity,
    )?;
    check_positive(
        "shrimp_params.high_temp_repro_penalty_start_c",
        params.high_temp_repro_penalty_start_c,
    )?;
    check_positive(
        "shrimp_params.high_temp_repro_penalty_full_c",
        params.high_temp_repro_penalty_full_c,
    )?;
    check_strictly_increasing(
        "shrimp_params.high_temp_repro_penalty_start_c",
        params.high_temp_repro_penalty_start_c,
        "shrimp_params.high_temp_repro_penalty_full_c",
        params.high_temp_repro_penalty_full_c,
    )?;
    check_positive(
        "shrimp_params.body_nitrogen_mg_per_g_wet_mass",
        params.body_nitrogen_mg_per_g_wet_mass,
    )?;
    check_positive(
        "shrimp_params.body_carbon_mg_per_g_wet_mass",
        params.body_carbon_mg_per_g_wet_mass,
    )?;
    check_positive(
        "shrimp_params.juvenile_to_subadult_days",
        params.juvenile_to_subadult_days,
    )?;
    check_positive(
        "shrimp_params.subadult_to_adult_days",
        params.subadult_to_adult_days,
    )?;
    check_unit_interval(
        "shrimp_params.juvenile_maturation_condition_threshold",
        params.juvenile_maturation_condition_threshold,
    )?;
    check_unit_interval(
        "shrimp_params.subadult_maturation_condition_threshold",
        params.subadult_maturation_condition_threshold,
    )?;
    check_positive(
        "shrimp_params.base_molt_interval_days",
        params.base_molt_interval_days,
    )?;
    check_non_negative(
        "shrimp_params.failed_molt_mortality_scale",
        params.failed_molt_mortality_scale,
    )?;
    check_non_negative(
        "shrimp_params.sub_adult_sensitivity",
        params.sub_adult_sensitivity,
    )?;
    if params.base_clutch_size == 0 {
        return Err(SimError::InvariantViolation {
            field: "shrimp_params.base_clutch_size",
            value: 0.0,
        });
    }
    check_unit_interval(
        "shrimp_params.min_clutch_condition",
        params.min_clutch_condition,
    )?;
    check_positive("shrimp_params.ca_min_mg_per_l", params.ca_min_mg_per_l)?;
    check_positive("shrimp_params.mg_min_mg_per_l", params.mg_min_mg_per_l)?;
    check_positive(
        "shrimp_params.molt_reserve_fraction",
        params.molt_reserve_fraction,
    )?;
    check_unit_interval(
        "shrimp_params.molt_reserve_factor_floor",
        params.molt_reserve_factor_floor,
    )?;
    check_unit_interval(
        "shrimp_params.molt_condition_weight",
        params.molt_condition_weight,
    )?;
    check_unit_interval(
        "shrimp_params.molt_reserve_weight",
        params.molt_reserve_weight,
    )?;
    check_sum_close_to_one(
        "shrimp_params.molt_condition_weight + shrimp_params.molt_reserve_weight",
        params.molt_condition_weight + params.molt_reserve_weight,
    )?;
    check_positive(
        "shrimp_params.juvenile_molt_interval_days",
        params.juvenile_molt_interval_days,
    )?;
    check_positive(
        "shrimp_params.sub_adult_molt_interval_days",
        params.sub_adult_molt_interval_days,
    )?;
    check_unit_interval(
        "shrimp_params.molt_success_threshold",
        params.molt_success_threshold,
    )?;
    check_unit_interval(
        "shrimp_params.critical_molt_gh_ratio",
        params.critical_molt_gh_ratio,
    )?;
    check_non_negative(
        "shrimp_params.chloride_protection_factor",
        params.chloride_protection_factor,
    )?;
    check_strictly_increasing(
        "shrimp_params.juvenile_molt_interval_days",
        params.juvenile_molt_interval_days,
        "shrimp_params.sub_adult_molt_interval_days",
        params.sub_adult_molt_interval_days,
    )?;
    check_strictly_increasing(
        "shrimp_params.sub_adult_molt_interval_days",
        params.sub_adult_molt_interval_days,
        "shrimp_params.base_molt_interval_days",
        params.base_molt_interval_days,
    )?;

    Ok(())
}

fn normalized_non_negative_if_finite(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        value
    }
}

fn check_non_negative(field: &'static str, value: f64) -> Result<(), SimError> {
    if !value.is_finite() || value < 0.0 {
        Err(SimError::InvariantViolation { field, value })
    } else {
        Ok(())
    }
}

fn check_positive(field: &'static str, value: f64) -> Result<(), SimError> {
    if !value.is_finite() || value <= 0.0 {
        Err(SimError::InvariantViolation { field, value })
    } else {
        Ok(())
    }
}

fn check_unit_interval(field: &'static str, value: f64) -> Result<(), SimError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        Err(SimError::InvariantViolation { field, value })
    } else {
        Ok(())
    }
}

fn check_open_unit_interval(field: &'static str, value: f64) -> Result<(), SimError> {
    if !value.is_finite() || value <= 0.0 || value >= 1.0 {
        Err(SimError::InvariantViolation { field, value })
    } else {
        Ok(())
    }
}

fn check_sum_close_to_one(field: &'static str, value: f64) -> Result<(), SimError> {
    if !value.is_finite() || (value - 1.0).abs() > SHRIMP_ROUTE_SUM_TOLERANCE {
        Err(SimError::InvariantViolation { field, value })
    } else {
        Ok(())
    }
}

fn check_strictly_increasing(
    lower_field: &'static str,
    lower_value: f64,
    upper_field: &'static str,
    upper_value: f64,
) -> Result<(), SimError> {
    if lower_value >= upper_value {
        Err(SimError::OrderingViolation {
            lower_field,
            lower_value,
            upper_field,
            upper_value,
        })
    } else {
        Ok(())
    }
}
