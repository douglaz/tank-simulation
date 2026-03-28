use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::{PlantGuild, SimEvent, TankState};
use crate::systems::{chemistry::compute_nh3_mg_n_per_l, temperature::do_sat_mg_l};

/// Legacy chemistry field names accepted during snapshot deserialization and
/// still emitted in API responses for compatibility.
pub const LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES: [(&str, &str); 8] = [
    ("tan_mg_l", "tan_mg_n_per_l"),
    ("nh3_mg_l", "nh3_mg_n_per_l"),
    ("nitrite_mg_l", "nitrite_mg_n_per_l"),
    ("nitrate_mg_l", "nitrate_mg_n_per_l"),
    ("phosphate_mg_l", "phosphate_mg_p_per_l"),
    (
        "dissolved_inorganic_carbon_mg_l",
        "dissolved_inorganic_carbon_mg_c_per_l",
    ),
    ("tds_mg_l", "estimated_tds_7_ion_mg_per_l"),
    ("conductivity_us_cm", "estimated_conductivity_us_cm"),
];

#[derive(Debug, Clone, Serialize, PartialEq)]
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
    pub total_shrimp_count: u32,
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

#[derive(Debug, Deserialize)]
struct TankSnapshotRepr {
    day: u32,
    hour: u8,
    ambient_temp_c: f64,
    water_temp_c: f64,
    water_volume_l: f64,
    tan_mg_n_per_l: f64,
    nh3_mg_n_per_l: f64,
    nitrite_mg_n_per_l: f64,
    nitrate_mg_n_per_l: f64,
    phosphate_mg_p_per_l: f64,
    dissolved_inorganic_carbon_mg_c_per_l: f64,
    co2_aq_mmol_per_l: f64,
    do_mg_l: f64,
    do_sat_mg_l: f64,
    gh_d: f64,
    kh_d: f64,
    estimated_tds_7_ion_mg_per_l: f64,
    estimated_conductivity_us_cm: f64,
    ph: f64,
    light_enabled: bool,
    photoperiod_hours: f64,
    light_intensity_index: f64,
    heater_enabled: bool,
    heater_setpoint_c: f64,
    aeration_enabled: bool,
    aeration_intensity: f64,
    filter_cleanliness_index: f64,
    total_shrimp_count: u32,
    adult_shrimp_count: u32,
    sub_adult_count: u32,
    juveniles_count: u32,
    berried_females_count: u32,
    shrimp_condition_index: f64,
    shrimp_molt_stress_index: f64,
    shrimp_reproductive_readiness: f64,
    microfauna_population_index: f64,
    microfauna_grazing_pressure_index: f64,
    total_plant_biomass_g: f64,
    fast_stem_biomass_g: f64,
    fast_stem_health_index: f64,
    root_feeding_rosette_biomass_g: f64,
    root_feeding_rosette_health_index: f64,
    suspended_algae_biomass_g: f64,
    periphyton_biomass_g: f64,
    algae_nuisance_index: f64,
    detritus_particulate_g_total: f64,
    detritus_fine_g_total: f64,
    substrate_nutrient_remaining_mg_n_total: f64,
    substrate_nutrient_remaining_mg_p_total: f64,
    biofilter_maturity_index: f64,
    ammonia_oxidizer_biomass_g: f64,
    nitrite_oxidizer_biomass_g: f64,
    comammox_biomass_g: f64,
    decomposer_biomass_g: f64,
    last_heater_output_w: f64,
    recent_events: Vec<SimEvent>,
}

