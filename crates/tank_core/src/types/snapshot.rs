use serde::{Deserialize, Serialize};

use super::{PlantGuild, SimEvent, TankState};
use crate::systems::{
    chemistry::{compute_nh3_mg_n_per_l, solve_carbonate_equilibrium},
    temperature::do_sat_mg_l,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TankSnapshot {
    pub day: u32,
    pub hour: u8,
    pub ambient_temp_c: f64,
    pub water_temp_c: f64,
    pub water_volume_l: f64,
    pub tan_mg_n_per_l: f64,
    pub nh3_mg_n_per_l: f64,
    pub nitrite_mg_n_per_l: f64,
    pub nitrate_mg_n_per_l: f64,
    pub phosphate_mg_p_per_l: f64,
    pub dissolved_inorganic_carbon_mg_c_per_l: f64,
    pub co2_aq_mmol_per_l: f64,
    pub do_mg_l: f64,
    pub do_sat_mg_l: f64,
    pub gh_d: f64,
    pub kh_d: f64,
    pub estimated_tds_7_ion_mg_per_l: f64,
    pub estimated_conductivity_us_cm: f64,
    pub ph: f64,
    pub light_enabled: bool,
    pub photoperiod_hours: f64,
    pub light_intensity_index: f64,
    pub heater_enabled: bool,
    pub heater_setpoint_c: f64,
    pub aeration_enabled: bool,
    pub aeration_intensity: f64,
    pub filter_cleanliness_index: f64,
    pub adult_shrimp_count: u32,
    pub sub_adult_count: u32,
    pub juveniles_count: u32,
    pub berried_females_count: u32,
    pub shrimp_condition_index: f64,
    pub shrimp_molt_stress_index: f64,
    pub shrimp_reproductive_readiness: f64,
    pub microfauna_population_index: f64,
    pub microfauna_grazing_pressure_index: f64,
    pub total_plant_biomass_g: f64,
    pub fast_stem_biomass_g: f64,
    pub fast_stem_health_index: f64,
    pub root_feeding_rosette_biomass_g: f64,
    pub root_feeding_rosette_health_index: f64,
    pub suspended_algae_biomass_g: f64,
    pub periphyton_biomass_g: f64,
    pub algae_nuisance_index: f64,
    pub detritus_particulate_g_total: f64,
    pub detritus_fine_g_total: f64,
    pub substrate_nutrient_remaining_mg_n_total: f64,
    pub substrate_nutrient_remaining_mg_p_total: f64,
    pub biofilter_maturity_index: f64,
    pub ammonia_oxidizer_biomass_g: f64,
    pub nitrite_oxidizer_biomass_g: f64,
    pub comammox_biomass_g: f64,
    pub decomposer_biomass_g: f64,
    pub last_heater_output_w: f64,
    pub recent_events: Vec<SimEvent>,
}

impl TankSnapshot {
    pub fn from_state(state: &TankState) -> Self {
        let chemistry = state.concentrations();
        let volume_l = chemistry.volume_l();
        let tan_mg_n_per_l = chemistry.tan_mg_n_per_l();
        let nitrite_mg_n_per_l = chemistry.nitrite_mg_n_per_l();
        let nitrate_mg_n_per_l = chemistry.nitrate_mg_n_per_l();
        let phosphate_mg_p_per_l = chemistry.phosphate_mg_p_per_l();
        let dissolved_inorganic_carbon_mg_c_per_l = chemistry.dic_mg_c_per_l();
        let carbonate_eq = solve_carbonate_equilibrium(
            state.water.dissolved_inorganic_carbon_mg_c_total,
            state.water.alkalinity_meq_total,
            state.water.temperature_c,
            volume_l,
        );
        let do_mg_l_val = chemistry.do_mg_per_l();
        let gh_d = chemistry.gh_d();
        let kh_d = chemistry.kh_d();
        let estimated_tds_7_ion_mg_per_l = chemistry.tds_mg_per_l();
        let estimated_conductivity_us_cm = chemistry.conductivity_us_cm();
        let ph = carbonate_eq.ph;
        let nh3_mg_n_per_l = compute_nh3_mg_n_per_l(tan_mg_n_per_l, ph, state.water.temperature_c);
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
        let fast_stem_health_index =
            average_guild_health_index(state, PlantGuild::FastStem).unwrap_or(0.0);
        let root_feeding_rosette_health_index =
            average_guild_health_index(state, PlantGuild::RootFeedingRosette).unwrap_or(0.0);
        let mut recent_events: Vec<_> = state.event_log.iter().rev().take(20).cloned().collect();
        recent_events.reverse();

        Self {
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            ambient_temp_c: state.environment.ambient_temp_c,
            water_temp_c: state.water.temperature_c,
            water_volume_l: volume_l,
            tan_mg_n_per_l,
            nh3_mg_n_per_l,
            nitrite_mg_n_per_l,
            nitrate_mg_n_per_l,
            phosphate_mg_p_per_l,
            dissolved_inorganic_carbon_mg_c_per_l,
            co2_aq_mmol_per_l: carbonate_eq.co2_aq_mmol_per_l,
            do_mg_l: do_mg_l_val,
            do_sat_mg_l: do_sat_mg_l(state.water.temperature_c),
            gh_d,
            kh_d,
            estimated_tds_7_ion_mg_per_l,
            estimated_conductivity_us_cm,
            ph,
            light_enabled: state.hardware.light.enabled,
            photoperiod_hours: state.hardware.light.photoperiod_hours,
            light_intensity_index: state.hardware.light.intensity_index,
            heater_enabled: state.hardware.heater.enabled,
            heater_setpoint_c: state.hardware.heater.setpoint_c,
            aeration_enabled: state.hardware.aeration.enabled,
            aeration_intensity: state.hardware.aeration.intensity,
            filter_cleanliness_index: state.hardware.filter.cleanliness_index,
            adult_shrimp_count: state.animal.adult.count,
            sub_adult_count: state.animal.sub_adult.count,
            juveniles_count: state.animal.juvenile.count,
            berried_females_count: state.animal.berried_females_count,
            shrimp_condition_index: state.animal.population_condition_index(),
            shrimp_molt_stress_index: state.animal.molt_stress_index,
            shrimp_reproductive_readiness: state.animal.reproductive_readiness_index,
            microfauna_population_index: state.microfauna.population_index,
            microfauna_grazing_pressure_index: state.microfauna.grazing_pressure_index,
            total_plant_biomass_g: state.plant_guilds.iter().map(|plant| plant.biomass_g).sum(),
            fast_stem_biomass_g,
            fast_stem_health_index,
            root_feeding_rosette_biomass_g,
            root_feeding_rosette_health_index,
            suspended_algae_biomass_g: state.algae.suspended_biomass_g,
            periphyton_biomass_g: state.algae.periphyton_biomass_g,
            algae_nuisance_index: state.algae.nuisance_index,
            detritus_particulate_g_total: state.detritus.particulate_organics_g_total,
            detritus_fine_g_total: state.detritus.fine_detritus_g_total,
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
            biofilter_maturity_index: state.filter_state.biofilter_maturity_index,
            ammonia_oxidizer_biomass_g: state.microbe.ammonia_oxidizer_biomass_g,
            nitrite_oxidizer_biomass_g: state.microbe.nitrite_oxidizer_biomass_g,
            comammox_biomass_g: state.microbe.comammox_biomass_g,
            decomposer_biomass_g: state.microbe.decomposer_biomass_g,
            last_heater_output_w: state.hardware.heater.last_output_w,
            recent_events,
        }
    }
}

fn average_guild_health_index(state: &TankState, guild: PlantGuild) -> Option<f64> {
    let matching: Vec<_> = state
        .plant_guilds
        .iter()
        .filter(|plant| plant.guild == guild)
        .collect();

    if matching.is_empty() {
        None
    } else {
        Some(matching.iter().map(|plant| plant.health_index).sum::<f64>() / matching.len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::TankSnapshot;
    use crate::{rng::SimSeed, systems::chemistry::solve_carbonate_equilibrium, TankState};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual} (tolerance {tolerance})"
        );
    }

    #[test]
    fn snapshot_uses_one_carbonate_solution_for_ph_and_co2() {
        let mut state = TankState::new(SimSeed(404));
        let volume_l = state.water_volume_l();
        state.water.dissolved_inorganic_carbon_mg_c_total = 8.0 * volume_l;
        state.water.alkalinity_meq_total = 2.6 * volume_l;
        state.water.ph = 6.1;

        let expected = solve_carbonate_equilibrium(
            state.water.dissolved_inorganic_carbon_mg_c_total,
            state.water.alkalinity_meq_total,
            state.water.temperature_c,
            volume_l,
        );
        let snapshot = TankSnapshot::from_state(&state);

        assert_close(snapshot.ph, expected.ph, 1e-12);
        assert_close(
            snapshot.co2_aq_mmol_per_l,
            expected.co2_aq_mmol_per_l,
            1e-12,
        );
    }
}
