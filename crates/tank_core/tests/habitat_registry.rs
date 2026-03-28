use tank_core::{
    compute_habitat_registry, find_habitat, Engine, HabitatEntry, HabitatKind, PlantGuild,
    PlantGuildState, PlayerAction, SimSeed, SimulationEngine, SubstrateKind, SubstrateLayerState,
    TankGeometry, TankState, WaterState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_state() -> TankState {
    TankState::new(SimSeed(42))
}

fn bare_state() -> TankState {
    let mut state = TankState::new(SimSeed(42));
    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.refresh_habitat_registry();
    state
}

fn find(registry: &[HabitatEntry], kind: HabitatKind) -> &HabitatEntry {
    find_habitat(registry, kind).unwrap_or_else(|| panic!("missing habitat {kind:?}"))
}

// ---------------------------------------------------------------------------
// 1. Serialization round-trip
// ---------------------------------------------------------------------------

#[test]
fn habitat_kind_serialization_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    for kind in HabitatKind::ALL {
        let json = serde_json::to_string(&kind)?;
        let deserialized: HabitatKind = serde_json::from_str(&json)?;
        assert_eq!(kind, deserialized, "round-trip failed for {kind:?}");
    }
    Ok(())
}

#[test]
fn habitat_entry_serialization_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let entry = HabitatEntry {
        kind: HabitatKind::FilterMedia,
        colonizable_area_cm2: 2000.0,
        flow_exposure: 0.85,
        oxygen_exposure: 0.72,
        light_exposure: 0.03,
    };
    let json = serde_json::to_string(&entry)?;
    let deserialized: HabitatEntry = serde_json::from_str(&json)?;
    assert_eq!(entry, deserialized);
    Ok(())
}

#[test]
fn full_registry_serialization_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let state = default_state();
    let registry = &state.habitat_registry;
    assert_eq!(
        registry.len(),
        5,
        "registry should have one entry per HabitatKind"
    );

    let json = serde_json::to_string(registry)?;
    let deserialized: Vec<HabitatEntry> = serde_json::from_str(&json)?;
    assert_eq!(registry, &deserialized);
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Area computation
// ---------------------------------------------------------------------------

