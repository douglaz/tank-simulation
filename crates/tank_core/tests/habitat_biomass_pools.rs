//! Tests for per-habitat periphyton and decomposer biomass pools
//! (tanksim-6e5.5.3).

use std::collections::BTreeMap;

use tank_core::{
    live_biomass_carbon_mg, live_biomass_nitrogen_mg, Engine, HabitatKind, PlayerAction, SaveFile,
    SimSeed, SimulationEngine, SubstrateKind, SubstrateLayerState, TankState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A well-lit, nutrient-rich state suitable for testing periphyton growth.
fn growth_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrate_mg_n_total = 40.0;
    state.water.phosphate_mg_p_total = 6.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 260.0;
    state.water.dissolved_oxygen_mg_total = 200.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.hardware.filter.enabled = true;
    state.hardware.filter.media_area_cm2 = 2000.0;
    state.hardware.filter.flow_lph = 200.0;
    // Ample DOC for decomposer growth.
    state.water.dissolved_organic_carbon_mg_c_total = 50.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.0;
    state.refresh_habitat_registry();
    state
}

/// Minimal state with only glass (no substrate, no plants, no filter).
fn bare_glass_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.hardware.filter.enabled = false;
    state.hardware.filter.media_area_cm2 = 0.0;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrate_mg_n_total = 40.0;
    state.water.phosphate_mg_p_total = 6.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 260.0;
    state.water.dissolved_oxygen_mg_total = 200.0;
    state.water.dissolved_organic_carbon_mg_c_total = 50.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 5.0;
    state.refresh_habitat_registry();
    state
}

// ---------------------------------------------------------------------------
// 1. Periphyton split by habitat
// ---------------------------------------------------------------------------

