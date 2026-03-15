use serde::{Deserialize, Serialize};

use super::{PlantGuild, SimEvent, TankState};
use crate::systems::{chemistry::compute_nh3_mg_l, temperature::do_sat_mg_l};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TankSnapshot {
    pub day: u32,
    pub hour: u8,
    pub water_temp_c: f64,
    pub water_volume_l: f64,
    pub tan_mg_l: f64,
    pub nh3_mg_l: f64,
    pub nitrite_mg_l: f64,
    pub nitrate_mg_l: f64,
    pub phosphate_mg_l: f64,
    pub do_mg_l: f64,
    pub do_sat_mg_l: f64,
    pub gh_d: f64,
    pub kh_d: f64,
    pub tds_mg_l: f64,
    pub conductivity_us_cm: f64,
    pub ph: f64,
    pub adult_shrimp_count: u32,
    pub juveniles_count: u32,
    pub berried_females_count: u32,
    pub shrimp_condition_index: f64,
    pub shrimp_molt_stress_index: f64,
    pub shrimp_reproductive_readiness: f64,
    pub microfauna_population_index: f64,
    pub microfauna_grazing_pressure_index: f64,
    pub total_plant_biomass_g: f64,
    pub fast_stem_biomass_g: f64,
    pub root_feeding_rosette_biomass_g: f64,
    pub suspended_algae_biomass_g: f64,
    pub periphyton_biomass_g: f64,
    pub algae_nuisance_index: f64,
    pub substrate_nutrient_remaining_mg_n_total: f64,
    pub substrate_nutrient_remaining_mg_p_total: f64,
    pub last_heater_output_w: f64,
    pub recent_events: Vec<SimEvent>,
}

impl TankSnapshot {
    pub fn from_state(state: &TankState) -> Self {
        let volume_l = state.geometry.water_volume_l();
        let tan_mg_l = safe_div(state.water.ammonia_total_mg_n_total, volume_l);
        let nitrite_mg_l = safe_div(state.water.nitrite_mg_n_total, volume_l);
        let nitrate_mg_l = safe_div(state.water.nitrate_mg_n_total, volume_l);
        let phosphate_mg_l = safe_div(state.water.phosphate_mg_p_total, volume_l);
        let do_mg_l_val = safe_div(state.water.dissolved_oxygen_mg_total, volume_l);
        let ca_mg_l = safe_div(state.water.calcium_mg_total, volume_l);
        let mg_mg_l = safe_div(state.water.magnesium_mg_total, volume_l);
        let alkalinity_meq_l = safe_div(state.water.alkalinity_meq_total, volume_l);
        let total_tracked_ions_mg = state.water.total_tracked_ions_mg();
        let tds_mg_l = safe_div(total_tracked_ions_mg, volume_l);
        let conductivity_us_cm = tds_mg_l / 0.65;
        let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
        let kh_d = (alkalinity_meq_l * 50.0) / 17.848;
        let ph = state.water.ph;
        let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, ph, state.water.temperature_c);
        let fast_stem_biomass_g: f64 = state
            .plant_guilds
            .iter()
            .filter(|plant| plant.guild == PlantGuild::FastStem)
            .map(|plant| plant.biomass_g)
            .sum();
        let root_feeding_rosette_biomass_g: f64 = state
            .plant_guilds
            .iter()
            .filter(|plant| plant.guild == PlantGuild::RootFeedingRosette)
            .map(|plant| plant.biomass_g)
            .sum();

        Self {
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            water_temp_c: state.water.temperature_c,
            water_volume_l: volume_l,
            tan_mg_l,
            nh3_mg_l,
            nitrite_mg_l,
            nitrate_mg_l,
            phosphate_mg_l,
            do_mg_l: do_mg_l_val,
            do_sat_mg_l: do_sat_mg_l(state.water.temperature_c),
            gh_d,
            kh_d,
            tds_mg_l,
            conductivity_us_cm,
            ph,
            adult_shrimp_count: state.animal.adults_count,
            juveniles_count: state.animal.juveniles_count,
            berried_females_count: state.animal.berried_females_count,
            shrimp_condition_index: state.animal.condition_index,
            shrimp_molt_stress_index: state.animal.molt_stress_index,
            shrimp_reproductive_readiness: state.animal.reproductive_readiness_index,
            microfauna_population_index: state.microfauna.population_index,
            microfauna_grazing_pressure_index: state.microfauna.grazing_pressure_index,
            total_plant_biomass_g: state.plant_guilds.iter().map(|plant| plant.biomass_g).sum(),
            fast_stem_biomass_g,
            root_feeding_rosette_biomass_g,
            suspended_algae_biomass_g: state.algae.suspended_biomass_g,
            periphyton_biomass_g: state.algae.periphyton_biomass_g,
            algae_nuisance_index: state.algae.nuisance_index,
            substrate_nutrient_remaining_mg_n_total: state
                .substrate_layers
                .iter()
                .map(|layer| layer.nutrient_store_mg_n_total)
                .sum(),
            substrate_nutrient_remaining_mg_p_total: state
                .substrate_layers
                .iter()
                .map(|layer| layer.nutrient_store_mg_p_total)
                .sum(),
            last_heater_output_w: state.hardware.heater.last_output_w,
            recent_events: state.event_log.iter().rev().take(20).cloned().collect(),
        }
    }
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator <= f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}
