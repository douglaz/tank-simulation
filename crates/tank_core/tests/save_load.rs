use tank_core::systems::chemistry::solve_carbonate_equilibrium;
use tank_core::systems::shrimp::update_stability_tracker;
use tank_core::{
    compute_habitat_registry, shrimp_biomass_g, Engine, MicrobeState, PlayerAction, ProcessParams,
    SaveFile, ShrimpRuntimeParams, SimError, SimSeed, SimulationEngine, SourceWaterProfile,
    StabilityTracker, TankState, WaterState, APP_VERSION, LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
    SCHEMA_VERSION,
};

fn legacy_pre_stage_state_json(
    state: &TankState,
    adults_count: u32,
    juveniles_count: u32,
    reserve_g: Option<f64>,
    condition_index: f64,
    maturation_accum: f64,
) -> serde_json::Value {
    let mut state_json = serde_json::to_value(state).expect("serialize legacy state");
    let state_obj = state_json.as_object_mut().expect("state json object");
    state_obj.remove("habitat_registry");
    state_obj
        .get_mut("geometry")
        .and_then(serde_json::Value::as_object_mut)
        .expect("geometry object")
        .remove("hardscape_area_cm2");
    state_obj
        .get_mut("hardware")
        .and_then(serde_json::Value::as_object_mut)
        .expect("hardware object")
        .get_mut("filter")
        .and_then(serde_json::Value::as_object_mut)
        .expect("filter object")
        .remove("media_area_cm2");
    state_obj.remove("shrimp_params");

    let mut animal = serde_json::Map::new();
    animal.insert("adults_count".to_string(), serde_json::json!(adults_count));
    animal.insert(
        "juveniles_count".to_string(),
        serde_json::json!(juveniles_count),
    );
    animal.insert(
        "condition_index".to_string(),
        serde_json::json!(condition_index),
    );
    animal.insert(
        "maturation_accum".to_string(),
        serde_json::json!(maturation_accum),
    );
    animal.insert(
        "berried_females_count".to_string(),
        serde_json::json!(state.animal.berried_females_count),
    );
    animal.insert(
        "molt_stress_index".to_string(),
        serde_json::json!(state.animal.molt_stress_index),
    );
    animal.insert(
        "reproductive_readiness_index".to_string(),
        serde_json::json!(state.animal.reproductive_readiness_index),
    );
    animal.insert(
        "egg_progress_days".to_string(),
        serde_json::json!(state.animal.egg_progress_days),
    );
    if let Some(reserve_g) = reserve_g {
        animal.insert("reserve_g".to_string(), serde_json::json!(reserve_g));
    }
    state_obj.insert("animal".to_string(), serde_json::Value::Object(animal));

    state_json
}

#[test]
fn save_load_roundtrip() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::new(SimSeed(42));
    engine.apply_action(PlayerAction::Feed { grams: 1.25 })?;
    engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 25.5 })?;
    engine.step_hours(1)?;
    engine.apply_action(PlayerAction::ChangeLightIntensity {
        intensity_index: 0.6,
    })?;

    let save = SaveFile::from_engine(&engine);
    let json = save.to_json_pretty()?;
    let restored = SaveFile::from_json(&json)?;
    let restored_engine = restored.clone().into_engine()?;

    assert_eq!(save, restored);
    assert_eq!(engine.full_state(), restored_engine.full_state());
    assert_eq!(engine.queued_actions(), restored_engine.queued_actions());

    Ok(())
}

#[test]
fn current_schema_load_rebuilds_stale_habitat_registry() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(142));
    let stale_registry = state.habitat_registry.clone();
    state.plant_guilds[0].biomass_g *= 3.0;
    state.habitat_registry = stale_registry;

    let json = serde_json::to_string(&SaveFile {
        schema_version: SCHEMA_VERSION,
        app_version: APP_VERSION.to_string(),
        state,
        queued_actions: vec![],
    })
    .expect("serialize current-schema save");

    let loaded = SaveFile::from_json(&json)?;
    assert_eq!(
        loaded.state.habitat_registry,
        compute_habitat_registry(&loaded.state)
    );

    Ok(())
}