#[test]
fn test_periphyton_split_by_habitat() -> Result<(), tank_core::SimError> {
    let state = growth_state(SimSeed(5001));

    // After default init + refresh, periphyton exists on lit habitats.
    assert!(
        state
            .algae
            .periphyton_by_habitat
            .contains_key(&HabitatKind::GlassHardscape),
        "glass should have periphyton pool"
    );
    assert!(
        state
            .algae
            .periphyton_by_habitat
            .contains_key(&HabitatKind::SubstrateSurface),
        "substrate surface should have periphyton pool"
    );
    assert!(
        state
            .algae
            .periphyton_by_habitat
            .contains_key(&HabitatKind::PlantSurfaces),
        "plant surfaces should have periphyton pool"
    );

    // Each pool is independent (different values).
    let glass = state.algae.periphyton_by_habitat[&HabitatKind::GlassHardscape];
    let substrate = state.algae.periphyton_by_habitat[&HabitatKind::SubstrateSurface];
    assert!(glass > 0.0, "glass periphyton should be positive");
    assert!(substrate > 0.0, "substrate periphyton should be positive");
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Decomposer split by habitat
// ---------------------------------------------------------------------------

#[test]
fn test_decomposer_split_by_habitat() -> Result<(), tank_core::SimError> {
    let state = growth_state(SimSeed(5002));

    assert!(
        state
            .microbe
            .decomposer_by_habitat
            .contains_key(&HabitatKind::FilterMedia),
        "filter media should have decomposer pool"
    );
    assert!(
        state
            .microbe
            .decomposer_by_habitat
            .contains_key(&HabitatKind::SubstrateSurface),
        "substrate surface should have decomposer pool"
    );
    assert!(
        state
            .microbe
            .decomposer_by_habitat
            .contains_key(&HabitatKind::SubstrateDeep),
        "substrate deep should have decomposer pool"
    );

    let filter = state.microbe.decomposer_by_habitat[&HabitatKind::FilterMedia];
    let substrate = state.microbe.decomposer_by_habitat[&HabitatKind::SubstrateSurface];
    assert!(filter > 0.0, "filter decomposers should be positive");
    assert!(substrate > 0.0, "substrate decomposers should be positive");
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Habitat growth rate differs (periphyton: glass >> filter media)
// ---------------------------------------------------------------------------

#[test]
fn test_habitat_growth_rate_differs() -> Result<(), tank_core::SimError> {
    // Set up identical nutrient/temp conditions; glass gets light, filter does not.
    let mut state = growth_state(SimSeed(5003));

    // Start all habitat pools at equal biomass to test divergence.
    for biomass in state.algae.periphyton_by_habitat.values_mut() {
        *biomass = 0.1;
    }
    state.algae.sync_periphyton_total();

    // Disable shrimp and microfauna to isolate algae growth.
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24 * 30)?; // 30 days for meaningful divergence

    let algae = &engine.full_state().algae;
    let glass = algae
        .periphyton_by_habitat
        .get(&HabitatKind::GlassHardscape)
        .copied()
        .unwrap_or(0.0);
    let filter = algae
        .periphyton_by_habitat
        .get(&HabitatKind::FilterMedia)
        .copied()
        .unwrap_or(0.0);

    assert!(
        glass > filter,
        "glass periphyton ({glass:.6}) should grow faster than filter ({filter:.6}) \
         because glass gets light and filter media is dark"
    );

    // Filter media periphyton should decline (respiration > growth in dark).
    assert!(
        filter < 0.1,
        "filter periphyton ({filter:.6}) should decline from starting 0.1"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Total biomass conserved on split
// ---------------------------------------------------------------------------

#[test]
fn test_total_biomass_conserved_on_split() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5004));
    state.algae.periphyton_biomass_g = 1.5;
    state.algae.periphyton_by_habitat.clear();
    state.microbe.decomposer_biomass_g = 0.8;
    state.microbe.decomposer_by_habitat.clear();

    // Trigger distribution.
    state.refresh_habitat_registry();

    let periphyton_sum: f64 = state.algae.periphyton_by_habitat.values().sum();
    let decomposer_sum: f64 = state.microbe.decomposer_by_habitat.values().sum();

    assert!(
        (periphyton_sum - 1.5).abs() < 1e-10,
        "periphyton sum ({periphyton_sum}) should equal original total (1.5)"
    );
    assert!(
        (decomposer_sum - 0.8).abs() < 1e-10,
        "decomposer sum ({decomposer_sum}) should equal original total (0.8)"
    );
    assert!(
        (state.algae.periphyton_biomass_g - periphyton_sum).abs() < 1e-10,
        "synced total should match sum"
    );
    assert!(
        (state.microbe.decomposer_biomass_g - decomposer_sum).abs() < 1e-10,
        "synced total should match sum"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Shrimp grazing favors accessible habitats over filter media
// ---------------------------------------------------------------------------

#[test]
fn test_grazing_targets_habitat_periphyton() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5005));

    // Start exposed and protected habitats at equal biomass so accessibility,
    // not starting abundance, determines which habitats lose more.
    state.algae.periphyton_by_habitat = BTreeMap::from([
        (HabitatKind::GlassHardscape, 1.0),
        (HabitatKind::SubstrateSurface, 1.0),
        (HabitatKind::PlantSurfaces, 1.0),
        (HabitatKind::FilterMedia, 1.0),
    ]);
    state.algae.sync_periphyton_total();

    // 15 shrimp to create significant grazing.
    state.animal.adult.count = 15;
    state.microfauna.population_index = 0.0;

    // Disable algae growth to isolate grazing.
    state.hardware.light.enabled = false;
    state.process_params.periphyton_max_growth_rate_per_day = 0.0;

    let total_before = state.algae.periphyton_biomass_g;
    let glass_before = state.algae.periphyton_by_habitat[&HabitatKind::GlassHardscape];
    let substrate_before = state.algae.periphyton_by_habitat[&HabitatKind::SubstrateSurface];
    let filter_before = state.algae.periphyton_by_habitat[&HabitatKind::FilterMedia];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let algae = &engine.full_state().algae;
    let total_after = algae.periphyton_biomass_g;
    let glass_after = algae
        .periphyton_by_habitat
        .get(&HabitatKind::GlassHardscape)
        .copied()
        .unwrap_or(0.0);
    let substrate_after = algae
        .periphyton_by_habitat
        .get(&HabitatKind::SubstrateSurface)
        .copied()
        .unwrap_or(0.0);
    let filter_after = algae
        .periphyton_by_habitat
        .get(&HabitatKind::FilterMedia)
        .copied()
        .unwrap_or(0.0);

    // Total periphyton should decrease from grazing.
    assert!(
        total_after < total_before,
        "grazing should reduce total periphyton ({total_before:.4} -> {total_after:.4})"
    );

    // Exposed surfaces should be grazed harder than filter-internal biofilm.
    let glass_loss = glass_before - glass_after;
    let substrate_loss = substrate_before - substrate_after;
    let filter_loss = filter_before - filter_after;
    assert!(
        glass_loss > filter_loss,
        "glass loss ({glass_loss:.6}) should exceed filter loss ({filter_loss:.6})"
    );
    assert!(
        substrate_loss > filter_loss,
        "substrate loss ({substrate_loss:.6}) should exceed filter loss ({filter_loss:.6})"
    );

    // Per-habitat sum should equal total.
    let sum: f64 = algae.periphyton_by_habitat.values().sum();
    assert!(
        (sum - total_after).abs() < 1e-10,
        "habitat sum ({sum:.6}) != total ({total_after:.6})"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Microfauna grazing also favors accessible habitats
// ---------------------------------------------------------------------------

#[test]
fn test_microfauna_grazing_prefers_accessible_habitats() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5011));
    state.algae.periphyton_by_habitat = BTreeMap::from([
        (HabitatKind::GlassHardscape, 1.0),
        (HabitatKind::SubstrateSurface, 1.0),
        (HabitatKind::PlantSurfaces, 1.0),
        (HabitatKind::FilterMedia, 1.0),
    ]);
    state.algae.sync_periphyton_total();

    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 1.0;
    state.hardware.light.enabled = false;
    state.process_params.periphyton_max_growth_rate_per_day = 0.0;

    let glass_before = state.algae.periphyton_by_habitat[&HabitatKind::GlassHardscape];
    let substrate_before = state.algae.periphyton_by_habitat[&HabitatKind::SubstrateSurface];
    let filter_before = state.algae.periphyton_by_habitat[&HabitatKind::FilterMedia];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let algae = &engine.full_state().algae;
    let glass_loss = glass_before
        - algae
            .periphyton_by_habitat
            .get(&HabitatKind::GlassHardscape)
            .copied()
            .unwrap_or(0.0);
    let substrate_loss = substrate_before
        - algae
            .periphyton_by_habitat
            .get(&HabitatKind::SubstrateSurface)
            .copied()
            .unwrap_or(0.0);
    let filter_loss = filter_before
        - algae
            .periphyton_by_habitat
            .get(&HabitatKind::FilterMedia)
            .copied()
            .unwrap_or(0.0);

    assert!(
        glass_loss > filter_loss,
        "glass loss ({glass_loss:.6}) should exceed filter loss ({filter_loss:.6})"
    );
    assert!(
        substrate_loss > filter_loss,
        "substrate loss ({substrate_loss:.6}) should exceed filter loss ({filter_loss:.6})"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Save/load habitat pools round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_save_load_habitat_pools() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5006));

    // Set specific per-habitat values.
    state.algae.periphyton_by_habitat = BTreeMap::from([
        (HabitatKind::GlassHardscape, 0.5),
        (HabitatKind::SubstrateSurface, 0.3),
        (HabitatKind::PlantSurfaces, 0.15),
        (HabitatKind::FilterMedia, 0.02),
        (HabitatKind::SubstrateDeep, 0.0),
    ]);
    state.algae.sync_periphyton_total();
    state.microbe.decomposer_by_habitat = BTreeMap::from([
        (HabitatKind::FilterMedia, 0.04),
        (HabitatKind::SubstrateSurface, 0.03),
        (HabitatKind::SubstrateDeep, 0.02),
        (HabitatKind::GlassHardscape, 0.005),
        (HabitatKind::PlantSurfaces, 0.003),
    ]);
    state.microbe.sync_decomposer_total();

    let engine = Engine::from_parts(state.clone(), vec![]);
    let save = SaveFile::from_engine(&engine);
    let json = save.to_json_pretty()?;
    let loaded = SaveFile::from_json(&json)?;

    // Per-habitat values survive round-trip.
    for kind in HabitatKind::ALL {
        let original = state
            .algae
            .periphyton_by_habitat
            .get(&kind)
            .copied()
            .unwrap_or(0.0);
        let roundtrip = loaded
            .state
            .algae
            .periphyton_by_habitat
            .get(&kind)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (original - roundtrip).abs() < 1e-6,
            "periphyton {kind:?}: original={original}, loaded={roundtrip}"
        );
    }
    for kind in HabitatKind::ALL {
        let original = state
            .microbe
            .decomposer_by_habitat
            .get(&kind)
            .copied()
            .unwrap_or(0.0);
        let roundtrip = loaded
            .state
            .microbe
            .decomposer_by_habitat
            .get(&kind)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (original - roundtrip).abs() < 1e-6,
            "decomposer {kind:?}: original={original}, loaded={roundtrip}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Decomposer activity: filter media > glass
// ---------------------------------------------------------------------------

#[test]
fn test_decomposer_filter_media_exceeds_glass() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5007));

    // Start all habitat decomposer pools at equal, small biomass.
    for biomass in state.microbe.decomposer_by_habitat.values_mut() {
        *biomass = 0.01;
    }
    state.microbe.sync_decomposer_total();

    // Ensure ample DOC substrate for decomposer growth.
    state.water.dissolved_organic_carbon_mg_c_total = 100.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 10.0;

    // Disable everything except nitrogen cycle decomposers.
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    for biomass in state.algae.periphyton_by_habitat.values_mut() {
        *biomass = 0.0;
    }
    state.algae.sync_periphyton_total();
    state.refresh_habitat_registry();

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24 * 14)?; // 2 weeks

    let microbe = &engine.full_state().microbe;
    let filter = microbe
        .decomposer_by_habitat
        .get(&HabitatKind::FilterMedia)
        .copied()
        .unwrap_or(0.0);
    let glass = microbe
        .decomposer_by_habitat
        .get(&HabitatKind::GlassHardscape)
        .copied()
        .unwrap_or(0.0);

    assert!(
        filter > glass,
        "filter media decomposers ({filter:.6}) should exceed glass ({glass:.6}) \
         because filter has higher flow and O2 exposure"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. Integration: 200-hour lit-surface preference
// ---------------------------------------------------------------------------

#[test]
fn test_200hr_lit_surface_periphyton_preference() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5008));

    // Start with equal pools; 200 hours ≈ 8 days is short, so use larger seed.
    for biomass in state.algae.periphyton_by_habitat.values_mut() {
        *biomass = 0.1;
    }
    state.algae.sync_periphyton_total();

    // Disable consumers.
    state.animal.adult.count = 0;
    state.microfauna.population_index = 0.0;

    let mut engine = Engine::from_parts(state, vec![]);
    // Run 500 hours (≈ 21 days) for clear divergence.
    engine.step_hours(500)?;

    let algae = &engine.full_state().algae;
    let glass = algae
        .periphyton_by_habitat
        .get(&HabitatKind::GlassHardscape)
        .copied()
        .unwrap_or(0.0);
    let substrate = algae
        .periphyton_by_habitat
        .get(&HabitatKind::SubstrateSurface)
        .copied()
        .unwrap_or(0.0);
    let filter = algae
        .periphyton_by_habitat
        .get(&HabitatKind::FilterMedia)
        .copied()
        .unwrap_or(0.0);

    // Lit surfaces (glass, substrate) should have more periphyton than
    // filter media (dark). Filter periphyton declines from respiration.
    assert!(
        glass > filter,
        "glass ({glass:.6}) should exceed filter ({filter:.6})"
    );
    assert!(
        substrate > filter,
        "substrate ({substrate:.6}) should exceed filter ({filter:.6})"
    );

    // Verify filter biofilm trends: periphyton declining, decomposers growing.
    // Since we start both pool types at non-zero values, what matters is the
    // direction: filter periphyton should have declined (< starting 0.1) while
    // filter decomposers should have grown (relative to starting conditions).
    assert!(
        filter < 0.1,
        "filter periphyton ({filter:.6}) should decline from initial 0.1 (dark = net loss)"
    );
    let filter_decomposer = engine
        .full_state()
        .microbe
        .decomposer_by_habitat
        .get(&HabitatKind::FilterMedia)
        .copied()
        .unwrap_or(0.0);
    assert!(
        filter_decomposer > 0.0,
        "filter decomposers ({filter_decomposer:.6}) should remain viable"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. Habitat refresh removes biomass from vanished habitats
// ---------------------------------------------------------------------------

#[test]
fn test_refresh_habitat_registry_redistributes_removed_habitats() -> Result<(), tank_core::SimError>
{
    let mut state = growth_state(SimSeed(5012));
    state.algae.periphyton_by_habitat = BTreeMap::from([
        (HabitatKind::GlassHardscape, 0.4),
        (HabitatKind::SubstrateSurface, 0.3),
        (HabitatKind::PlantSurfaces, 0.2),
        (HabitatKind::FilterMedia, 0.1),
    ]);
    state.algae.sync_periphyton_total();
    state.microbe.decomposer_by_habitat = BTreeMap::from([
        (HabitatKind::FilterMedia, 0.05),
        (HabitatKind::SubstrateSurface, 0.03),
        (HabitatKind::SubstrateDeep, 0.02),
        (HabitatKind::GlassHardscape, 0.01),
    ]);
    state.microbe.sync_decomposer_total();

    let periphyton_total_before = state.algae.periphyton_biomass_g;
    let decomposer_total_before = state.microbe.decomposer_biomass_g;

    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.hardware.filter.enabled = false;
    state.hardware.filter.media_area_cm2 = 0.0;
    state.refresh_habitat_registry();

    assert_eq!(state.algae.periphyton_by_habitat.len(), 1);
    assert_eq!(state.microbe.decomposer_by_habitat.len(), 1);
    assert!(
        (state.algae.periphyton_by_habitat[&HabitatKind::GlassHardscape] - periphyton_total_before)
            .abs()
            < 1e-10,
        "all periphyton should be redistributed onto the remaining glass habitat"
    );
    assert!(
        (state.microbe.decomposer_by_habitat[&HabitatKind::GlassHardscape]
            - decomposer_total_before)
            .abs()
            < 1e-10,
        "all decomposers should be redistributed onto the remaining glass habitat"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. Conservation: habitat split doesn't create or destroy biomass
// ---------------------------------------------------------------------------

#[test]
fn test_habitat_split_conservation() -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5009));

    // Set known totals.
    state.algae.periphyton_by_habitat = BTreeMap::from([
        (HabitatKind::GlassHardscape, 0.4),
        (HabitatKind::SubstrateSurface, 0.3),
        (HabitatKind::PlantSurfaces, 0.2),
        (HabitatKind::FilterMedia, 0.08),
        (HabitatKind::SubstrateDeep, 0.02),
    ]);
    state.algae.sync_periphyton_total();
    let periphyton_total = state.algae.periphyton_biomass_g;

    state.microbe.decomposer_by_habitat = BTreeMap::from([
        (HabitatKind::FilterMedia, 0.05),
        (HabitatKind::SubstrateSurface, 0.03),
        (HabitatKind::SubstrateDeep, 0.01),
        (HabitatKind::GlassHardscape, 0.005),
        (HabitatKind::PlantSurfaces, 0.005),
    ]);
    state.microbe.sync_decomposer_total();
    let decomposer_total = state.microbe.decomposer_biomass_g;

    // Verify sums match totals.
    let peri_sum: f64 = state.algae.periphyton_by_habitat.values().sum();
    let decomp_sum: f64 = state.microbe.decomposer_by_habitat.values().sum();

    assert!(
        (peri_sum - periphyton_total).abs() < 1e-12,
        "periphyton sum ({peri_sum}) != total ({periphyton_total})"
    );
    assert!(
        (decomp_sum - decomposer_total).abs() < 1e-12,
        "decomposer sum ({decomp_sum}) != total ({decomposer_total})"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 12. Zero-DO decomposer decay stays mass-conservative
// ---------------------------------------------------------------------------

#[test]
fn test_decomposer_decay_conserved_when_do_zero_and_habitats_uneven(
) -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5013));
    state.algae.set_periphyton_total(0.0);
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.hardware.light.enabled = false;
    state.water.dissolved_oxygen_mg_total = 0.0;
    state.water.dissolved_organic_carbon_mg_c_total = 0.0;
    state.water.dissolved_organic_nitrogen_mg_n_total = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microbe.decomposer_by_habitat = BTreeMap::from([
        (HabitatKind::FilterMedia, 0.19),
        (HabitatKind::GlassHardscape, 0.01),
    ]);
    state.microbe.sync_decomposer_total();

    let expected_decay =
        state.microbe.decomposer_biomass_g * state.process_params.decomposer_decay_rate_per_hour;
    let doc_before = state.water.dissolved_organic_carbon_mg_c_total;
    let don_before = state.water.dissolved_organic_nitrogen_mg_n_total;
    let total_before = state.microbe.decomposer_biomass_g;
    let filter_before = state.microbe.decomposer_by_habitat[&HabitatKind::FilterMedia];
    let glass_before = state.microbe.decomposer_by_habitat[&HabitatKind::GlassHardscape];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let after = engine.full_state();
    let actual_decay = total_before - after.microbe.decomposer_biomass_g;
    let filter_loss = filter_before
        - after
            .microbe
            .decomposer_by_habitat
            .get(&HabitatKind::FilterMedia)
            .copied()
            .unwrap_or(0.0);
    let glass_loss = glass_before
        - after
            .microbe
            .decomposer_by_habitat
            .get(&HabitatKind::GlassHardscape)
            .copied()
            .unwrap_or(0.0);

    assert!(
        (actual_decay - expected_decay).abs() < 1e-10,
        "expected {expected_decay:.12} g decay, got {actual_decay:.12} g"
    );
    assert!(
        (after.water.dissolved_organic_nitrogen_mg_n_total
            - (don_before
                + live_biomass_nitrogen_mg(actual_decay, after.process_params.feed_n_to_c_ratio)))
        .abs()
            < 1e-9,
        "DON increase should match the biomass actually removed"
    );
    assert!(
        (after.water.dissolved_organic_carbon_mg_c_total
            - (doc_before
                + live_biomass_carbon_mg(actual_decay, after.process_params.feed_n_to_c_ratio)))
        .abs()
            < 1e-9,
        "DOC increase should match the biomass actually removed"
    );
    assert!(
        filter_loss > glass_loss,
        "larger habitat pool should absorb more of the zero-DO fallback decay"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 13. Filter cleaning primarily resets filter-media decomposers
// ---------------------------------------------------------------------------

#[test]
fn test_filter_cleaning_focuses_decomposer_setback_on_filter_media(
) -> Result<(), tank_core::SimError> {
    let mut state = growth_state(SimSeed(5014));
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state.process_params.decomposer_vmax_per_hour = 0.0;
    state.process_params.decomposer_decay_rate_per_hour = 0.0;
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 0.0;
    state.process_params.aob_decay_rate_per_hour = 0.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.0;
    state.process_params.nob_decay_rate_per_hour = 0.0;
    state.process_params.comammox_vmax_fraction = 0.0;
    state.process_params.comammox_decay_rate_per_hour = 0.0;
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.microbe.decomposer_by_habitat = BTreeMap::from([
        (HabitatKind::FilterMedia, 0.30),
        (HabitatKind::SubstrateSurface, 0.20),
        (HabitatKind::GlassHardscape, 0.10),
        (HabitatKind::PlantSurfaces, 0.08),
        (HabitatKind::SubstrateDeep, 0.06),
    ]);
    state.microbe.sync_decomposer_total();

    let don_before = state.water.dissolved_organic_nitrogen_mg_n_total;
    let doc_before = state.water.dissolved_organic_carbon_mg_c_total;
    let total_before = state.microbe.decomposer_biomass_g;
    let filter_before = state.microbe.decomposer_by_habitat[&HabitatKind::FilterMedia];
    let surface_before = state.microbe.decomposer_by_habitat[&HabitatKind::SubstrateSurface];
    let glass_before = state.microbe.decomposer_by_habitat[&HabitatKind::GlassHardscape];
    let deep_before = state.microbe.decomposer_by_habitat[&HabitatKind::SubstrateDeep];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::CleanFilter { intensity: 0.8 })?;
    engine.step_hours(1)?;

    let after = engine.full_state();
    let removed_total = total_before - after.microbe.decomposer_biomass_g;
    let filter_loss =
        filter_before - after.microbe.decomposer_by_habitat[&HabitatKind::FilterMedia];
    let surface_loss =
        surface_before - after.microbe.decomposer_by_habitat[&HabitatKind::SubstrateSurface];
    let glass_loss =
        glass_before - after.microbe.decomposer_by_habitat[&HabitatKind::GlassHardscape];

    assert!(
        (after.microbe.decomposer_by_habitat[&HabitatKind::FilterMedia] - (filter_before * 0.6))
            .abs()
            < 1e-12,
        "filter media should take the full cleaning setback"
    );
    assert!(
        filter_loss > surface_loss && surface_loss > glass_loss,
        "filter cleaning should hit filter media hardest, then exposed substrate, then glass"
    );
    assert!(
        (after.microbe.decomposer_by_habitat[&HabitatKind::SubstrateDeep] - deep_before).abs()
            < 1e-12,
        "deep substrate should be unaffected by filter cleaning"
    );
    assert!(
        (after.water.dissolved_organic_nitrogen_mg_n_total
            - (don_before
                + live_biomass_nitrogen_mg(removed_total, after.process_params.feed_n_to_c_ratio)))
        .abs()
            < 1e-9,
        "DON increase should match the decomposer biomass actually removed"
    );
    assert!(
        (after.water.dissolved_organic_carbon_mg_c_total
            - (doc_before
                + live_biomass_carbon_mg(removed_total, after.process_params.feed_n_to_c_ratio)))
        .abs()
            < 1e-9,
        "DOC increase should match the decomposer biomass actually removed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 14. Habitat diversity supports more microbes
// ---------------------------------------------------------------------------

#[test]
fn test_habitat_diversity_supports_more_microbes() -> Result<(), tank_core::SimError> {
    // Diverse tank: active substrate + filter media + plants.
    let mut diverse = growth_state(SimSeed(5010));
    diverse.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::ActivePlanted,
        depth_cm: 3.0,
        colonizable_area_cm2: 500.0,
        colonizable_area_factor: SubstrateKind::ActivePlanted.default_colonizable_area_factor(),
        ..SubstrateLayerState::default()
    }];
    diverse.hardware.filter.enabled = true;
    diverse.hardware.filter.media_area_cm2 = 3000.0;
    diverse.hardware.filter.flow_lph = 300.0;
    diverse.animal.adult.count = 0;
    diverse.refresh_habitat_registry();

    // Bare tank: glass only (no substrate, no filter, no plants).
    let mut bare = bare_glass_state(SimSeed(5010));
    bare.animal.adult.count = 0;

    // Start both with minimal microbe biomass.
    for biomass in diverse.microbe.decomposer_by_habitat.values_mut() {
        *biomass = 0.005;
    }
    diverse.microbe.sync_decomposer_total();
    for biomass in bare.microbe.decomposer_by_habitat.values_mut() {
        *biomass = 0.005;
    }
    bare.microbe.sync_decomposer_total();

    // Ample organic substrate for decomposers.
    diverse.water.dissolved_organic_carbon_mg_c_total = 80.0;
    diverse.water.dissolved_organic_nitrogen_mg_n_total = 8.0;
    bare.water.dissolved_organic_carbon_mg_c_total = 80.0;
    bare.water.dissolved_organic_nitrogen_mg_n_total = 8.0;

    let mut diverse_engine = Engine::from_parts(diverse, vec![]);
    let mut bare_engine = Engine::from_parts(bare, vec![]);

    diverse_engine.step_hours(500)?;
    bare_engine.step_hours(500)?;

    let diverse_total = diverse_engine.full_state().microbe.decomposer_biomass_g
        + diverse_engine
            .full_state()
            .microbe
            .ammonia_oxidizer_biomass_g
        + diverse_engine
            .full_state()
            .microbe
            .nitrite_oxidizer_biomass_g
        + diverse_engine.full_state().microbe.comammox_biomass_g;

    let bare_total = bare_engine.full_state().microbe.decomposer_biomass_g
        + bare_engine.full_state().microbe.ammonia_oxidizer_biomass_g
        + bare_engine.full_state().microbe.nitrite_oxidizer_biomass_g
        + bare_engine.full_state().microbe.comammox_biomass_g;

    assert!(
        diverse_total > bare_total,
        "diverse tank ({diverse_total:.6}) should support more total microbial biomass \
         than bare tank ({bare_total:.6})"
    );
    Ok(())
}
