//! Integration tests for root-zone oxygenation (radial oxygen loss, ROL).
//!
//! Validates that rooted plants deepen substrate O₂ penetration, that the
//! effect is proportional to biomass with diminishing returns, and that the
//! deeper oxic zone increases substrate-surface nitrification and (when E4b
//! is active) shifts denitrification capacity.

use tank_core::{
    Engine, PlantGuild, SimSeed, SimulationEngine, SubstrateKind, SubstrateLayerState, TankState,
};

/// Build a state with thick substrate, moderate DO, and controllable
/// plant presence for testing root-zone oxygenation effects.
fn make_rol_test_state(seed: SimSeed, rooted_biomass_g: f64) -> TankState {
    let mut state = TankState::new(seed);
    let vol = state.water_volume_l();
    let footprint_cm2 = state.geometry.footprint_area_cm2();

    state.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::ActivePlanted,
        depth_cm: 8.0,
        o2_penetration_depth_cm: 1.0,
        porosity: 0.50,
        colonizable_area_factor: 0.8,
        colonizable_area_cm2: footprint_cm2 * 8.0 * 0.8,
        nutrient_store_mg_n_total: 100.0,
        nutrient_store_mg_p_total: 20.0,
        cation_exchange_capacity_index: 0.5,
        detritus_trapping_index: 0.3,
        low_oxygen_tendency_index: 0.5,
        grazing_surface_index: 0.4,
    }];

    // Moderate DO: 6 mg/L
    state.water.dissolved_oxygen_mg_total = 6.0 * vol;

    // High decomposer biomass keeps the diffusive baseline shallow so the
    // explicit ROL bonus dominates.
    state.microbe.decomposer_biomass_g = 20.0;

    // Nitrifiers for substrate-surface nitrification
    state.microbe.ammonia_oxidizer_biomass_g = 0.05;
    state.microbe.nitrite_oxidizer_biomass_g = 0.025;
    state.microbe.comammox_biomass_g = 0.01;
    state.microbe.denitrifier_activity_index = 0.8;
    state.filter_state.biofilter_maturity_index = 1.0;

    // Ammonia plus a modest organic reservoir keep the long-horizon scenario
    // active enough for nitrifier biomass to diverge.
    state.water.ammonia_total_mg_n_total = 5.0 * vol;
    state.water.dissolved_organic_carbon_mg_c_total = 12.0 * vol;
    state.water.dissolved_organic_nitrogen_mg_n_total = 1.5 * vol;
    state.detritus.particulate_organics_g_total = 1.5;
    state.detritus.fine_detritus_g_total = 0.8;

    // Disable plant growth/respiration so biomass stays fixed
    state
        .process_params
        .plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    state.process_params.plant_max_growth_rate_fast_stem_per_day = 0.0;
    state
        .process_params
        .plant_max_growth_rate_root_rosette_per_day = 0.0;
    state.process_params.plant_respiration_fraction_per_day = 0.0;
    state.process_params.plant_senescence_fraction_per_day = 0.0;

    // Zero animals to simplify
    state.animal.adult.count = 0;
    state.animal.sub_adult.count = 0;
    state.animal.juvenile.count = 0;
    state.algae.suspended_biomass_g = 0.0;
    state.algae.set_periphyton_total(0.0);

    // Set plant biomass
    for plant in &mut state.plant_guilds {
        plant.biomass_g = if matches!(plant.guild, PlantGuild::RootFeedingRosette) {
            rooted_biomass_g
        } else {
            0.0
        };
    }

    state.refresh_habitat_registry();
    tank_core::systems::substrate::step_substrate_zones(&mut state);
    state
}

// ---------------------------------------------------------------------------
// Integration: planted vs unplanted diverge in penetration over 500 hours
// ---------------------------------------------------------------------------