#[test]
fn save_load_with_source_water_catalog() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(42));
    state
        .source_water_catalog
        .insert("ro_like".to_string(), SourceWaterProfile::zero());
    state.source_water_catalog.insert(
        "custom".to_string(),
        SourceWaterProfile {
            temperature_c: 25.0,
            ammonia_mg_n_per_l: 0.0,
            nitrite_mg_n_per_l: 0.0,
            nitrate_mg_n_per_l: 10.0,
            phosphate_mg_p_per_l: 0.5,
            dic_mg_c_per_l: 15.0,
            doc_mg_c_per_l: 0.0,
            don_mg_n_per_l: 0.0,
            alkalinity_meq_per_l: 2.0,
            calcium_mg_per_l: 30.0,
            magnesium_mg_per_l: 8.0,
            sodium_mg_per_l: 12.0,
            potassium_mg_per_l: 3.0,
            bicarbonate_mg_per_l: 80.0,
            chloride_mg_per_l: 15.0,
            sulfate_mg_per_l: 10.0,
        },
    );
    state.process_params = ProcessParams {
        mineralization_rate_per_day: 0.2,
        nitrification_vmax: 0.1,
        reaeration_kla_base: 0.5,
        aeration_kla_boost: 1.1,
        background_bod_mg_o2_per_g_biomass_per_hour: 0.07,
        plant_photosynthesis_o2_mg_per_g_per_hour: 0.22,
        respiration_dic_rate_mg_c_per_g_per_hour: 0.09,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.14,
        k_surface_w_per_m2_k: 12.0,
        k_wall_w_per_m2_k: 6.0,
        ..ProcessParams::default()
    };

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let save = SaveFile::from_engine(&engine);
    let json = save.to_json_pretty()?;
    let restored = SaveFile::from_json(&json)?;
    let restored_engine = restored.into_engine()?;

    // Source water catalog and process params must survive roundtrip
    assert_eq!(
        engine.full_state().source_water_catalog,
        restored_engine.full_state().source_water_catalog
    );
    assert_eq!(
        engine.full_state().process_params,
        restored_engine.full_state().process_params
    );
    assert_eq!(
        engine.full_state().hardware.heater.last_output_w,
        restored_engine.full_state().hardware.heater.last_output_w
    );
    assert_eq!(engine.full_state(), restored_engine.full_state());

    Ok(())
}

#[test]
fn save_load_resume_determinism() -> Result<(), tank_core::SimError> {
    // Run 240 hours, save, load, run 480 more vs uninterrupted 720
    let mut engine_continuous = Engine::new(SimSeed(77));
    engine_continuous.step_hours(720)?;

    let mut engine_split = Engine::new(SimSeed(77));
    engine_split.step_hours(240)?;

    let save = SaveFile::from_engine(&engine_split);
    let json = save.to_json_pretty()?;
    let restored = SaveFile::from_json(&json)?;
    let mut engine_resumed = restored.into_engine()?;
    engine_resumed.step_hours(480)?;

    assert_eq!(
        engine_continuous.full_state(),
        engine_resumed.full_state(),
        "Save/load/resume must produce identical state to uninterrupted run"
    );

    Ok(())
}

#[test]
fn save_load_with_active_cycle_state() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(78));
    // Set up active nitrogen cycle state
    state.microbe = MicrobeState {
        decomposer_biomass_g: 0.2,
        ammonia_oxidizer_biomass_g: 0.15,
        nitrite_oxidizer_biomass_g: 0.12,
        comammox_biomass_g: 0.03,
        maturity_index: 0.5,
    };
    state.filter_state.biofilter_maturity_index = 0.5;

    let mut engine = Engine::from_parts(state, vec![]);

    // Feed and run to generate active cycle state
    engine.apply_action(PlayerAction::Feed { grams: 1.0 })?;
    engine.step_hours(48)?;

    // Save, load, continue — compare with uninterrupted
    let save = SaveFile::from_engine(&engine);
    let json = save.to_json_pretty()?;
    let restored = SaveFile::from_json(&json)?;
    let mut resumed = restored.into_engine()?;

    let mut continued = engine;
    continued.step_hours(48)?;
    resumed.step_hours(48)?;

    assert_eq!(
        continued.full_state(),
        resumed.full_state(),
        "Save/load with active cycle state must produce identical results"
    );

    Ok(())
}