impl From<TankSnapshotRepr> for TankSnapshot {
    fn from(value: TankSnapshotRepr) -> Self {
        Self {
            day: value.day,
            hour: value.hour,
            ambient_temp_c: value.ambient_temp_c,
            water_temp_c: value.water_temp_c,
            water_volume_l: value.water_volume_l,
            tan_mg_n_per_l: value.tan_mg_n_per_l,
            nh3_mg_n_per_l: value.nh3_mg_n_per_l,
            nitrite_mg_n_per_l: value.nitrite_mg_n_per_l,
            nitrate_mg_n_per_l: value.nitrate_mg_n_per_l,
            phosphate_mg_p_per_l: value.phosphate_mg_p_per_l,
            dissolved_inorganic_carbon_mg_c_per_l: value.dissolved_inorganic_carbon_mg_c_per_l,
            co2_aq_mmol_per_l: value.co2_aq_mmol_per_l,
            do_mg_l: value.do_mg_l,
            do_sat_mg_l: value.do_sat_mg_l,
            gh_d: value.gh_d,
            kh_d: value.kh_d,
            estimated_tds_7_ion_mg_per_l: value.estimated_tds_7_ion_mg_per_l,
            estimated_conductivity_us_cm: value.estimated_conductivity_us_cm,
            ph: value.ph,
            light_enabled: value.light_enabled,
            photoperiod_hours: value.photoperiod_hours,
            light_intensity_index: value.light_intensity_index,
            heater_enabled: value.heater_enabled,
            heater_setpoint_c: value.heater_setpoint_c,
            aeration_enabled: value.aeration_enabled,
            aeration_intensity: value.aeration_intensity,
            filter_cleanliness_index: value.filter_cleanliness_index,
            total_shrimp_count: value.total_shrimp_count,
            adult_shrimp_count: value.adult_shrimp_count,
            sub_adult_count: value.sub_adult_count,
            juveniles_count: value.juveniles_count,
            berried_females_count: value.berried_females_count,
            shrimp_condition_index: value.shrimp_condition_index,
            shrimp_molt_stress_index: value.shrimp_molt_stress_index,
            shrimp_reproductive_readiness: value.shrimp_reproductive_readiness,
            microfauna_population_index: value.microfauna_population_index,
            microfauna_grazing_pressure_index: value.microfauna_grazing_pressure_index,
            total_plant_biomass_g: value.total_plant_biomass_g,
            fast_stem_biomass_g: value.fast_stem_biomass_g,
            fast_stem_health_index: value.fast_stem_health_index,
            root_feeding_rosette_biomass_g: value.root_feeding_rosette_biomass_g,
            root_feeding_rosette_health_index: value.root_feeding_rosette_health_index,
            suspended_algae_biomass_g: value.suspended_algae_biomass_g,
            periphyton_biomass_g: value.periphyton_biomass_g,
            algae_nuisance_index: value.algae_nuisance_index,
            detritus_particulate_g_total: value.detritus_particulate_g_total,
            detritus_fine_g_total: value.detritus_fine_g_total,
            substrate_nutrient_remaining_mg_n_total: value.substrate_nutrient_remaining_mg_n_total,
            substrate_nutrient_remaining_mg_p_total: value.substrate_nutrient_remaining_mg_p_total,
            biofilter_maturity_index: value.biofilter_maturity_index,
            ammonia_oxidizer_biomass_g: value.ammonia_oxidizer_biomass_g,
            nitrite_oxidizer_biomass_g: value.nitrite_oxidizer_biomass_g,
            comammox_biomass_g: value.comammox_biomass_g,
            decomposer_biomass_g: value.decomposer_biomass_g,
            last_heater_output_w: value.last_heater_output_w,
            recent_events: value.recent_events,
        }
    }
}

fn normalize_snapshot_object(object: &mut Map<String, Value>) {
    for (legacy, canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
        if object.contains_key(canonical) {
            object.remove(legacy);
            continue;
        }
        if let Some(legacy_value) = object.remove(legacy) {
            object.insert(canonical.to_string(), legacy_value);
        }
    }
}

