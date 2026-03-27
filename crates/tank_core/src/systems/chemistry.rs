use crate::types::{
    BudgetDelta, ElementBudget, TankState, ADULT_SHRIMP_BIOMASS_G, JUVENILE_SHRIMP_BIOMASS_G,
};

pub fn compute_ph_from_totals(
    alkalinity_meq_total: f64,
    dissolved_inorganic_carbon_mg_c_total: f64,
    volume_l: f64,
) -> f64 {
    if volume_l <= f64::EPSILON {
        return 7.0;
    }

    let alkalinity_meq_l = alkalinity_meq_total / volume_l;
    let dic_mmol_l = (dissolved_inorganic_carbon_mg_c_total / 12.0) / volume_l;

    (6.3 + safe_log10(alkalinity_meq_l.max(0.05)) - safe_log10(dic_mmol_l.max(0.02)))
        .clamp(5.5, 8.5)
}

pub fn compute_nh3_mg_l(tan_mg_l: f64, ph: f64, temp_c: f64) -> f64 {
    let pka = 0.09018 + 2729.92 / (273.2 + temp_c);
    let fraction_nh3 = 1.0 / (1.0 + 10.0_f64.powf(pka - ph));
    tan_mg_l * fraction_nh3
}

pub fn step_hourly_chemistry(state: &mut TankState, light_on: bool) {
    let Some(terms) = hourly_chemistry_terms(state, light_on) else {
        return;
    };

    apply_hourly_chemistry_terms(state, terms);
}

pub fn step_hourly_chemistry_with_budget(state: &mut TankState, light_on: bool) -> BudgetDelta {
    let Some(terms) = hourly_chemistry_terms(state, light_on) else {
        return BudgetDelta::default();
    };

    BudgetDelta {
        carbon: apply_hourly_chemistry_terms(state, terms),
        ..BudgetDelta::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct HourlyChemistryTerms {
    respiration_dic_mg: f64,
    photosynthesis_dic_mg: f64,
}

fn hourly_chemistry_terms(state: &TankState, light_on: bool) -> Option<HourlyChemistryTerms> {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return None;
    }

    // These DIC terms are an intentional atmospheric-exchange simplification:
    // respiration adds CO2 into the lumped DIC pool and light-driven uptake
    // removes it again, even though there is no paired organic-C store inside
    // this hourly chemistry pass. Closed-system carbon conservation therefore
    // only holds when these rates are zeroed unless the guard subtracts this
    // explicit chemistry-stage source/sink budget; preset packs may opt into
    // this open-system shortcut without disabling the rest of the tick guard.
    let respiration_dic_mg = state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour
        * respiring_biomass_g(state);
    let photosynthesis_dic_mg = if light_on {
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour
            * photosynthetic_biomass_g(state)
            * state.hardware.light.intensity_index
    } else {
        0.0
    };

    Some(HourlyChemistryTerms {
        respiration_dic_mg,
        photosynthesis_dic_mg,
    })
}

fn apply_hourly_chemistry_terms(
    state: &mut TankState,
    terms: HourlyChemistryTerms,
) -> ElementBudget {
    let dic_before_mg = state.water.dissolved_inorganic_carbon_mg_c_total.max(0.0);
    let carbon_in_mg = terms.respiration_dic_mg.max(0.0);
    let carbon_out_requested_mg = terms.photosynthesis_dic_mg.max(0.0);
    let carbon_out_mg = carbon_out_requested_mg.min(dic_before_mg + carbon_in_mg);

    state.water.dissolved_inorganic_carbon_mg_c_total =
        (dic_before_mg + carbon_in_mg - carbon_out_mg).max(0.0);
    state.water.ph = compute_ph_from_totals(
        state.water.alkalinity_meq_total,
        state.water.dissolved_inorganic_carbon_mg_c_total,
        state.water_volume_l(),
    );

    ElementBudget {
        in_mg: carbon_in_mg,
        out_mg: carbon_out_mg,
    }
}

pub(crate) fn respiring_biomass_g(state: &TankState) -> f64 {
    let plant_biomass_g: f64 = state.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
    let algae_biomass_g = state.algae.suspended_biomass_g + state.algae.periphyton_biomass_g;
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let shrimp_biomass_g = (f64::from(state.animal.adults_count) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(state.animal.juveniles_count) * JUVENILE_SHRIMP_BIOMASS_G);

    plant_biomass_g + algae_biomass_g + microbe_biomass_g + shrimp_biomass_g
}

pub(crate) fn photosynthetic_biomass_g(state: &TankState) -> f64 {
    let plant_biomass_g: f64 = state.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
    plant_biomass_g + state.algae.periphyton_biomass_g + state.algae.suspended_biomass_g
}

fn safe_log10(value: f64) -> f64 {
    value.max(f64::MIN_POSITIVE).log10()
}
