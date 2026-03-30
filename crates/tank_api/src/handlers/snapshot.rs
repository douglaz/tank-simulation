use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::{json, Map, Value};
use tank_core::{
    SimulationEngine, TankSnapshot, ESTIMATED_TDS_OMITTED_CONTRIBUTORS,
    ESTIMATED_TDS_TRACKED_MAJOR_IONS, LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES,
};

use crate::state::AppState;

pub(crate) fn snapshot_response_json(snapshot: &TankSnapshot) -> Value {
    let mut value = serde_json::to_value(snapshot).expect("TankSnapshot should serialize");
    let object = value
        .as_object_mut()
        .expect("TankSnapshot JSON should be an object");
    insert_legacy_chemistry_fields(object);
    object.insert(
        "chemistry_field_semantics".to_string(),
        chemistry_field_semantics_json(),
    );
    object.insert(
        "estimated_tds_scope".to_string(),
        estimated_tds_scope_json(),
    );
    object.insert(
        "legacy_chemistry_aliases".to_string(),
        legacy_chemistry_aliases_json(),
    );
    value
}

pub(crate) fn chemistry_response_json(snapshot: &TankSnapshot) -> Value {
    let mut object = Map::from_iter([
        ("ph".to_string(), json!(snapshot.ph)),
        ("tan_mg_n_per_l".to_string(), json!(snapshot.tan_mg_n_per_l)),
        ("nh3_mg_n_per_l".to_string(), json!(snapshot.nh3_mg_n_per_l)),
        (
            "nitrite_mg_n_per_l".to_string(),
            json!(snapshot.nitrite_mg_n_per_l),
        ),
        (
            "nitrate_mg_n_per_l".to_string(),
            json!(snapshot.nitrate_mg_n_per_l),
        ),
        (
            "phosphate_mg_p_per_l".to_string(),
            json!(snapshot.phosphate_mg_p_per_l),
        ),
        (
            "dissolved_inorganic_carbon_mg_c_per_l".to_string(),
            json!(snapshot.dissolved_inorganic_carbon_mg_c_per_l),
        ),
        ("do_mg_l".to_string(), json!(snapshot.do_mg_l)),
        ("do_sat_mg_l".to_string(), json!(snapshot.do_sat_mg_l)),
        ("gh_d".to_string(), json!(snapshot.gh_d)),
        ("kh_d".to_string(), json!(snapshot.kh_d)),
        (
            "estimated_tds_7_ion_mg_per_l".to_string(),
            json!(snapshot.estimated_tds_7_ion_mg_per_l),
        ),
        (
            "estimated_conductivity_us_cm".to_string(),
            json!(snapshot.estimated_conductivity_us_cm),
        ),
        (
            "chemistry_field_semantics".to_string(),
            chemistry_field_semantics_json(),
        ),
        (
            "estimated_tds_scope".to_string(),
            estimated_tds_scope_json(),
        ),
        (
            "legacy_chemistry_aliases".to_string(),
            legacy_chemistry_aliases_json(),
        ),
    ]);
    insert_legacy_chemistry_fields(&mut object);
    Value::Object(object)
}

fn insert_legacy_chemistry_fields(object: &mut Map<String, Value>) {
    for (legacy, canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
        let Some(value) = object.get(canonical).cloned() else {
            continue;
        };
        object.insert(legacy.to_string(), value);
    }
}

fn chemistry_field_semantics_json() -> Value {
    json!({
        "tan_mg_n_per_l": "Total ammonia nitrogen, mg N/L.",
        "nh3_mg_n_per_l": "Un-ionized free ammonia on a nitrogen basis, mg NH3-N/L.",
        "nitrite_mg_n_per_l": "Nitrite on a nitrogen basis, mg N/L.",
        "nitrate_mg_n_per_l": "Nitrate on a nitrogen basis, mg N/L.",
        "phosphate_mg_p_per_l": "Orthophosphate on a phosphorus basis, mg P/L.",
        "dissolved_inorganic_carbon_mg_c_per_l": "Dissolved inorganic carbon on a carbon basis, mg C/L.",
        "do_mg_l": "Dissolved oxygen concentration, mg O2/L.",
        "do_sat_mg_l": "Oxygen saturation concentration at the current temperature, mg O2/L.",
        "gh_d": "General hardness derived from tracked calcium plus magnesium, in German degrees.",
        "kh_d": "Carbonate hardness proxy derived from tracked alkalinity, in German degrees.",
        "estimated_tds_7_ion_mg_per_l": "Estimated TDS from 7 tracked major ions only, in mg/L.",
        "estimated_conductivity_us_cm": "Estimated conductivity derived from the same 7-ion proxy, in uS/cm."
    })
}

fn estimated_tds_scope_json() -> Value {
    json!({
        "tracked_major_ions": ESTIMATED_TDS_TRACKED_MAJOR_IONS,
        "omitted_contributors": ESTIMATED_TDS_OMITTED_CONTRIBUTORS
    })
}