impl<'de> Deserialize<'de> for TankSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = Map::<String, Value>::deserialize(deserializer)?;
        normalize_snapshot_object(&mut object);
        serde_json::from_value::<TankSnapshotRepr>(Value::Object(object))
            .map(Into::into)
            .map_err(D::Error::custom)
    }
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
        let bicarbonate_mg_total =
            bicarbonate_mg_total_from_mmol_per_l(carbonate_eq.hco3_mmol_per_l, volume_l);
        let estimated_tds_7_ion_mg_per_l = state
            .water
            .tds_mg_per_l_with_bicarbonate_total(volume_l, bicarbonate_mg_total);
        let estimated_conductivity_us_cm = state
            .water
            .conductivity_us_cm_with_bicarbonate_total(volume_l, bicarbonate_mg_total);
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
            total_shrimp_count: state.animal.total_count(),
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
    use serde_json::{json, Map};

    use super::{TankSnapshot, LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES};
    use crate::{
        rng::SimSeed,
        systems::chemistry::{bicarbonate_mg_total_from_mmol_per_l, solve_carbonate_equilibrium},
        TankState, ESTIMATED_TDS_OMITTED_CONTRIBUTORS, ESTIMATED_TDS_TRACKED_MAJOR_IONS,
    };

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual} (tolerance {tolerance})"
        );
    }

    fn canonical_snapshot_value(seed: SimSeed) -> serde_json::Value {
        serde_json::to_value(TankSnapshot::from_state(&TankState::new(seed)))
            .expect("snapshot should serialize")
    }

    fn legacy_snapshot_value(seed: SimSeed) -> serde_json::Value {
        let mut value = canonical_snapshot_value(seed);
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        for (legacy, canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
            let canonical_value = object
                .remove(canonical)
                .expect("compatibility test expects canonical chemistry field");
            object.insert(legacy.to_string(), canonical_value);
        }
        value
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

    #[test]
    fn snapshot_tds_and_conductivity_use_fresh_carbonate_projection() {
        let mut state = TankState::new(SimSeed(406));
        let volume_l = state.water_volume_l();
        state.water.dissolved_inorganic_carbon_mg_c_total = 42.0 * volume_l;
        state.water.alkalinity_meq_total = 3.4 * volume_l;
        state.water.bicarbonate_mg_total = 0.0;

        let expected = solve_carbonate_equilibrium(
            state.water.dissolved_inorganic_carbon_mg_c_total,
            state.water.alkalinity_meq_total,
            state.water.temperature_c,
            volume_l,
        );
        let fresh = state
            .water
            .estimated_dissolved_solids_with_carbonate_equilibrium(volume_l, expected);
        let snapshot = TankSnapshot::from_state(&state);

        assert!(fresh.bicarbonate_mg_total > 0.0);
        assert_close(
            snapshot.estimated_tds_7_ion_mg_per_l,
            fresh.tds_mg_per_l,
            1e-12,
        );
        assert_close(
            snapshot.estimated_conductivity_us_cm,
            fresh.conductivity_us_cm,
            1e-12,
        );
        assert_close(
            snapshot.estimated_tds_7_ion_mg_per_l,
            state.tds_mg_per_l(),
            1e-12,
        );
        assert_close(
            snapshot.estimated_conductivity_us_cm,
            state.conductivity_us_cm(),
            1e-12,
        );
    }

    #[test]
    fn snapshot_exposes_total_and_stage_shrimp_counts() {
        let mut state = TankState::new(SimSeed(405));
        state.animal.adult.count = 4;
        state.animal.sub_adult.count = 3;
        state.animal.juvenile.count = 2;
        state.animal.berried_females_count = 1;

        let snapshot = TankSnapshot::from_state(&state);

        assert_eq!(snapshot.total_shrimp_count, 9);
        assert_eq!(snapshot.adult_shrimp_count, 4);
        assert_eq!(snapshot.sub_adult_count, 3);
        assert_eq!(snapshot.juveniles_count, 2);
        assert_eq!(snapshot.berried_females_count, 1);
    }

    #[test]
    fn snapshot_deserializes_legacy_chemistry_aliases() {
        let expected = TankSnapshot::from_state(&TankState::new(SimSeed(777)));
        let value = legacy_snapshot_value(SimSeed(777));

        let parsed: TankSnapshot =
            serde_json::from_value(value).expect("legacy snapshot should deserialize");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn snapshot_deserializes_enriched_api_payload_with_canonical_precedence() {
        let expected = TankSnapshot::from_state(&TankState::new(SimSeed(778)));
        let mut value = canonical_snapshot_value(SimSeed(778));
        let object = value
            .as_object_mut()
            .expect("serialized snapshot should be an object");
        for (legacy, _canonical) in LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES {
            object.insert(legacy.to_string(), json!(-999.0));
        }
        object.insert(
            "chemistry_field_semantics".to_string(),
            json!({
                "tan_mg_n_per_l": "Total ammonia nitrogen, mg N/L.",
                "estimated_tds_7_ion_mg_per_l": "Estimated TDS from 7 tracked major ions only, in mg/L."
            }),
        );
        object.insert(
            "estimated_tds_scope".to_string(),
            json!({
                "tracked_major_ions": ESTIMATED_TDS_TRACKED_MAJOR_IONS,
                "omitted_contributors": ESTIMATED_TDS_OMITTED_CONTRIBUTORS,
            }),
        );
        object.insert(
            "legacy_chemistry_aliases".to_string(),
            serde_json::Value::Object(Map::from_iter(
                LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES
                    .into_iter()
                    .map(|(legacy, canonical)| (legacy.to_string(), json!(canonical))),
            )),
        );

        let parsed: TankSnapshot =
            serde_json::from_value(value).expect("enriched snapshot should deserialize");

        assert_eq!(parsed, expected);
    }
}