#[test]
fn legacy_schema_v2_saves_are_migrated_to_net_water_volume() -> Result<(), tank_core::SimError> {
    let mut legacy_state = TankState::new(SimSeed(99));
    let gross_volume_l = legacy_state.geometry.gross_water_volume_l();
    let net_volume_l = legacy_state.water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.water.ammonia_total_mg_n_total = 1.5 * gross_volume_l;
    legacy_state.water.nitrate_mg_n_total = 3.0 * gross_volume_l;
    legacy_state.water.phosphate_mg_p_total = 0.4 * gross_volume_l;
    let state_json = legacy_pre_stage_state_json(&legacy_state, 0, 0, None, 0.8, 0.0);

    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert!((migrated.state.water_volume_l() - net_volume_l).abs() < 1e-9);
    assert!((migrated.state.tan_mg_n_per_l() - 1.5).abs() < 1e-9);
    assert!((migrated.state.nitrate_mg_n_per_l() - 3.0).abs() < 1e-9);
    assert!((migrated.state.phosphate_mg_p_per_l() - 0.4).abs() < 1e-9);
    assert!((migrated.state.water.do_mg_per_l(net_volume_l) - 8.0).abs() < 1e-9);

    Ok(())
}

#[test]
fn legacy_schema_v2_saves_without_tracker_reseed_stability_baselines(
) -> Result<(), tank_core::SimError> {
    let mut legacy_state = TankState::new(SimSeed(100));
    let gross_volume_l = legacy_state.geometry.gross_water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.water.temperature_c = 26.5;
    legacy_state.water.ph = 6.6;
    legacy_state.water.dissolved_oxygen_mg_total = 6.2 * gross_volume_l;
    legacy_state.water.alkalinity_meq_total = 2.3 * gross_volume_l;
    legacy_state.water.calcium_mg_total = 34.0 * gross_volume_l;
    legacy_state.water.magnesium_mg_total = 9.0 * gross_volume_l;

    let mut state_json = legacy_pre_stage_state_json(&legacy_state, 0, 0, None, 0.8, 0.0);
    state_json
        .as_object_mut()
        .expect("state json object")
        .remove("stability_tracker");

    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;

    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert!(
        (migrated.state.stability_tracker.prev_temp_c - migrated.state.water.temperature_c).abs()
            < 1e-9
    );
    assert!((migrated.state.stability_tracker.prev_ph - migrated.state.water.ph).abs() < 1e-9);
    assert!(
        (migrated.state.stability_tracker.prev_do_mg_l - migrated.state.do_mg_per_l()).abs() < 1e-9
    );
    assert!((migrated.state.stability_tracker.prev_gh_d - migrated.state.gh_d()).abs() < 1e-9);
    assert_eq!(migrated.state.stability_tracker.instability_index, 0.0);

    Ok(())
}

#[test]
fn legacy_schema_v2_saves_with_unseeded_tracker_reseed_stability_baselines(
) -> Result<(), tank_core::SimError> {
    let mut legacy_state = TankState::new(SimSeed(108));
    let gross_volume_l = legacy_state.geometry.gross_water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.stability_tracker = StabilityTracker::default();
    let state_json = legacy_pre_stage_state_json(&legacy_state, 0, 0, None, 0.8, 0.0);

    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;

    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert!(
        (migrated.state.stability_tracker.prev_temp_c - migrated.state.water.temperature_c).abs()
            < 1e-9
    );
    assert!((migrated.state.stability_tracker.prev_ph - migrated.state.water.ph).abs() < 1e-9);
    assert!(
        (migrated.state.stability_tracker.prev_do_mg_l - migrated.state.do_mg_per_l()).abs() < 1e-9
    );
    assert!((migrated.state.stability_tracker.prev_gh_d - migrated.state.gh_d()).abs() < 1e-9);
    assert_eq!(migrated.state.stability_tracker.instability_index, 0.0);

    let mut loaded_state = migrated.state.clone();
    update_stability_tracker(&mut loaded_state);
    assert!(
        loaded_state.stability_tracker.instability_index.abs() < 1e-12,
        "reseeded legacy tracker should not register a fake first-day swing"
    );

    Ok(())
}

