use crate::systems::chemistry::{CARBONATE_PH_MAX, CARBONATE_PH_MIN};
use crate::types::{SimError, TankState};

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
    state.animal.clamp_berried_to_adults();
    state.animal.egg_progress_days = state.animal.egg_progress_days.max(0.0);
    state.animal.egg_cohorts.retain(|c| c.count > 0);
    state.stability_tracker.instability_index =
        state.stability_tracker.instability_index.clamp(0.0, 1.0);

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
    for plant in &state.plant_guilds {
        check_non_negative("plant.biomass_g", plant.biomass_g)?;
    }
    check_non_negative("algae.suspended_biomass_g", state.algae.suspended_biomass_g)?;
    check_non_negative(
        "algae.periphyton_biomass_g",
        state.algae.periphyton_biomass_g,
    )?;
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
        "animal.sub_adult.reserve_g",
        state.animal.sub_adult.reserve_g,
    )?;
    check_non_negative("animal.juvenile.reserve_g", state.animal.juvenile.reserve_g)?;

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