#[test]
fn filter_media_area_matches_hardware_config() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.hardware.filter.enabled = true;
    state.hardware.filter.media_area_cm2 = 3500.0;
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::FilterMedia);
    assert!(
        (entry.colonizable_area_cm2 - 3500.0).abs() < f64::EPSILON,
        "FilterMedia area should match hardware config: got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn filter_media_area_zero_when_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.hardware.filter.enabled = false;
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::FilterMedia);
    assert!(
        entry.colonizable_area_cm2.abs() < f64::EPSILON,
        "FilterMedia area should be 0 when filter disabled: got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn glass_hardscape_area_equals_wall_plus_hardscape() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.geometry.hardscape_area_cm2 = 500.0;
    let expected = state.geometry.wall_area_cm2() + 500.0;
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::GlassHardscape);
    assert!(
        (entry.colonizable_area_cm2 - expected).abs() < 0.01,
        "GlassHardscape area should equal wall + hardscape: expected {expected}, got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn plant_surfaces_area_from_biomass_and_sla() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = bare_state();
    state.plant_guilds = vec![
        PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 10.0,
            health_index: 0.8,
            crowding_index: 0.1,
            habitat_index: 0.8,
            water_column_uptake_bias: None,
            substrate_uptake_bias: None,
        },
        PlantGuildState {
            guild: PlantGuild::RootFeedingRosette,
            biomass_g: 8.0,
            health_index: 0.8,
            crowding_index: 0.1,
            habitat_index: 0.7,
            water_column_uptake_bias: None,
            substrate_uptake_bias: None,
        },
    ];
    // FastStem SLA = 40 cm²/g, RootRosette SLA = 25 cm²/g
    let expected = 10.0 * 40.0 + 8.0 * 25.0; // 400 + 200 = 600
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::PlantSurfaces);
    assert!(
        (entry.colonizable_area_cm2 - expected).abs() < 0.01,
        "PlantSurfaces area should be sum(biomass * SLA): expected {expected}, got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn plant_surfaces_zero_without_plants() -> Result<(), Box<dyn std::error::Error>> {
    let state = bare_state();
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::PlantSurfaces);
    assert!(
        entry.colonizable_area_cm2.abs() < f64::EPSILON,
        "PlantSurfaces area should be 0 without plants: got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn substrate_surface_area_equals_footprint() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.substrate_layers = vec![SubstrateLayerState::default()];
    let expected = state.geometry.footprint_area_cm2();
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateSurface);
    assert!(
        (entry.colonizable_area_cm2 - expected).abs() < 0.01,
        "SubstrateSurface should equal footprint: expected {expected}, got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn substrate_surface_zero_without_positive_depth() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.substrate_layers = vec![SubstrateLayerState {
        depth_cm: 0.0,
        ..SubstrateLayerState::default()
    }];
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateSurface);
    assert!(
        entry.colonizable_area_cm2.abs() < f64::EPSILON,
        "SubstrateSurface should be 0 for zero-depth substrate: got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn substrate_surface_zero_without_substrate() -> Result<(), Box<dyn std::error::Error>> {
    let state = bare_state();
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateSurface);
    assert!(
        entry.colonizable_area_cm2.abs() < f64::EPSILON,
        "SubstrateSurface should be 0 without substrate: got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

#[test]
fn substrate_deep_area_scales_with_depth_and_kind() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = bare_state();
    state.substrate_layers = vec![
        SubstrateLayerState {
            depth_cm: 2.0,
            colonizable_area_cm2: 1.0,
            ..SubstrateLayerState::default()
        },
        SubstrateLayerState {
            kind: SubstrateKind::CoarsePorous,
            depth_cm: 1.5,
            colonizable_area_cm2: 9_999.0,
            ..SubstrateLayerState::default()
        },
        SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 0.0,
            colonizable_area_cm2: 9_999.0,
            ..SubstrateLayerState::default()
        },
    ];
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateDeep);
    let footprint = state.geometry.footprint_area_cm2();
    let expected = footprint * 2.0 * 0.5 + footprint * 1.5 * 0.9;
    assert!(
        (entry.colonizable_area_cm2 - expected).abs() < 0.01,
        "SubstrateDeep area should scale with depth and substrate kind: expected {expected}, got {}",
        entry.colonizable_area_cm2
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Modifier sensitivity
// ---------------------------------------------------------------------------

#[test]
fn higher_flow_increases_flow_exposure() -> Result<(), Box<dyn std::error::Error>> {
    let mut low_flow = default_state();
    low_flow.hardware.filter.flow_lph = 50.0;
    let low_reg = compute_habitat_registry(&low_flow);

    let mut high_flow = default_state();
    high_flow.hardware.filter.flow_lph = 400.0;
    let high_reg = compute_habitat_registry(&high_flow);

    for kind in [HabitatKind::FilterMedia, HabitatKind::GlassHardscape] {
        let low_entry = find(&low_reg, kind);
        let high_entry = find(&high_reg, kind);
        assert!(
            high_entry.flow_exposure > low_entry.flow_exposure,
            "{kind:?} flow_exposure should increase with flow: low={}, high={}",
            low_entry.flow_exposure,
            high_entry.flow_exposure
        );
    }
    Ok(())
}

#[test]
fn aeration_boosts_oxygen_exposure() -> Result<(), Box<dyn std::error::Error>> {
    let mut no_aer = default_state();
    no_aer.hardware.aeration.enabled = false;
    let no_reg = compute_habitat_registry(&no_aer);

    let mut with_aer = default_state();
    with_aer.hardware.aeration.enabled = true;
    with_aer.hardware.aeration.intensity = 0.8;
    let aer_reg = compute_habitat_registry(&with_aer);

    for kind in [
        HabitatKind::FilterMedia,
        HabitatKind::GlassHardscape,
        HabitatKind::PlantSurfaces,
        HabitatKind::SubstrateSurface,
    ] {
        let no_entry = find(&no_reg, kind);
        let aer_entry = find(&aer_reg, kind);
        assert!(
            aer_entry.oxygen_exposure >= no_entry.oxygen_exposure,
            "{kind:?} oxygen should not decrease with aeration: no_aer={}, with_aer={}",
            no_entry.oxygen_exposure,
            aer_entry.oxygen_exposure
        );
    }
    Ok(())
}

#[test]
fn clogging_reduces_filter_oxygen() -> Result<(), Box<dyn std::error::Error>> {
    let mut clean = default_state();
    clean.filter_state.clogging_index = 0.0;
    let clean_reg = compute_habitat_registry(&clean);

    let mut clogged = default_state();
    clogged.filter_state.clogging_index = 0.9;
    let clogged_reg = compute_habitat_registry(&clogged);

    let clean_o2 = find(&clean_reg, HabitatKind::FilterMedia).oxygen_exposure;
    let clogged_o2 = find(&clogged_reg, HabitatKind::FilterMedia).oxygen_exposure;
    assert!(
        clogged_o2 < clean_o2,
        "clogging should reduce FilterMedia oxygen: clean={clean_o2}, clogged={clogged_o2}"
    );
    Ok(())
}

#[test]
fn light_intensity_affects_exposure() -> Result<(), Box<dyn std::error::Error>> {
    let mut dark = default_state();
    dark.hardware.light.enabled = false;
    let dark_reg = compute_habitat_registry(&dark);

    let mut bright = default_state();
    bright.hardware.light.enabled = true;
    bright.hardware.light.intensity_index = 0.9;
    let bright_reg = compute_habitat_registry(&bright);

    for kind in [HabitatKind::GlassHardscape, HabitatKind::PlantSurfaces] {
        let dark_entry = find(&dark_reg, kind);
        let bright_entry = find(&bright_reg, kind);
        assert!(
            bright_entry.light_exposure > dark_entry.light_exposure,
            "{kind:?} light should increase with intensity: dark={}, bright={}",
            dark_entry.light_exposure,
            bright_entry.light_exposure
        );
    }
    Ok(())
}

#[test]
fn rooted_plants_boost_substrate_surface_oxygen() -> Result<(), Box<dyn std::error::Error>> {
    let mut no_roots = default_state();
    no_roots.plant_guilds.clear();
    let no_reg = compute_habitat_registry(&no_roots);

    let mut with_roots = default_state();
    with_roots.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::RootFeedingRosette,
        biomass_g: 5.0,
        health_index: 0.8,
        crowding_index: 0.1,
        habitat_index: 0.7,
        water_column_uptake_bias: None,
        substrate_uptake_bias: None,
    }];
    let root_reg = compute_habitat_registry(&with_roots);

    let no_o2 = find(&no_reg, HabitatKind::SubstrateSurface).oxygen_exposure;
    let root_o2 = find(&root_reg, HabitatKind::SubstrateSurface).oxygen_exposure;
    assert!(
        root_o2 > no_o2,
        "rooted plants should boost SubstrateSurface O2: without={no_o2}, with={root_o2}"
    );
    Ok(())
}

