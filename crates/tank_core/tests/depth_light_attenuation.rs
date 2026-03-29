use tank_core::{
    systems::{algae_growth::step_daily_algae, plant_growth::step_daily_plants},
    Engine, PlantGuild, PlantGuildState, SimSeed, SimulationEngine, TankState,
};

/// Helper: build a state with controlled geometry and zeroed microfauna.
fn growth_state(seed: SimSeed, fill_height_cm: f64) -> TankState {
    let mut state = TankState::new(seed);
    // Ensure tank is tall enough to hold the requested fill height.
    state.geometry.height_cm = fill_height_cm.max(state.geometry.height_cm) + 2.0;
    state.geometry.fill_height_cm = fill_height_cm;
    state.substrate_layers.clear();
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour = 0.0;
    state
        .process_params
        .photosynthesis_dic_rate_mg_c_per_g_per_hour = 0.0;
    // Set nutrient concentrations (per-litre values scaled to total).
    let volume_l = state.water_volume_l();
    state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
    state.water.nitrate_mg_n_total = 3.0 * volume_l;
    state.water.phosphate_mg_p_total = 0.4 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    state.refresh_habitat_registry();
    state
}

// ---------------------------------------------------------------------------
// Habitat light exposure tests
// ---------------------------------------------------------------------------

/// Habitat registry: deeper tank has lower substrate-surface light exposure.
#[test]
fn habitat_substrate_light_lower_in_deeper_tank() {
    let shallow = growth_state(SimSeed(9100), 10.0);
    let deep = growth_state(SimSeed(9100), 40.0);

    let shallow_sub = shallow
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::SubstrateSurface)
        .unwrap();
    let deep_sub = deep
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::SubstrateSurface)
        .unwrap();

    assert!(
        shallow_sub.light_exposure > deep_sub.light_exposure,
        "shallow substrate light ({}) should exceed deep ({})",
        shallow_sub.light_exposure,
        deep_sub.light_exposure
    );
}

/// Habitat registry: glass/hardscape light varies with depth.
#[test]
fn habitat_glass_light_lower_in_deeper_tank() {
    let shallow = growth_state(SimSeed(9101), 10.0);
    let deep = growth_state(SimSeed(9101), 40.0);

    let shallow_glass = shallow
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::GlassHardscape)
        .unwrap();
    let deep_glass = deep
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::GlassHardscape)
        .unwrap();

    assert!(
        shallow_glass.light_exposure > deep_glass.light_exposure,
        "shallow glass light ({}) should exceed deep ({})",
        shallow_glass.light_exposure,
        deep_glass.light_exposure
    );
}

/// Habitat registry: plant surface light varies with depth.
#[test]
fn habitat_plant_surface_light_lower_in_deeper_tank() {
    let shallow = growth_state(SimSeed(9102), 10.0);
    let deep = growth_state(SimSeed(9102), 40.0);

    let shallow_plant = shallow
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::PlantSurfaces)
        .unwrap();
    let deep_plant = deep
        .habitat_registry
        .iter()
        .find(|h| h.kind == tank_core::HabitatKind::PlantSurfaces)
        .unwrap();

    assert!(
        shallow_plant.light_exposure > deep_plant.light_exposure,
        "shallow plant light ({}) should exceed deep ({})",
        shallow_plant.light_exposure,
        deep_plant.light_exposure
    );
}

// ---------------------------------------------------------------------------
// Algae responds to depth
// ---------------------------------------------------------------------------

/// Algae grows more in a shallow tank than a deep one (same concentrations).
#[test]
fn algae_grows_more_in_shallow_tank() {
    let mut shallow = growth_state(SimSeed(9200), 10.0);
    let mut deep = growth_state(SimSeed(9200), 40.0);
    shallow.algae.suspended_biomass_g = 0.5;
    shallow.algae.periphyton_biomass_g = 0.0;
    deep.algae.suspended_biomass_g = 0.5;
    deep.algae.periphyton_biomass_g = 0.0;

    step_daily_algae(&mut shallow);
    step_daily_algae(&mut deep);

    assert!(
        shallow.algae.suspended_biomass_g > deep.algae.suspended_biomass_g,
        "shallow algae ({}) should outgrow deep ({})",
        shallow.algae.suspended_biomass_g,
        deep.algae.suspended_biomass_g
    );
}

// ---------------------------------------------------------------------------
// Plant responds to depth/light attenuation
// ---------------------------------------------------------------------------

/// Plants grow more in a shallow tank than a deep one.
#[test]
fn plant_grows_more_in_shallow_tank() {
    let build = |fill_height_cm: f64| -> TankState {
        let mut state = growth_state(SimSeed(9300), fill_height_cm);
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        }];
        state
    };

    let mut shallow = build(10.0);
    let mut deep = build(40.0);

    step_daily_plants(&mut shallow);
    step_daily_plants(&mut deep);

    assert!(
        shallow.plant_guilds[0].biomass_g > deep.plant_guilds[0].biomass_g,
        "shallow plant ({}) should outgrow deep ({})",
        shallow.plant_guilds[0].biomass_g,
        deep.plant_guilds[0].biomass_g
    );
}

// ---------------------------------------------------------------------------
// Turbidity (higher k) reduces growth
// ---------------------------------------------------------------------------