#[test]
fn legacy_schema_v3_saves_seed_reserve_from_existing_shrimp_biomass() -> Result<(), SimError> {
    let legacy_state = TankState::new(SimSeed(103));
    let mut state_json = legacy_pre_stage_state_json(&legacy_state, 30, 20, None, 0.8, 0.0);
    state_json
        .as_object_mut()
        .expect("state json object")
        .get_mut("animal")
        .and_then(serde_json::Value::as_object_mut)
        .expect("animal object")
        .remove("reserve_g");

    let process_obj = state_json
        .as_object_mut()
        .expect("state json object")
        .get_mut("process_params")
        .and_then(serde_json::Value::as_object_mut)
        .expect("process params object");
    for field in [
        "shrimp_assimilation_efficiency",
        "shrimp_respiration_fraction_of_assimilated",
        "shrimp_excretion_fraction_of_assimilated",
        "shrimp_growth_fraction_of_assimilated",
        "shrimp_o2_per_mg_c_respired",
    ] {
        process_obj.remove(field);
    }

    let json = serde_json::json!({
        "schema_version": 3,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;
    let defaults = ProcessParams::default();
    let expected_reserve_g = shrimp_biomass_g(30, 0, 20) * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert!(
        (migrated.state.animal.total_reserve_g() - expected_reserve_g).abs() < 1e-12,
        "legacy reserve should be reconstructed from serialized shrimp biomass"
    );
    assert_eq!(
        migrated.state.process_params.shrimp_assimilation_efficiency,
        defaults.shrimp_assimilation_efficiency
    );
    assert_eq!(
        migrated
            .state
            .process_params
            .shrimp_respiration_fraction_of_assimilated,
        defaults.shrimp_respiration_fraction_of_assimilated
    );
    assert_eq!(
        migrated
            .state
            .process_params
            .shrimp_excretion_fraction_of_assimilated,
        defaults.shrimp_excretion_fraction_of_assimilated
    );
    assert_eq!(
        migrated
            .state
            .process_params
            .shrimp_growth_fraction_of_assimilated,
        defaults.shrimp_growth_fraction_of_assimilated
    );
    assert_eq!(
        migrated.state.process_params.shrimp_o2_per_mg_c_respired,
        defaults.shrimp_o2_per_mg_c_respired
    );

    let _engine = migrated.into_engine()?;
    Ok(())
}

#[test]
fn legacy_schema_v4_saves_preserve_trim_plants_as_leave_cuttings() -> Result<(), SimError> {
    let mut legacy_state = TankState::new(SimSeed(110));
    legacy_state.animal.berried_females_count = 1;
    legacy_state.animal.molt_stress_index = 0.25;
    legacy_state.animal.reproductive_readiness_index = 0.6;
    legacy_state.animal.egg_progress_days = 4.0;

    let mut state_json = serde_json::to_value(&legacy_state).expect("serialize legacy state");
    let state_obj = state_json.as_object_mut().expect("state json object");
    state_obj.remove("habitat_registry");
    state_obj
        .get_mut("geometry")
        .and_then(serde_json::Value::as_object_mut)
        .expect("geometry object")
        .remove("hardscape_area_cm2");
    state_obj
        .get_mut("hardware")
        .and_then(serde_json::Value::as_object_mut)
        .expect("hardware object")
        .get_mut("filter")
        .and_then(serde_json::Value::as_object_mut)
        .expect("filter object")
        .remove("media_area_cm2");
    state_obj.remove("shrimp_params");
    state_obj.insert(
        "animal".to_string(),
        serde_json::json!({
            "adults_count": 4,
            "juveniles_count": 7,
            "condition_index": 0.65,
            "reserve_g": 1.2,
            "maturation_accum": 0.35,
            "berried_females_count": legacy_state.animal.berried_females_count,
            "molt_stress_index": legacy_state.animal.molt_stress_index,
            "reproductive_readiness_index": legacy_state.animal.reproductive_readiness_index,
            "egg_progress_days": legacy_state.animal.egg_progress_days,
        }),
    );

    let json = serde_json::json!({
        "schema_version": 4,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [
            { "TrimPlants": { "fraction": 0.3 } },
            { "Feed": { "grams": 1.0 } },
            { "TrimPlants": { "fraction": 0.5 } },
        ],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert_eq!(migrated.state.animal.adult.count, 4);
    assert_eq!(migrated.state.animal.juvenile.count, 7);
    assert_eq!(migrated.state.animal.sub_adult.count, 0);
    assert_eq!(migrated.state.animal.berried_females_count, 1);
    assert!(!migrated.state.habitat_registry.is_empty());
    assert_eq!(
        migrated.queued_actions,
        vec![
            PlayerAction::TrimPlantsAndLeaveCuttings { fraction: 0.3 },
            PlayerAction::Feed { grams: 1.0 },
            PlayerAction::TrimPlantsAndLeaveCuttings { fraction: 0.5 },
        ]
    );

    let engine = migrated.into_engine()?;
    let actions = engine.queued_actions();
    assert_eq!(actions.len(), 3);
    assert_eq!(
        actions[0],
        PlayerAction::TrimPlantsAndLeaveCuttings { fraction: 0.3 }
    );
    assert_eq!(actions[1], PlayerAction::Feed { grams: 1.0 });
    assert_eq!(
        actions[2],
        PlayerAction::TrimPlantsAndLeaveCuttings { fraction: 0.5 }
    );

    Ok(())
}

#[test]
fn legacy_schema_v4_saves_split_legacy_maturation_days_into_stage_params() -> Result<(), SimError> {
    let mut legacy_state = TankState::new(SimSeed(111));
    legacy_state.process_params.shrimp_juvenile_maturation_days = 75.0;
    let state_json = legacy_pre_stage_state_json(&legacy_state, 4, 7, Some(1.2), 0.65, 0.35);

    let json = serde_json::json!({
        "schema_version": 4,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let migrated = SaveFile::from_json(&json)?;
    let (expected_juvenile, expected_subadult) =
        ShrimpRuntimeParams::split_legacy_total_maturation_days(75.0);

    assert!(
        (migrated.state.shrimp_params.juvenile_to_subadult_days - expected_juvenile).abs() < 1e-9
    );
    assert!((migrated.state.shrimp_params.subadult_to_adult_days - expected_subadult).abs() < 1e-9);

    let _engine = migrated.into_engine()?;
    Ok(())
}

fn assert_schema_v2_migration_failure(state_json: serde_json::Value, expected_message: &str) {
    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let err = SaveFile::from_json(&json).unwrap_err();
    match err {
        SimError::SchemaMigration { from, to, message } => {
            assert_eq!(from, 2);
            assert_eq!(to, 3);
            assert!(
                message.contains(expected_message),
                "error should mention `{expected_message}`: {message}"
            );
        }
        other => panic!("expected SchemaMigration, got: {other:?}"),
    }
}

#[test]
fn malformed_legacy_schema_v2_save_reports_migration_failure() {
    let mut state_json =
        serde_json::to_value(TankState::new(SimSeed(104))).expect("serialize legacy state");
    state_json
        .as_object_mut()
        .expect("state json object")
        .remove("water");

    assert_schema_v2_migration_failure(state_json, "/state/water");
}

#[test]
fn malformed_legacy_schema_v2_negative_fill_height_reports_migration_failure() {
    let mut state_json =
        serde_json::to_value(TankState::new(SimSeed(105))).expect("serialize legacy state");
    *state_json
        .pointer_mut("/geometry/fill_height_cm")
        .expect("fill height path") = serde_json::json!(-1.0);

    assert_schema_v2_migration_failure(state_json, "/state/geometry/fill_height_cm");
}

#[test]
fn malformed_legacy_schema_v2_negative_substrate_depth_reports_migration_failure() {
    let mut state_json =
        serde_json::to_value(TankState::new(SimSeed(106))).expect("serialize legacy state");
    *state_json
        .pointer_mut("/substrate_layers/0/depth_cm")
        .expect("substrate depth path") = serde_json::json!(-0.5);

    assert_schema_v2_migration_failure(state_json, "/state/substrate_layers/0/depth_cm");
}

#[test]
fn malformed_legacy_schema_v2_substrate_depth_exceeds_fill_height_reports_migration_failure() {
    let legacy_state = TankState::new(SimSeed(107));
    let mut state_json = serde_json::to_value(&legacy_state).expect("serialize legacy state");
    *state_json
        .pointer_mut("/substrate_layers/0/depth_cm")
        .expect("substrate depth path") =
        serde_json::json!(legacy_state.geometry.fill_height_cm + 1.0);

    assert_schema_v2_migration_failure(state_json, "exceeds fill height");
}

#[test]
fn future_schema_version_produces_clear_error() {
    let state = TankState::new(SimSeed(1));
    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION + 1,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let err = SaveFile::from_json(&json).unwrap_err();
    match err {
        SimError::SchemaVersionTooNew {
            actual,
            max_supported,
        } => {
            assert_eq!(actual, SCHEMA_VERSION + 1);
            assert_eq!(max_supported, SCHEMA_VERSION);
        }
        other => panic!("expected SchemaVersionTooNew, got: {other}"),
    }

    // Verify the error message is human-readable (not silent data loss).
    let msg = err.to_string();
    assert!(msg.contains("newer"), "error should mention 'newer': {msg}");
    assert!(
        msg.contains("upgrade"),
        "error should suggest upgrade: {msg}"
    );
}

#[test]
fn very_old_schema_version_produces_clear_error() {
    let state = TankState::new(SimSeed(1));
    let json = serde_json::json!({
        "schema_version": 1,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let err = SaveFile::from_json(&json).unwrap_err();
    match err {
        SimError::SchemaVersionTooOld {
            actual,
            min_supported,
        } => {
            assert_eq!(actual, 1);
            assert_eq!(min_supported, 2);
        }
        other => panic!("expected SchemaVersionTooOld, got: {other}"),
    }

    let msg = err.to_string();
    assert!(
        msg.contains("too old"),
        "error should mention 'too old': {msg}"
    );
    assert!(
        msg.contains("re-create this save"),
        "error should suggest re-creating the save: {msg}"
    );
}

#[test]
fn zero_schema_version_produces_clear_error() {
    // Covers malformed saves with missing or zero schema_version.
    let state = TankState::new(SimSeed(1));
    let json = serde_json::json!({
        "schema_version": 0,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let err = SaveFile::from_json(&json).unwrap_err();
    assert!(matches!(err, SimError::SchemaVersionTooOld { .. }));
}

#[test]
fn migration_chain_applies_v2_to_current() -> Result<(), SimError> {
    // Build a v2 save and verify the full migration chain lands at current schema.
    let mut legacy_state = TankState::new(SimSeed(50));
    let gross_volume_l = legacy_state.geometry.gross_water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.water.ammonia_total_mg_n_total = 2.0 * gross_volume_l;
    let state_json = legacy_pre_stage_state_json(&legacy_state, 0, 0, None, 0.8, 0.0);

    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": state_json,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    assert_eq!(loaded.schema_version, SCHEMA_VERSION);
    // After migration, concentration should be preserved at 2.0 mg/L.
    assert!((loaded.state.tan_mg_n_per_l() - 2.0).abs() < 1e-9);

    // The loaded save should produce a working engine.
    let _engine = loaded.into_engine()?;
    Ok(())
}

#[test]
fn malformed_v4_save_with_negative_reserve_is_rejected() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(101));
    state.animal.adult.reserve_g = -0.01;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    let err = loaded.into_engine().unwrap_err();

    assert_eq!(
        err,
        SimError::InvariantViolation {
            field: "animal.adult.reserve_g",
            value: -0.01,
        }
    );

    Ok(())
}

#[test]
fn malformed_v4_save_with_invalid_shrimp_partition_sum_is_rejected() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(102));
    state
        .process_params
        .shrimp_respiration_fraction_of_assimilated = 0.6;
    state
        .process_params
        .shrimp_excretion_fraction_of_assimilated = 0.1;
    state.process_params.shrimp_growth_fraction_of_assimilated = 0.1;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    let err = loaded.into_engine().unwrap_err();

    match err {
        SimError::InvariantViolation { field, value } => {
            assert_eq!(field, "process.shrimp_assimilated_partition_sum");
            assert!(
                (value - 0.8).abs() < 1e-12,
                "unexpected partition sum: {value}"
            );
        }
        other => panic!("expected shrimp partition invariant violation, got: {other:?}"),
    }

    Ok(())
}

#[test]
fn malformed_current_save_with_invalid_shrimp_stage_duration_is_rejected() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(112));
    state.shrimp_params.juvenile_to_subadult_days = 0.0;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    let err = loaded.into_engine().unwrap_err();

    assert_eq!(
        err,
        SimError::InvariantViolation {
            field: "shrimp_params.juvenile_to_subadult_days",
            value: 0.0,
        }
    );

    Ok(())
}

#[test]
fn malformed_current_save_with_inverted_shrimp_thermal_penalty_range_is_rejected(
) -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(113));
    state.shrimp_params.high_temp_repro_penalty_start_c = 33.0;
    state.shrimp_params.high_temp_repro_penalty_full_c = 30.0;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    let err = loaded.into_engine().unwrap_err();

    assert_eq!(
        err,
        SimError::OrderingViolation {
            lower_field: "shrimp_params.high_temp_repro_penalty_start_c",
            lower_value: 33.0,
            upper_field: "shrimp_params.high_temp_repro_penalty_full_c",
            upper_value: 30.0,
        }
    );

    Ok(())
}

#[test]
fn malformed_v4_save_with_out_of_range_death_biomass_fraction_is_rejected() -> Result<(), SimError>
{
    let mut state = TankState::new(SimSeed(104));
    state.process_params.death_biomass_to_detritus_fraction = 2.0;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;
    let err = loaded.into_engine().unwrap_err();

    assert_eq!(
        err,
        SimError::InvariantViolation {
            field: "process.death_biomass_to_detritus_fraction",
            value: 2.0,
        }
    );

    Ok(())
}

#[test]
fn current_schema_load_repairs_stale_carbonate_caches() -> Result<(), SimError> {
    let mut state = TankState::new(SimSeed(103));
    let volume_l = state.water_volume_l();
    let expected = solve_carbonate_equilibrium(
        state.water.dissolved_inorganic_carbon_mg_c_total,
        state.water.alkalinity_meq_total,
        state.water.temperature_c,
        volume_l,
    );

    state.water.ph = 6.05;
    state.water.bicarbonate_mg_total = 1.0;
    state.stability_tracker.prev_ph = 6.05;
    state.stability_tracker.instability_index = 0.42;

    let json = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "app_version": APP_VERSION,
        "state": state,
        "queued_actions": [],
    })
    .to_string();

    let loaded = SaveFile::from_json(&json)?;

    assert!((loaded.state.water.ph - expected.ph).abs() < 1e-9);
    assert!(
        (loaded.state.water.bicarbonate_mg_total - expected.hco3_mmol_per_l * 61.0 * volume_l)
            .abs()
            < 1e-6
    );
    assert!((loaded.state.stability_tracker.prev_ph - expected.ph).abs() < 1e-9);
    assert!(
        (loaded.state.stability_tracker.instability_index - 0.42).abs() < 1e-12,
        "repairing stale caches should not wipe tracker history"
    );

    Ok(())
}