#[test]
fn plant_crowding_reduces_light_exposure() -> Result<(), Box<dyn std::error::Error>> {
    let mut sparse = default_state();
    for plant in &mut sparse.plant_guilds {
        plant.biomass_g = 2.0;
        plant.crowding_index = 0.0;
    }
    let mut dense = default_state();
    for plant in &mut dense.plant_guilds {
        plant.biomass_g = 30.0;
        plant.crowding_index = 0.0;
    }
    dense.hardware.light.enabled = true;
    dense.hardware.light.intensity_index = 0.8;
    sparse.hardware.light.enabled = true;
    sparse.hardware.light.intensity_index = 0.8;
    let sparse_reg = compute_habitat_registry(&sparse);
    let dense_reg = compute_habitat_registry(&dense);

    let sparse_light = find(&sparse_reg, HabitatKind::GlassHardscape).light_exposure;
    let dense_light = find(&dense_reg, HabitatKind::GlassHardscape).light_exposure;
    assert!(
        dense_light < sparse_light,
        "crowding should reduce GlassHardscape light: sparse={sparse_light}, dense={dense_light}"
    );
    Ok(())
}

#[test]
fn substrate_surface_light_uses_water_above_substrate() -> Result<(), Box<dyn std::error::Error>> {
    let mut shallow_bed = bare_state();
    shallow_bed.hardware.light.enabled = true;
    shallow_bed.hardware.light.intensity_index = 1.0;
    shallow_bed.substrate_layers = vec![SubstrateLayerState {
        depth_cm: 2.0,
        ..SubstrateLayerState::default()
    }];

    let mut deep_bed = shallow_bed.clone();
    deep_bed.substrate_layers[0].depth_cm = 8.0;

    let shallow_reg = compute_habitat_registry(&shallow_bed);
    let deep_reg = compute_habitat_registry(&deep_bed);

    let shallow_light = find(&shallow_reg, HabitatKind::SubstrateSurface).light_exposure;
    let deep_light = find(&deep_reg, HabitatKind::SubstrateSurface).light_exposure;
    assert!(
        deep_light > shallow_light,
        "shallower water above the substrate should increase substrate-surface light: shallow={shallow_light}, deep={deep_light}"
    );
    Ok(())
}