/// More turbid water (higher DOC/detritus) reduces algae growth.
#[test]
fn turbidity_reduces_algae_growth() {
    let mut clear = growth_state(SimSeed(9400), 22.0);
    clear.algae.suspended_biomass_g = 0.5;
    clear.algae.periphyton_biomass_g = 0.0;
    clear.detritus.fine_detritus_g_total = 0.0;
    clear.water.dissolved_organic_carbon_mg_c_total = 0.0;

    let mut turbid = clear.clone();
    turbid.detritus.fine_detritus_g_total = 5.0;
    turbid.water.dissolved_organic_carbon_mg_c_total = 50.0;

    // Verify extinction coefficients differ.
    assert!(
        turbid.extinction_coefficient() > clear.extinction_coefficient(),
        "turbid k ({}) should exceed clear k ({})",
        turbid.extinction_coefficient(),
        clear.extinction_coefficient()
    );

    step_daily_algae(&mut clear);
    step_daily_algae(&mut turbid);

    assert!(
        clear.algae.suspended_biomass_g > turbid.algae.suspended_biomass_g,
        "clear water algae ({}) should outgrow turbid ({})",
        clear.algae.suspended_biomass_g,
        turbid.algae.suspended_biomass_g
    );
}

// ---------------------------------------------------------------------------
// Self-shading feedback: algae bloom increases k
// ---------------------------------------------------------------------------

/// Denser algae suspension produces higher extinction coefficient.
#[test]
fn algae_self_shading_increases_extinction() {
    let mut sparse = growth_state(SimSeed(9500), 22.0);
    sparse.algae.suspended_biomass_g = 0.1;

    let mut dense = sparse.clone();
    dense.algae.suspended_biomass_g = 5.0;

    assert!(
        dense.extinction_coefficient() > sparse.extinction_coefficient(),
        "dense algae k ({}) should exceed sparse k ({})",
        dense.extinction_coefficient(),
        sparse.extinction_coefficient()
    );
}

// ---------------------------------------------------------------------------
// Depth-light competition scenario (AC integration test)
// ---------------------------------------------------------------------------

/// AC: shallow (20cm) vs deep (50cm) tank → shallow has higher plant growth
/// over 200 hours.  Elevated base extinction makes the depth difference
/// significant enough to dominate over the deeper tank's larger nutrient pool.
#[test]
fn shallow_vs_deep_200h_plant_growth() -> Result<(), tank_core::SimError> {
    let build = |fill_height_cm: f64| -> TankState {
        let mut state = growth_state(SimSeed(9600), fill_height_cm);
        // Boost base extinction so depth has a measurable light effect.
        state.process_params.base_extinction_coeff_per_cm = 0.03;
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        }];
        state.refresh_habitat_registry();
        state
    };

    let mut shallow_engine = Engine::from_parts(build(20.0), vec![]);
    let mut deep_engine = Engine::from_parts(build(50.0), vec![]);

    shallow_engine.step_hours(200)?;
    deep_engine.step_hours(200)?;

    let shallow_biomass = shallow_engine.full_state().plant_guilds[0].biomass_g;
    let deep_biomass = deep_engine.full_state().plant_guilds[0].biomass_g;

    assert!(
        shallow_biomass > deep_biomass,
        "after 200h, shallow plant biomass ({shallow_biomass:.4}) should \
         exceed deep ({deep_biomass:.4})"
    );

    Ok(())
}

/// Depth-light competition: shallow tank algae outcompetes deep tank algae.
#[test]
fn shallow_vs_deep_algae_competition() -> Result<(), tank_core::SimError> {
    let build = |fill_height_cm: f64| -> TankState {
        let mut state = growth_state(SimSeed(9601), fill_height_cm);
        state.process_params.base_extinction_coeff_per_cm = 0.03;
        state.plant_guilds.clear();
        state.algae.suspended_biomass_g = 0.3;
        state.algae.periphyton_biomass_g = 0.2;
        state.refresh_habitat_registry();
        state
    };

    let mut shallow_engine = Engine::from_parts(build(15.0), vec![]);
    let mut deep_engine = Engine::from_parts(build(45.0), vec![]);

    shallow_engine.step_hours(168)?; // 7 days
    deep_engine.step_hours(168)?;

    let shallow_algae = shallow_engine.full_state().algae.suspended_biomass_g;
    let deep_algae = deep_engine.full_state().algae.suspended_biomass_g;

    assert!(
        shallow_algae > deep_algae,
        "after 7 days, shallow algae ({shallow_algae:.4}) should exceed deep ({deep_algae:.4})"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Extinction coefficient composition
// ---------------------------------------------------------------------------

/// Extinction coefficient increases with each contributing factor.
#[test]
fn extinction_coefficient_is_additive() {
    let base = growth_state(SimSeed(9700), 22.0);
    let k_base = base.extinction_coefficient();
    assert!(k_base > 0.0, "base k should be positive");

    let mut with_algae = base.clone();
    with_algae.algae.suspended_biomass_g = 3.0;
    assert!(
        with_algae.extinction_coefficient() > k_base,
        "adding algae should increase k"
    );

    let mut with_doc = base.clone();
    with_doc.water.dissolved_organic_carbon_mg_c_total = 40.0;
    assert!(
        with_doc.extinction_coefficient() > k_base,
        "adding DOC should increase k"
    );

    let mut with_detritus = base.clone();
    with_detritus.detritus.fine_detritus_g_total = 3.0;
    assert!(
        with_detritus.extinction_coefficient() > k_base,
        "adding detritus should increase k"
    );
}
