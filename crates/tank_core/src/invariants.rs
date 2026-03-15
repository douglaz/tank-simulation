use crate::types::{SimError, TankState};

pub fn enforce_invariants(state: &mut TankState) -> Result<(), SimError> {
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