fn legacy_chemistry_aliases_json() -> Value {
    Value::Object(Map::from_iter(
        LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES
            .into_iter()
            .map(|(legacy, canonical)| (legacy.to_string(), json!(canonical))),
    ))
}

pub async fn get_snapshot(State(state): State<AppState>) -> Json<Value> {
    let engine = state.engine.lock().unwrap();
    Json(snapshot_response_json(&engine.snapshot()))
}

pub async fn get_chemistry(State(state): State<AppState>) -> Json<Value> {
    let engine = state.engine.lock().unwrap();
    Json(chemistry_response_json(&engine.snapshot()))
}

#[derive(Serialize)]
pub struct BiologySnapshot {
    pub total_shrimp_count: u32,
    pub adult_shrimp_count: u32,
    pub sub_adult_count: u32,
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

fn biology_snapshot_from_snapshot(s: &TankSnapshot) -> BiologySnapshot {
    BiologySnapshot {
        total_shrimp_count: s.total_shrimp_count,
        adult_shrimp_count: s.adult_shrimp_count,
        sub_adult_count: s.sub_adult_count,
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
    }
}

pub async fn get_biology(State(state): State<AppState>) -> Json<BiologySnapshot> {
    let engine = state.engine.lock().unwrap();
    let s = engine.snapshot();
    Json(biology_snapshot_from_snapshot(&s))
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tank_core::{
        SimSeed, TankState, ESTIMATED_TDS_OMITTED_CONTRIBUTORS, ESTIMATED_TDS_TRACKED_MAJOR_IONS,
    };

    use super::{biology_snapshot_from_snapshot, chemistry_response_json, snapshot_response_json};

    #[test]
    fn snapshot_response_keeps_legacy_chemistry_aliases() {
        let snapshot = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(7001)));
        let value = snapshot_response_json(&snapshot);

        assert_eq!(value["tan_mg_l"], json!(snapshot.tan_mg_n_per_l));
        assert_eq!(value["nh3_mg_l"], json!(snapshot.nh3_mg_n_per_l));
        assert_eq!(value["nitrite_mg_l"], json!(snapshot.nitrite_mg_n_per_l));
        assert_eq!(
            value["tds_mg_l"],
            json!(snapshot.estimated_tds_7_ion_mg_per_l)
        );
        assert_eq!(
            value["dissolved_inorganic_carbon_mg_l"],
            json!(snapshot.dissolved_inorganic_carbon_mg_c_per_l)
        );
        assert_eq!(
            value["legacy_chemistry_aliases"]["dissolved_inorganic_carbon_mg_l"],
            json!("dissolved_inorganic_carbon_mg_c_per_l")
        );
        assert_eq!(
            value["legacy_chemistry_aliases"]["conductivity_us_cm"],
            json!("estimated_conductivity_us_cm")
        );
    }

    #[test]
    fn chemistry_response_explains_estimate_scope() {
        let snapshot = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(7002)));
        let value = chemistry_response_json(&snapshot);

        assert_eq!(
            value["chemistry_field_semantics"]["do_mg_l"],
            json!("Dissolved oxygen concentration, mg O2/L.")
        );
        assert_eq!(
            value["chemistry_field_semantics"]["do_sat_mg_l"],
            json!("Oxygen saturation concentration at the current temperature, mg O2/L.")
        );
        assert_eq!(
            value["chemistry_field_semantics"]["gh_d"],
            json!(
                "General hardness derived from tracked calcium plus magnesium, in German degrees."
            )
        );
        assert_eq!(
            value["chemistry_field_semantics"]["estimated_tds_7_ion_mg_per_l"],
            json!("Estimated TDS from 7 tracked major ions only, in mg/L.")
        );
        assert_eq!(
            value["estimated_tds_scope"]["tracked_major_ions"]
                .as_array()
                .expect("tracked ion list"),
            &ESTIMATED_TDS_TRACKED_MAJOR_IONS
                .into_iter()
                .map(|ion| json!(ion))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            value["estimated_tds_scope"]["omitted_contributors"][0],
            json!(ESTIMATED_TDS_OMITTED_CONTRIBUTORS[0])
        );
        assert_eq!(
            value["phosphate_mg_l"],
            json!(snapshot.phosphate_mg_p_per_l)
        );
    }

    #[test]
    fn biology_snapshot_keeps_total_and_stage_breakdown() {
        let mut state = TankState::new(SimSeed(7003));
        state.animal.adult.count = 6;
        state.animal.sub_adult.count = 4;
        state.animal.juvenile.count = 3;
        state.animal.berried_females_count = 2;

        let snapshot = tank_core::TankSnapshot::from_state(&state);
        let biology = serde_json::to_value(biology_snapshot_from_snapshot(&snapshot))
            .expect("biology snapshot should serialize");

        assert_eq!(biology["total_shrimp_count"], json!(13));
        assert_eq!(biology["adult_shrimp_count"], json!(6));
        assert_eq!(biology["sub_adult_count"], json!(4));
        assert_eq!(biology["juveniles_count"], json!(3));
        assert_eq!(biology["berried_females_count"], json!(2));
    }
}
