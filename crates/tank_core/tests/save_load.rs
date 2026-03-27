use tank_core::{
    Engine, MicrobeState, PlayerAction, ProcessParams, SaveFile, SimSeed, SimulationEngine,
    SourceWaterProfile, TankState, WaterState, APP_VERSION, SCHEMA_VERSION,
};

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
    let gross_volume_l = legacy_state.geometry.water_volume_l();
    let net_volume_l = legacy_state.water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.water.ammonia_total_mg_n_total = 1.5 * gross_volume_l;
    legacy_state.water.nitrate_mg_n_total = 3.0 * gross_volume_l;
    legacy_state.water.phosphate_mg_p_total = 0.4 * gross_volume_l;

    let json = serde_json::json!({
        "schema_version": 2,
        "app_version": APP_VERSION,
        "state": legacy_state,
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
    let gross_volume_l = legacy_state.geometry.water_volume_l();
    legacy_state.water = WaterState::default_for_volume_l(gross_volume_l);
    legacy_state.water.temperature_c = 26.5;
    legacy_state.water.ph = 6.6;
    legacy_state.water.dissolved_oxygen_mg_total = 6.2 * gross_volume_l;
    legacy_state.water.alkalinity_meq_total = 2.3 * gross_volume_l;
    legacy_state.water.calcium_mg_total = 34.0 * gross_volume_l;
    legacy_state.water.magnesium_mg_total = 9.0 * gross_volume_l;

    let mut state_json = serde_json::to_value(&legacy_state).expect("serialize legacy state");
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
