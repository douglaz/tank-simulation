use tank_core::{compute_habitat_registry, SimSeed};

#[test]
fn nano_cycle_materializes_correctly() {
    let state = tank_scenarios::seeded_state(SimSeed(42), "nano_cycle")
        .expect("nano_cycle should materialize");

    // Geometry from scenario: 30×20, 20cm tall, 18cm filled
    assert!((state.geometry.length_cm - 30.0).abs() < f64::EPSILON);
    assert!((state.geometry.width_cm - 20.0).abs() < f64::EPSILON);
    assert!((state.geometry.height_cm - 20.0).abs() < f64::EPSILON);
    assert!((state.geometry.fill_height_cm - 18.0).abs() < f64::EPSILON);

    // Volume: 30 * 20 * 18 / 1000 = 10.8L
    let expected_vol = 30.0 * 20.0 * 18.0 / 1000.0;
    let actual_vol = state.geometry.gross_water_volume_l();
    assert!(
        (actual_vol - expected_vol).abs() < 0.01,
        "Volume should be {expected_vol}, got {actual_vol}"
    );

    // Ambient from scenario
    assert!((state.environment.ambient_temp_c - 24.0).abs() < f64::EPSILON);

    // Meta
    assert_eq!(state.meta.scenario_id.as_deref(), Some("nano_cycle"));
    assert_eq!(state.meta.notes.as_deref(), Some("Nano Cycle"));

    // Source water catalog should contain all presets
    assert!(state.source_water_catalog.contains_key("soft_acidic"));
    assert!(state.source_water_catalog.contains_key("moderate"));
    assert!(state.source_water_catalog.contains_key("hard_shrimp"));
    assert!(state.source_water_catalog.contains_key("ro_like"));

    // Initial water chemistry comes from soft_acidic profile
    let sw = &state.source_water_catalog["soft_acidic"];
    let vol = state.water_volume_l();
    assert!(
        (state.water.calcium_mg_total - sw.calcium_mg_per_l * vol).abs() < 0.01,
        "Calcium should match source water * volume"
    );
    assert!(
        (state.water.temperature_c - sw.temperature_c).abs() < 0.01,
        "Initial water temp should match source water"
    );

    // Substrate: single inert_sand layer
    assert_eq!(state.substrate_layers.len(), 1);
    assert_eq!(
        state.substrate_layers[0].kind,
        tank_core::SubstrateKind::InertSand
    );

    // Plants: single fast_stem
    assert_eq!(state.plant_guilds.len(), 1);
    assert_eq!(state.plant_guilds[0].guild, tank_core::PlantGuild::FastStem);

    // Process params should be non-default (loaded from preset)
    assert!(state.process_params.k_surface_w_per_m2_k > 0.0);
    assert!(state.process_params.k_wall_w_per_m2_k > 0.0);
}

#[test]
fn medium_planted_materializes_with_two_substrates_and_plants() {
    let state = tank_scenarios::seeded_state(SimSeed(43), "medium_planted")
        .expect("medium_planted should materialize");

    // Two substrate layers
    assert_eq!(state.substrate_layers.len(), 2);
    assert_eq!(
        state.substrate_layers[0].kind,
        tank_core::SubstrateKind::ActivePlanted
    );
    assert_eq!(
        state.substrate_layers[1].kind,
        tank_core::SubstrateKind::CoarsePorous
    );

    // Two plant guilds
    assert_eq!(state.plant_guilds.len(), 2);

    // Geometry: 60×30, 36cm height, 32cm filled
    assert!((state.geometry.length_cm - 60.0).abs() < f64::EPSILON);
    assert!((state.geometry.fill_height_cm - 32.0).abs() < f64::EPSILON);

    // Volume
    let expected_vol = 60.0 * 30.0 * 32.0 / 1000.0;
    let actual_vol = state.geometry.gross_water_volume_l();
    assert!(
        (actual_vol - expected_vol).abs() < 0.01,
        "Expected {expected_vol}L, got {actual_vol}L"
    );
}

#[test]
fn warm_room_materializes_with_high_ambient() {
    let state = tank_scenarios::seeded_state(SimSeed(44), "warm_room")
        .expect("warm_room should materialize");

    assert!((state.environment.ambient_temp_c - 29.0).abs() < f64::EPSILON);
}

#[test]
fn deterministic_materialization() {
    let state_a =
        tank_scenarios::seeded_state(SimSeed(99), "nano_cycle").expect("should materialize");
    let state_b =
        tank_scenarios::seeded_state(SimSeed(99), "nano_cycle").expect("should materialize");

    assert_eq!(
        state_a, state_b,
        "Same seed + scenario must produce identical state"
    );
}

#[test]
fn materialized_state_keeps_habitat_registry_current() {
    let state = tank_scenarios::seeded_state(SimSeed(120), "medium_planted")
        .expect("scenario should materialize");

    assert_eq!(state.habitat_registry, compute_habitat_registry(&state));
}

#[test]
fn geometry_overrides_scale_size_and_fill() {
    let state = tank_scenarios::seeded_state_with_overrides(
        SimSeed(77),
        "nano_cycle",
        tank_scenarios::ScenarioGeometryOverrides {
            size_scale: 1.25,
            fill_ratio: 0.8,
        },
    )
    .expect("overrides should materialize");

    assert!((state.geometry.length_cm - 37.5).abs() < f64::EPSILON);
    assert!((state.geometry.width_cm - 25.0).abs() < f64::EPSILON);
    assert!((state.geometry.height_cm - 25.0).abs() < f64::EPSILON);
    assert!((state.geometry.fill_height_cm - 18.0).abs() < f64::EPSILON);

    let volume_l = state.water_volume_l();
    let profile = &state.source_water_catalog["soft_acidic"];
    assert!(
        (state.water.calcium_mg_total - profile.calcium_mg_per_l * volume_l).abs() < 0.01,
        "water totals should be rebuilt for overridden geometry"
    );
}

#[test]
fn unknown_scenario_returns_error() {
    let result = tank_scenarios::seeded_state(SimSeed(1), "nonexistent_scenario");
    assert!(result.is_err());
}

#[test]
fn cycling_fixtures_are_deterministic_and_differ_in_seeding() {
    let (seeded_a, unseeded_a) = tank_scenarios::cycling_fixture_pair(SimSeed(100));
    let (seeded_b, unseeded_b) = tank_scenarios::cycling_fixture_pair(SimSeed(100));

    // Deterministic: same seed produces identical states
    assert_eq!(seeded_a, seeded_b, "Seeded fixtures must be deterministic");
    assert_eq!(
        unseeded_a, unseeded_b,
        "Unseeded fixtures must be deterministic"
    );

    // They differ only in seeding inputs (microbe state and filter state)
    assert_ne!(
        seeded_a.microbe, unseeded_a.microbe,
        "Seeded and unseeded must have different microbe state"
    );
    assert_ne!(
        seeded_a.filter_state, unseeded_a.filter_state,
        "Seeded and unseeded must have different filter state"
    );

    // But share the same geometry, water, and process params
    assert_eq!(seeded_a.geometry, unseeded_a.geometry);
    assert_eq!(seeded_a.water, unseeded_a.water);
    assert_eq!(seeded_a.process_params, unseeded_a.process_params);
    assert_eq!(seeded_a.environment, unseeded_a.environment);
}
