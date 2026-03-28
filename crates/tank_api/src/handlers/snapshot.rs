use axum::{extract::State, Json};
use serde::Serialize;
use tank_core::SimulationEngine;

use crate::state::AppState;

pub async fn get_snapshot(State(state): State<AppState>) -> Json<tank_core::TankSnapshot> {
    let engine = state.engine.lock().unwrap();
    Json(engine.snapshot())
}

#[derive(Serialize)]
pub struct ChemistrySnapshot {
    pub ph: f64,
    pub tan_mg_n_per_l: f64,
    pub nh3_mg_n_per_l: f64,
    pub nitrite_mg_n_per_l: f64,
    pub nitrate_mg_n_per_l: f64,
    pub phosphate_mg_p_per_l: f64,
    pub dissolved_inorganic_carbon_mg_c_per_l: f64,
    pub do_mg_l: f64,
    pub do_sat_mg_l: f64,
    pub gh_d: f64,
    pub kh_d: f64,
    pub estimated_tds_7_ion_mg_per_l: f64,
    pub estimated_conductivity_us_cm: f64,
}

pub async fn get_chemistry(State(state): State<AppState>) -> Json<ChemistrySnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(ChemistrySnapshot {
        ph: s.ph,
        tan_mg_n_per_l: s.tan_mg_n_per_l,
        nh3_mg_n_per_l: s.nh3_mg_n_per_l,
        nitrite_mg_n_per_l: s.nitrite_mg_n_per_l,
        nitrate_mg_n_per_l: s.nitrate_mg_n_per_l,
        phosphate_mg_p_per_l: s.phosphate_mg_p_per_l,
        dissolved_inorganic_carbon_mg_c_per_l: s.dissolved_inorganic_carbon_mg_c_per_l,
        do_mg_l: s.do_mg_l,
        do_sat_mg_l: s.do_sat_mg_l,
        gh_d: s.gh_d,
        kh_d: s.kh_d,
        estimated_tds_7_ion_mg_per_l: s.estimated_tds_7_ion_mg_per_l,
        estimated_conductivity_us_cm: s.estimated_conductivity_us_cm,
    })
}

#[derive(Serialize)]
pub struct BiologySnapshot {
    pub adult_shrimp_count: u32,
    pub juveniles_count: u32,
    pub berried_females_count: u32,
    pub shrimp_condition_index: f64,
    pub shrimp_molt_stress_index: f64,
    pub shrimp_reproductive_readiness: f64,
    pub total_plant_biomass_g: f64,
    pub fast_stem_biomass_g: f64,
    pub fast_stem_health_index: f64,
    pub root_feeding_rosette_biomass_g: f64,
    pub root_feeding_rosette_health_index: f64,
    pub suspended_algae_biomass_g: f64,
    pub periphyton_biomass_g: f64,
    pub algae_nuisance_index: f64,
    pub microfauna_population_index: f64,
    pub microfauna_grazing_pressure_index: f64,
}

pub async fn get_biology(State(state): State<AppState>) -> Json<BiologySnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(BiologySnapshot {
        adult_shrimp_count: s.adult_shrimp_count,
        juveniles_count: s.juveniles_count,
        berried_females_count: s.berried_females_count,
        shrimp_condition_index: s.shrimp_condition_index,
        shrimp_molt_stress_index: s.shrimp_molt_stress_index,
        shrimp_reproductive_readiness: s.shrimp_reproductive_readiness,
        total_plant_biomass_g: s.total_plant_biomass_g,
        fast_stem_biomass_g: s.fast_stem_biomass_g,
        fast_stem_health_index: s.fast_stem_health_index,
        root_feeding_rosette_biomass_g: s.root_feeding_rosette_biomass_g,
        root_feeding_rosette_health_index: s.root_feeding_rosette_health_index,
        suspended_algae_biomass_g: s.suspended_algae_biomass_g,
        periphyton_biomass_g: s.periphyton_biomass_g,
        algae_nuisance_index: s.algae_nuisance_index,
        microfauna_population_index: s.microfauna_population_index,
        microfauna_grazing_pressure_index: s.microfauna_grazing_pressure_index,
    })
}

#[derive(Serialize)]
pub struct HardwareSnapshot {
    pub light_enabled: bool,
    pub photoperiod_hours: f64,
    pub light_intensity_index: f64,
    pub heater_enabled: bool,
    pub heater_setpoint_c: f64,
    pub last_heater_output_w: f64,
    pub aeration_enabled: bool,
    pub aeration_intensity: f64,
    pub filter_cleanliness_index: f64,
}

pub async fn get_hardware(State(state): State<AppState>) -> Json<HardwareSnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(HardwareSnapshot {
        light_enabled: s.light_enabled,
        photoperiod_hours: s.photoperiod_hours,
        light_intensity_index: s.light_intensity_index,
        heater_enabled: s.heater_enabled,
        heater_setpoint_c: s.heater_setpoint_c,
        last_heater_output_w: s.last_heater_output_w,
        aeration_enabled: s.aeration_enabled,
        aeration_intensity: s.aeration_intensity,
        filter_cleanliness_index: s.filter_cleanliness_index,
    })
}

#[derive(Serialize)]
pub struct BiofilterSnapshot {
    pub biofilter_maturity_index: f64,
    pub ammonia_oxidizer_biomass_g: f64,
    pub nitrite_oxidizer_biomass_g: f64,
    pub comammox_biomass_g: f64,
    pub decomposer_biomass_g: f64,
}

pub async fn get_biofilter(State(state): State<AppState>) -> Json<BiofilterSnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(BiofilterSnapshot {
        biofilter_maturity_index: s.biofilter_maturity_index,
        ammonia_oxidizer_biomass_g: s.ammonia_oxidizer_biomass_g,
        nitrite_oxidizer_biomass_g: s.nitrite_oxidizer_biomass_g,
        comammox_biomass_g: s.comammox_biomass_g,
        decomposer_biomass_g: s.decomposer_biomass_g,
    })
}

#[derive(Serialize)]
pub struct EnvironmentSnapshot {
    pub day: u32,
    pub hour: u8,
    pub ambient_temp_c: f64,
    pub water_temp_c: f64,
    pub water_volume_l: f64,
}

pub async fn get_environment(State(state): State<AppState>) -> Json<EnvironmentSnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(EnvironmentSnapshot {
        day: s.day,
        hour: s.hour,
        ambient_temp_c: s.ambient_temp_c,
        water_temp_c: s.water_temp_c,
        water_volume_l: s.water_volume_l,
    })
}