#[test]
fn planted_vs_unplanted_diverge_in_penetration_depth() -> Result<(), Box<dyn std::error::Error>> {
    let planted = make_rol_test_state(SimSeed(100), 15.0);
    let unplanted = make_rol_test_state(SimSeed(100), 0.0);

    // Verify initial divergence
    let pen_planted_init = planted.substrate_o2_penetration_depth_cm();
    let pen_unplanted_init = unplanted.substrate_o2_penetration_depth_cm();
    assert!(
        pen_planted_init > pen_unplanted_init,
        "planted should start with deeper penetration: planted={pen_planted_init:.4}, unplanted={pen_unplanted_init:.4}"
    );

    let mut engine_planted = Engine::from_parts(planted, vec![]);
    let mut engine_unplanted = Engine::from_parts(unplanted, vec![]);

    engine_planted.step_hours(500)?;
    engine_unplanted.step_hours(500)?;

    let pen_planted = engine_planted
        .full_state()
        .substrate_o2_penetration_depth_cm();
    let pen_unplanted = engine_unplanted
        .full_state()
        .substrate_o2_penetration_depth_cm();

    assert!(
        pen_planted > pen_unplanted,
        "after 500 hours, planted should maintain deeper O₂ penetration: \
         planted={pen_planted:.4} cm, unplanted={pen_unplanted:.4} cm"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Integration: planted substrate shifts denitrification because the deeper
// rooted oxic zone leaves less suboxic volume available near the surface.
// ---------------------------------------------------------------------------

#[test]
fn planted_substrate_denitrification_interaction() -> Result<(), Box<dyn std::error::Error>> {
    let planted = make_rol_test_state(SimSeed(101), 15.0);
    let unplanted = make_rol_test_state(SimSeed(101), 0.0);

    let mut engine_planted = Engine::from_parts(planted, vec![]);
    let mut engine_unplanted = Engine::from_parts(unplanted, vec![]);

    engine_planted.step_hours(500)?;
    engine_unplanted.step_hours(500)?;

    let export_planted = engine_planted.full_state().cumulative_n2_export_mg_n;
    let export_unplanted = engine_unplanted.full_state().cumulative_n2_export_mg_n;

    // Both should have some denitrification (both have suboxic zones in 8cm bed)
    assert!(
        export_planted > 0.001 || export_unplanted > 0.001,
        "at least one tank should show measurable denitrification: \
         planted={export_planted}, unplanted={export_unplanted}"
    );

    // The key assertion: they should diverge (planted has different O₂ profile)
    let diff = (export_planted - export_unplanted).abs();
    assert!(
        diff > 0.001,
        "planted and unplanted should diverge in denitrification: \
         planted={export_planted:.4}, unplanted={export_unplanted:.4}, diff={diff:.6}"
    );

    Ok(())
}

#[test]
fn root_oxygenation_reduces_hourly_denitrification_when_it_shrinks_suboxic_volume(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut planted = make_rol_test_state(SimSeed(104), 15.0);
    let mut unplanted = make_rol_test_state(SimSeed(104), 0.0);

    let planted_output = tank_core::systems::nitrogen_cycle::step_nitrogen_cycle(&mut planted);
    let unplanted_output = tank_core::systems::nitrogen_cycle::step_nitrogen_cycle(&mut unplanted);

    assert!(
        planted.substrate_suboxic_pore_volume_cm3() < unplanted.substrate_suboxic_pore_volume_cm3(),
        "deeper rooted oxic zones should leave less suboxic pore volume: planted={}, unplanted={}",
        planted.substrate_suboxic_pore_volume_cm3(),
        unplanted.substrate_suboxic_pore_volume_cm3()
    );
    assert!(
        planted_output.denitrification_n2_export_mg_n < unplanted_output.denitrification_n2_export_mg_n,
        "smaller suboxic volume should reduce hourly denitrification export: planted={}, unplanted={}",
        planted_output.denitrification_n2_export_mg_n,
        unplanted_output.denitrification_n2_export_mg_n
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Integration: planted substrate nitrification differences
// ---------------------------------------------------------------------------

#[test]
fn planted_substrate_nitrification_surface_area() -> Result<(), Box<dyn std::error::Error>> {
    let planted = make_rol_test_state(SimSeed(102), 15.0);
    let unplanted = make_rol_test_state(SimSeed(102), 0.0);

    let mut engine_planted = Engine::from_parts(planted, vec![]);
    let mut engine_unplanted = Engine::from_parts(unplanted, vec![]);
    engine_planted.enable_budget_tracking();
    engine_unplanted.enable_budget_tracking();

    engine_planted.step_hours(500)?;
    engine_unplanted.step_hours(500)?;

    // The planted tank has deeper O₂ penetration → larger oxic substrate zone
    // → more colonizable area in SubstrateSurface habitat → better conditions
    // for aerobic nitrification. Check that nitrate levels or substrate surface
    // areas diverge.
    let pen_planted = engine_planted
        .full_state()
        .substrate_o2_penetration_depth_cm();
    let pen_unplanted = engine_unplanted
        .full_state()
        .substrate_o2_penetration_depth_cm();

    assert!(
        pen_planted > pen_unplanted,
        "planted should maintain deeper penetration after 500h: \
         planted={pen_planted:.4}, unplanted={pen_unplanted:.4}"
    );

    // Oxic zone geometry should be larger for planted
    let oxic_planted = engine_planted.full_state().substrate_oxic_zone_geometry();
    let oxic_unplanted = engine_unplanted.full_state().substrate_oxic_zone_geometry();

    assert!(
        oxic_planted.volume_cm3 > oxic_unplanted.volume_cm3,
        "planted should have larger oxic zone: \
         planted={:.2} cm³, unplanted={:.2} cm³",
        oxic_planted.volume_cm3,
        oxic_unplanted.volume_cm3
    );

    let planted_nitrifiers = engine_planted
        .full_state()
        .microbe
        .ammonia_oxidizer_biomass_g
        + engine_planted
            .full_state()
            .microbe
            .nitrite_oxidizer_biomass_g
        + engine_planted.full_state().microbe.comammox_biomass_g;
    let unplanted_nitrifiers = engine_unplanted
        .full_state()
        .microbe
        .ammonia_oxidizer_biomass_g
        + engine_unplanted
            .full_state()
            .microbe
            .nitrite_oxidizer_biomass_g
        + engine_unplanted.full_state().microbe.comammox_biomass_g;
    assert!(
        planted_nitrifiers > unplanted_nitrifiers,
        "rooted oxygenation should support more nitrifier biomass over 500h: planted={planted_nitrifiers:.4}, unplanted={unplanted_nitrifiers:.4}"
    );

    let nitrate_planted = engine_planted.full_state().water.nitrate_mg_n_total;
    let nitrate_unplanted = engine_unplanted.full_state().water.nitrate_mg_n_total;
    assert!(
        nitrate_planted > nitrate_unplanted,
        "rooted oxygenation should leave more oxidized nitrogen in the water column over 500h: planted={nitrate_planted:.4}, unplanted={nitrate_unplanted:.4}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Integration: tracing exposes root oxygenation contribution
// ---------------------------------------------------------------------------

#[test]
fn tracing_exposes_root_oxygenation() -> Result<(), Box<dyn std::error::Error>> {
    use tank_core::tracing::{SimTracer, Verbosity};

    let state = make_rol_test_state(SimSeed(103), 10.0);
    let mut engine = Engine::from_parts(state, vec![]);
    engine.enable_budget_tracking();
    engine.enable_tracing(SimTracer::new(Verbosity::Trace));

    engine.step_hours(1)?;

    let tracer = engine.tracer().expect("tracer should be set");
    let ticks = tracer.ticks();
    assert!(!ticks.is_empty(), "should have at least one tick");

    let last_tick = ticks.last().unwrap();

    let substrate_stage = last_tick
        .system("system:substrate_zones")
        .expect("should have substrate_zones stage");

    let breakdown =
        tank_core::systems::substrate::substrate_oxygenation_breakdown(engine.full_state());
    let bonus = tank_core::systems::substrate::root_oxygenation_bonus_cm(engine.full_state());
    assert!(
        bonus > 0.0,
        "ROL bonus should be positive for 10g rooted plants: {bonus}",
    );
    assert!(
        breakdown.base_penetration_cm > 0.0,
        "trace should correspond to a positive base penetration depth"
    );
    assert!(
        substrate_stage
            .notes
            .iter()
            .any(|note| note.starts_with("substrate.base_o2_penetration_depth_cm=")),
        "trace notes should expose the base penetration separately"
    );
    assert!(
        substrate_stage
            .notes
            .iter()
            .any(|note| note.starts_with("substrate.root_oxygenation_bonus_cm=")),
        "trace notes should expose the ROL bonus separately"
    );

    assert_eq!(substrate_stage.system, "system:substrate_zones");

    let budget = engine
        .budget_ledger()
        .expect("budget tracking should be active");
    let budget_entry = budget
        .ticks
        .last()
        .and_then(|tick| {
            tick.entries
                .iter()
                .find(|entry| entry.label == "system:substrate_zones")
        })
        .expect("budget should contain substrate stage");
    assert!(
        budget_entry
            .metric("substrate.base_o2_penetration_depth_cm")
            .is_some(),
        "budget metrics should expose the base penetration"
    );
    assert!(
        budget_entry
            .metric("substrate.root_oxygenation_bonus_cm")
            .is_some(),
        "budget metrics should expose the ROL bonus"
    );

    Ok(())
}
