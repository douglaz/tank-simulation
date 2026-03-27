use crate::types::TankState;

const ADULT_SHRIMP_BIOMASS_G: f64 = 0.12;
const JUVENILE_SHRIMP_BIOMASS_G: f64 = 0.05;

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
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

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
    let net_dic_delta_mg = respiration_dic_mg - photosynthesis_dic_mg;

    state.water.dissolved_inorganic_carbon_mg_c_total =
        (state.water.dissolved_inorganic_carbon_mg_c_total + net_dic_delta_mg).max(0.0);
    state.water.ph = compute_ph_from_totals(
        state.water.alkalinity_meq_total,
        state.water.dissolved_inorganic_carbon_mg_c_total,
        volume_l,
    );
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