#[test]
fn substrate_deep_always_dark() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 1.0;
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateDeep);
    assert!(
        entry.light_exposure.abs() < f64::EPSILON,
        "SubstrateDeep should have zero light: got {}",
        entry.light_exposure
    );
    Ok(())
}

#[test]
fn substrate_deep_oxygen_capped() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;
    state.hardware.filter.flow_lph = 1000.0;
    // Set all substrate layers to have zero low-oxygen tendency (best case)
    for layer in &mut state.substrate_layers {
        layer.low_oxygen_tendency_index = 0.0;
    }
    let registry = compute_habitat_registry(&state);
    let entry = find(&registry, HabitatKind::SubstrateDeep);
    assert!(
        entry.oxygen_exposure <= 0.2,
        "SubstrateDeep oxygen should be capped at 0.2: got {}",
        entry.oxygen_exposure
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Bounds validation
// ---------------------------------------------------------------------------

#[test]
fn all_modifiers_bounded_under_extreme_inputs() -> Result<(), Box<dyn std::error::Error>> {
    type Scenario = (&'static str, Box<dyn Fn(&mut TankState)>);
    let scenarios: Vec<Scenario> = vec![
        (
            "max_everything",
            Box::new(|s: &mut TankState| {
                s.hardware.filter.enabled = true;
                s.hardware.filter.flow_lph = 10000.0;
                s.hardware.filter.media_area_cm2 = 50000.0;
                s.hardware.aeration.enabled = true;
                s.hardware.aeration.intensity = 1.0;
                s.hardware.light.enabled = true;
                s.hardware.light.intensity_index = 1.0;
                s.filter_state.clogging_index = 0.0;
                s.geometry.hardscape_area_cm2 = 10000.0;
                for p in &mut s.plant_guilds {
                    p.biomass_g = 100.0;
                    p.crowding_index = 1.0;
                }
            }),
        ),
        (
            "min_everything",
            Box::new(|s: &mut TankState| {
                s.hardware.filter.enabled = false;
                s.hardware.aeration.enabled = false;
                s.hardware.light.enabled = false;
                s.filter_state.clogging_index = 1.0;
                s.plant_guilds.clear();
                s.substrate_layers.clear();
                s.geometry.hardscape_area_cm2 = 0.0;
            }),
        ),
        (
            "max_clogging_with_filter",
            Box::new(|s: &mut TankState| {
                s.hardware.filter.enabled = true;
                s.filter_state.clogging_index = 1.0;
            }),
        ),
        (
            "huge_tank",
            Box::new(|s: &mut TankState| {
                s.geometry = TankGeometry {
                    length_cm: 300.0,
                    width_cm: 100.0,
                    height_cm: 80.0,
                    fill_height_cm: 70.0,
                    glass_thickness_mm: 12.0,
                    open_top: false,
                    lid_exchange_factor: 0.1,
                    hardscape_area_cm2: 5000.0,
                };
                s.water = WaterState::default_for_volume_l(s.water_volume_l());
            }),
        ),
    ];

    for (name, mutator) in &scenarios {
        let mut state = default_state();
        mutator(&mut state);
        let registry = compute_habitat_registry(&state);

        assert_eq!(
            registry.len(),
            5,
            "scenario '{name}': registry should always have 5 entries"
        );

        for entry in &registry {
            assert!(
                entry.colonizable_area_cm2 >= 0.0,
                "scenario '{name}', {:?}: area must be non-negative, got {}",
                entry.kind,
                entry.colonizable_area_cm2
            );
            assert!(
                (0.0..=1.0).contains(&entry.flow_exposure),
                "scenario '{name}', {:?}: flow must be in [0,1], got {}",
                entry.kind,
                entry.flow_exposure
            );
            assert!(
                (0.0..=1.0).contains(&entry.oxygen_exposure),
                "scenario '{name}', {:?}: oxygen must be in [0,1], got {}",
                entry.kind,
                entry.oxygen_exposure
            );
            assert!(
                (0.0..=1.0).contains(&entry.light_exposure),
                "scenario '{name}', {:?}: light must be in [0,1], got {}",
                entry.kind,
                entry.light_exposure
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Plant biomass responsiveness
// ---------------------------------------------------------------------------

#[test]
fn plant_biomass_change_updates_area() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = default_state();
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 5.0,
        health_index: 0.8,
        crowding_index: 0.1,
        habitat_index: 0.8,
        water_column_uptake_bias: None,
        substrate_uptake_bias: None,
    }];
    let reg_before = compute_habitat_registry(&state);
    let area_before = find(&reg_before, HabitatKind::PlantSurfaces).colonizable_area_cm2;

    state.plant_guilds[0].biomass_g = 20.0;
    let reg_after = compute_habitat_registry(&state);
    let area_after = find(&reg_after, HabitatKind::PlantSurfaces).colonizable_area_cm2;

    assert!(
        area_after > area_before,
        "PlantSurfaces area should grow with biomass: before={area_before}, after={area_after}"
    );
    // FastStem SLA = 40 cm²/g: 20g * 40 = 800, 5g * 40 = 200
    assert!(
        (area_after - 800.0).abs() < 0.01,
        "expected 800 cm² for 20g FastStem: got {area_after}"
    );
    assert!(
        (area_before - 200.0).abs() < 0.01,
        "expected 200 cm² for 5g FastStem: got {area_before}"
    );
    Ok(())
}

#[test]
fn engine_from_parts_refreshes_stale_registry() {
    let mut state = default_state();
    let stale_registry = state.habitat_registry.clone();
    state.plant_guilds[0].biomass_g *= 4.0;
    state.habitat_registry = stale_registry;

    let expected = compute_habitat_registry(&state);
    let engine = Engine::from_parts(state, vec![]);

    assert_eq!(engine.full_state().habitat_registry, expected);
}

#[test]
fn trim_action_hourly_step_keeps_stored_registry_current() -> Result<(), tank_core::SimError> {
    let mut state = bare_state();
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 20.0,
        health_index: 0.8,
        crowding_index: 0.1,
        habitat_index: 0.8,
        water_column_uptake_bias: None,
        substrate_uptake_bias: None,
    }];
    state.refresh_habitat_registry();
    let light_before = find(&state.habitat_registry, HabitatKind::GlassHardscape).light_exposure;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::TrimPlantsAndRemove { fraction: 0.5 })?;
    engine.step_hours(1)?;

    assert_eq!(
        engine.full_state().habitat_registry,
        compute_habitat_registry(engine.full_state())
    );
    assert!(
        (engine.full_state().plant_guilds[0].crowding_index - 0.4).abs() < 0.01,
        "trim should refresh the cached crowding index from the new biomass: got {}",
        engine.full_state().plant_guilds[0].crowding_index
    );
    let light_after = find(
        &engine.full_state().habitat_registry,
        HabitatKind::GlassHardscape,
    )
    .light_exposure;
    assert!(
        light_after > light_before,
        "trimming should increase light exposure once crowding is recomputed: before={light_before}, after={light_after}"
    );
    assert!(
        (find(
            &engine.full_state().habitat_registry,
            HabitatKind::PlantSurfaces
        )
        .colonizable_area_cm2
            - 400.0)
            .abs()
            < 0.01
    );

    Ok(())
}

