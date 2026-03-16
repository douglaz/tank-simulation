use tank_core::SimSeed;
use tank_scenarios::{
    seeded_state_with_full_overrides, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};

#[test]
fn startup_overrides_apply_heater_and_initial_shrimp() {
    let state = seeded_state_with_full_overrides(
        SimSeed(501),
        "nano_cycle",
        StartupOverrides {
            geometry: tank_scenarios::ScenarioGeometryOverrides::default(),
            source_water_profile_id: Some("moderate".to_string()),
            substrate_preset: Some(StartupSubstratePreset::InertSand),
            plant_selection: Some(StartupPlantSelection::FastStemOnly),
            filter_enabled: Some(true),
            light_preset: Some(StartupLightPreset::Hours8),
            heater_preset: Some(StartupHeaterPreset::Celsius26),
            aeration_enabled: Some(false),
            initial_adult_shrimp_count: Some(10),
        },
    )
    .expect("startup overrides should materialize");

    assert!(state.hardware.heater.enabled);
    assert!((state.hardware.heater.setpoint_c - 26.0).abs() < f64::EPSILON);
    assert_eq!(state.animal.adults_count, 10);
}

#[test]
fn startup_overrides_replace_source_water_profile() {
    let state = seeded_state_with_full_overrides(
        SimSeed(502),
        "nano_cycle",
        StartupOverrides {
            geometry: tank_scenarios::ScenarioGeometryOverrides::default(),
            source_water_profile_id: Some("ro_like".to_string()),
            substrate_preset: None,
            plant_selection: None,
            filter_enabled: None,
            light_preset: None,
            heater_preset: None,
            aeration_enabled: None,
            initial_adult_shrimp_count: None,
        },
    )
    .expect("source water override should materialize");

    let volume_l = state.geometry.water_volume_l();
    let profile = &state.source_water_catalog["ro_like"];

    assert!((state.water.temperature_c - profile.temperature_c).abs() < 0.001);
    assert!(
        (state.water.alkalinity_meq_total - profile.alkalinity_meq_per_l * volume_l).abs() < 0.001
    );
    assert!((state.water.calcium_mg_total - profile.calcium_mg_per_l * volume_l).abs() < 0.001);
    assert!((state.water.sodium_mg_total - profile.sodium_mg_per_l * volume_l).abs() < 0.001);
}
