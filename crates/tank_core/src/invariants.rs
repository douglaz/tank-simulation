use crate::types::{SimError, TankState};

pub fn enforce_invariants(state: &mut TankState) -> Result<(), SimError> {
    if state.water.dissolved_oxygen_mg_total.is_finite() {
        state.water.dissolved_oxygen_mg_total = state.water.dissolved_oxygen_mg_total.max(0.0);
    }
    check_non_negative(
        "ammonia_total_mg_n_total",
        state.water.ammonia_total_mg_n_total,
    )?;
    check_non_negative("nitrite_mg_n_total", state.water.nitrite_mg_n_total)?;
    check_non_negative("nitrate_mg_n_total", state.water.nitrate_mg_n_total)?;
    check_non_negative("phosphate_mg_p_total", state.water.phosphate_mg_p_total)?;
    check_non_negative(
        "dissolved_oxygen_mg_total",
        state.water.dissolved_oxygen_mg_total,
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
    state.water.ph = state.water.ph.clamp(5.5, 8.5);

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
    state.animal.condition_index = state.animal.condition_index.clamp(0.0, 1.0);
    state.animal.molt_stress_index = state.animal.molt_stress_index.clamp(0.0, 1.0);
    state.animal.reproductive_readiness_index =
        state.animal.reproductive_readiness_index.clamp(0.0, 1.0);
    state.animal.daily_food_consumed_g = state.animal.daily_food_consumed_g.max(0.0);
    // Shrimp population invariants: berried <= adults, no negative egg progress
    state.animal.clamp_berried_to_adults();
    state.animal.egg_progress_days = state.animal.egg_progress_days.max(0.0);
    // Remove any empty cohorts
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

    if state.event_log.len() > 200 {
        let keep_from = state.event_log.len() - 200;
        state.event_log.drain(0..keep_from);
    }

    Ok(())
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