#[test]
fn light_action_hourly_step_keeps_stored_registry_current() -> Result<(), tank_core::SimError> {
    let mut state = default_state();
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.2;
    state.refresh_habitat_registry();
    let light_before = find(&state.habitat_registry, HabitatKind::GlassHardscape).light_exposure;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::ChangeLightIntensity {
        intensity_index: 0.9,
    })?;
    engine.step_hours(1)?;

    assert_eq!(
        engine.full_state().habitat_registry,
        compute_habitat_registry(engine.full_state())
    );
    let light_after = find(
        &engine.full_state().habitat_registry,
        HabitatKind::GlassHardscape,
    )
    .light_exposure;
    assert!(
        light_after > light_before,
        "changing light intensity should refresh habitat light exposure: before={light_before}, after={light_after}"
    );

    Ok(())
}

#[test]
fn daily_update_keeps_stored_registry_current() -> Result<(), tank_core::SimError> {
    let mut state = default_state();
    state.hardware.filter.enabled = true;
    state.hardware.filter.cleanliness_index = 1.0;
    state.detritus.fine_detritus_g_total = 3.0;
    state.filter_state.clogging_index = 0.0;
    state.refresh_habitat_registry();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    assert!(engine.full_state().filter_state.clogging_index > 0.0);
    assert_eq!(
        engine.full_state().habitat_registry,
        compute_habitat_registry(engine.full_state())
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Registry structure invariants
// ---------------------------------------------------------------------------

#[test]
fn registry_covers_all_habitat_kinds() -> Result<(), Box<dyn std::error::Error>> {
    let state = default_state();
    let registry = compute_habitat_registry(&state);
    for kind in HabitatKind::ALL {
        assert!(
            find_habitat(&registry, kind).is_some(),
            "registry must contain {kind:?}"
        );
    }
    Ok(())
}

#[test]
fn find_habitat_returns_none_for_missing() -> Result<(), Box<dyn std::error::Error>> {
    let empty: Vec<HabitatEntry> = vec![];
    assert!(find_habitat(&empty, HabitatKind::FilterMedia).is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Geometry scaling
// ---------------------------------------------------------------------------

#[test]
fn larger_tank_produces_larger_wall_and_substrate_areas() -> Result<(), Box<dyn std::error::Error>>
{
    let small = default_state(); // 40×25 cm
    let small_reg = compute_habitat_registry(&small);

    let mut large = default_state();
    large.geometry = TankGeometry {
        length_cm: 120.0,
        width_cm: 50.0,
        height_cm: 50.0,
        fill_height_cm: 45.0,
        ..TankGeometry::default()
    };
    large.water = WaterState::default_for_volume_l(large.water_volume_l());
    let large_reg = compute_habitat_registry(&large);

    let small_glass = find(&small_reg, HabitatKind::GlassHardscape).colonizable_area_cm2;
    let large_glass = find(&large_reg, HabitatKind::GlassHardscape).colonizable_area_cm2;
    assert!(
        large_glass > small_glass,
        "larger tank should have more glass area: small={small_glass}, large={large_glass}"
    );

    let small_sub = find(&small_reg, HabitatKind::SubstrateSurface).colonizable_area_cm2;
    let large_sub = find(&large_reg, HabitatKind::SubstrateSurface).colonizable_area_cm2;
    assert!(
        large_sub > small_sub,
        "larger tank should have more substrate surface: small={small_sub}, large={large_sub}"
    );
    Ok(())
}
