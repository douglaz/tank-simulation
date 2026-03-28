use tank_core::{
    plant_carbon_mg, plant_nitrogen_mg,
    systems::{algae_growth::step_daily_algae, plant_growth::step_daily_plants},
    Engine, EventKind, PlantGuild, PlantGuildState, PlayerAction, SimSeed, SimulationEngine,
    SubstrateKind, SubstrateLayerState, TankState,
};

fn base_growth_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.environment.ambient_temp_c = 25.0;
    state.water.temperature_c = 25.0;
    state.water.ammonia_total_mg_n_total = 8.0;
    state.water.nitrate_mg_n_total = 40.0;
    state.water.phosphate_mg_p_total = 6.0;
    state.water.dissolved_inorganic_carbon_mg_c_total = 260.0;
    state.hardware.light.intensity_index = 0.9;
    state.hardware.light.photoperiod_hours = 10.0;
    state.microfauna.population_index = 0.1;
    state.microfauna.grazing_pressure_index = 0.1;
    state
}

#[test]
fn plant_growth_improves_with_light_and_nutrients() -> Result<(), tank_core::SimError> {
    let mut favorable = base_growth_state(SimSeed(8100));
    favorable.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 4.0,
        health_index: 0.8,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];
    let favorable_volume_l = favorable.water_volume_l();
    favorable.water.ammonia_total_mg_n_total = 2.0 * favorable_volume_l;
    favorable.water.nitrate_mg_n_total = 10.0 * favorable_volume_l;
    favorable.water.phosphate_mg_p_total = 1.2 * favorable_volume_l;
    favorable.water.dissolved_inorganic_carbon_mg_c_total = 24.0 * favorable_volume_l;

    let mut poor = favorable.clone();
    poor.hardware.light.intensity_index = 0.15;
    let poor_volume_l = poor.water_volume_l();
    poor.water.ammonia_total_mg_n_total = 0.02 * poor_volume_l;
    poor.water.nitrate_mg_n_total = 0.08 * poor_volume_l;
    poor.water.phosphate_mg_p_total = 0.01 * poor_volume_l;
    poor.water.dissolved_inorganic_carbon_mg_c_total = 2.0 * poor_volume_l;

    let mut favorable_engine = Engine::from_parts(favorable, vec![]);
    let mut poor_engine = Engine::from_parts(poor, vec![]);

    favorable_engine.step_hours(24 * 14)?;
    poor_engine.step_hours(24 * 14)?;

    assert!(
        favorable_engine.snapshot().fast_stem_biomass_g
            > poor_engine.snapshot().fast_stem_biomass_g * 1.2,
        "favorable conditions should support materially better growth"
    );

    Ok(())
}

#[test]
fn guild_differentiated_uptake_prefers_expected_pools() -> Result<(), tank_core::SimError> {
    let mut fast_stem_state = base_growth_state(SimSeed(8101));
    fast_stem_state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 5.0,
        health_index: 0.85,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];
    fast_stem_state.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::ActivePlanted,
        depth_cm: 3.0,
        nutrient_store_mg_n_total: 60.0,
        nutrient_store_mg_p_total: 12.0,
        cation_exchange_capacity_index: 0.8,
        detritus_trapping_index: 0.4,
        colonizable_area_factor: SubstrateKind::ActivePlanted.default_colonizable_area_factor(),
        colonizable_area_cm2: 500.0,
        low_oxygen_tendency_index: 0.3,
        grazing_surface_index: 0.4,
    }];

    let mut rosette_state = fast_stem_state.clone();
    rosette_state.plant_guilds[0].guild = PlantGuild::RootFeedingRosette;
    rosette_state.plant_guilds[0].water_column_uptake_bias = Some(0.3);
    rosette_state.plant_guilds[0].substrate_uptake_bias = Some(0.9);

    let fast_initial_water = fast_stem_state.water.ammonia_total_mg_n_total
        + fast_stem_state.water.nitrate_mg_n_total
        + fast_stem_state.water.phosphate_mg_p_total;
    let fast_initial_substrate = fast_stem_state.substrate_layers[0].nutrient_store_mg_n_total
        + fast_stem_state.substrate_layers[0].nutrient_store_mg_p_total;
    let rosette_initial_water = rosette_state.water.ammonia_total_mg_n_total
        + rosette_state.water.nitrate_mg_n_total
        + rosette_state.water.phosphate_mg_p_total;
    let rosette_initial_substrate = rosette_state.substrate_layers[0].nutrient_store_mg_n_total
        + rosette_state.substrate_layers[0].nutrient_store_mg_p_total;

    let mut fast_engine = Engine::from_parts(fast_stem_state, vec![]);
    let mut rosette_engine = Engine::from_parts(rosette_state, vec![]);
    fast_engine.step_hours(24)?;
    rosette_engine.step_hours(24)?;

    let fast = fast_engine.full_state();
    let rosette = rosette_engine.full_state();
    let fast_water_loss = fast_initial_water
        - (fast.water.ammonia_total_mg_n_total
            + fast.water.nitrate_mg_n_total
            + fast.water.phosphate_mg_p_total);
    let fast_substrate_loss = fast_initial_substrate
        - (fast.substrate_layers[0].nutrient_store_mg_n_total
            + fast.substrate_layers[0].nutrient_store_mg_p_total);
    let rosette_water_loss = rosette_initial_water
        - (rosette.water.ammonia_total_mg_n_total
            + rosette.water.nitrate_mg_n_total
            + rosette.water.phosphate_mg_p_total);
    let rosette_substrate_loss = rosette_initial_substrate
        - (rosette.substrate_layers[0].nutrient_store_mg_n_total
            + rosette.substrate_layers[0].nutrient_store_mg_p_total);

    assert!(
        fast_water_loss > fast_substrate_loss,
        "fast stems should pull more nutrients from the water column"
    );
    assert!(
        rosette_substrate_loss > rosette_water_loss,
        "root-feeding rosettes should pull more nutrients from substrate stores"
    );

    Ok(())
}

#[test]
fn algae_bloom_conditions_emit_events_and_overfeeding_raises_nuisance(
) -> Result<(), tank_core::SimError> {
    let mut bloom_state = base_growth_state(SimSeed(8102));
    bloom_state.plant_guilds.clear();
    bloom_state.algae.suspended_biomass_g = 2.5;
    bloom_state.algae.periphyton_biomass_g = 0.5;
    bloom_state.water.ammonia_total_mg_n_total = 12.0;
    bloom_state.water.nitrate_mg_n_total = 50.0;
    bloom_state.water.phosphate_mg_p_total = 8.0;
    bloom_state.environment.ambient_temp_c = 28.0;
    bloom_state.water.temperature_c = 28.0;

    let mut bloom_engine = Engine::from_parts(bloom_state.clone(), vec![]);
    bloom_engine.step_hours(24)?;

    assert!(
        bloom_engine
            .full_state()
            .event_log
            .iter()
            .any(|event| event.kind == EventKind::AlgaeBloom),
        "daily algae step should emit an AlgaeBloom event when suspended biomass crosses threshold"
    );

    let mut control = base_growth_state(SimSeed(8104));
    control.plant_guilds.clear();
    control.environment.ambient_temp_c = 28.0;
    control.water.temperature_c = 28.0;
    control.algae.suspended_biomass_g = 0.05;
    control.algae.periphyton_biomass_g = 0.2;
    control.water.ammonia_total_mg_n_total = 0.2;
    control.water.nitrate_mg_n_total = 1.0;
    control.water.phosphate_mg_p_total = 0.4;
    control.microfauna.population_index = 0.0;
    control.microfauna.grazing_pressure_index = 0.0;
    let overfed = control.clone();

    let mut control_engine = Engine::from_parts(control, vec![]);
    let mut overfed_engine = Engine::from_parts(overfed, vec![]);
    for _ in 0..21 {
        control_engine.apply_action(PlayerAction::Feed { grams: 0.0 })?;
        overfed_engine.apply_action(PlayerAction::Feed { grams: 2.5 })?;
        control_engine.step_hours(24)?;
        overfed_engine.step_hours(24)?;
    }

    assert!(
        overfed_engine.snapshot().algae_nuisance_index
            > control_engine.snapshot().algae_nuisance_index,
        "overfeeding should increase algae nuisance pressure (control={:.3}, overfed={:.3})",
        control_engine.snapshot().algae_nuisance_index,
        overfed_engine.snapshot().algae_nuisance_index
    );

    Ok(())
}

#[test]
fn trim_plants_and_leave_cuttings_routes_mass_to_detritus() -> Result<(), tank_core::SimError> {
    let mut state = base_growth_state(SimSeed(8103));
    state.process_params.fine_detritus_dissolution_rate_per_hour = 0.0;
    state.plant_guilds = vec![PlantGuildState {
        guild: PlantGuild::FastStem,
        biomass_g: 10.0,
        health_index: 0.9,
        crowding_index: 0.0,
        habitat_index: 0.8,
        ..PlantGuildState::default()
    }];

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::TrimPlantsAndLeaveCuttings { fraction: 0.25 })?;
    engine.step_hours(1)?;

    let state = engine.full_state();
    let trimmed_biomass_g = 2.5;
    let expected_detritus_g = (plant_nitrogen_mg(trimmed_biomass_g)
        + plant_carbon_mg(trimmed_biomass_g, state.process_params.feed_n_to_c_ratio))
        / 1000.0;
    assert!((state.plant_guilds[0].biomass_g - 7.5).abs() < 1e-6);
    assert!((state.detritus.fine_detritus_g_total - expected_detritus_g).abs() < 1e-6);

    Ok(())
}

#[test]
fn plant_water_column_limitation_is_volume_invariant_at_fixed_concentration() {
    let build_state = |fill_height_cm: f64| {
        let mut state = base_growth_state(SimSeed(8105));
        state.geometry.fill_height_cm = fill_height_cm;
        state.substrate_layers.clear();
        let volume_l = state.water_volume_l();
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        }];
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 2.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.3 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 12.0 * volume_l;
        state
    };

    let mut shallow = build_state(8.0);
    let mut deep = build_state(18.0);

    step_daily_plants(&mut shallow);
    step_daily_plants(&mut deep);

    assert!(
        (shallow.plant_guilds[0].biomass_g - deep.plant_guilds[0].biomass_g).abs() <= 1e-9,
        "Same water-column nutrient concentrations should yield the same plant growth regardless of tank volume"
    );
}

#[test]
fn later_plant_guilds_see_updated_water_column_chemistry() {
    let mut state = base_growth_state(SimSeed(8108));
    state.substrate_layers.clear();
    state.plant_guilds = vec![
        PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        },
        PlantGuildState {
            guild: PlantGuild::FastStem,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(1.0),
            substrate_uptake_bias: Some(0.0),
        },
    ];

    let volume_l = state.water_volume_l();
    state.water.ammonia_total_mg_n_total = 0.1 * volume_l;
    state.water.nitrate_mg_n_total = 0.6 * volume_l;
    state.water.phosphate_mg_p_total = 0.08 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 30.0 * volume_l;

    step_daily_plants(&mut state);

    assert!(
        state.plant_guilds[0].biomass_g > state.plant_guilds[1].biomass_g + 0.005,
        "later guilds should compute growth against the depleted water column after earlier guild uptake"
    );
    assert!(
        state.plant_guilds[0].health_index > state.plant_guilds[1].health_index,
        "later guilds should also register the stronger nutrient stress after earlier guild uptake"
    );
}

#[test]
fn plant_substrate_limitation_drops_when_areal_store_is_depleted() {
    let build_state = |substrate_n_mg_total: f64, substrate_p_mg_total: f64| {
        let mut state = base_growth_state(SimSeed(8107));
        let volume_l = state.water_volume_l();
        state.plant_guilds = vec![PlantGuildState {
            guild: PlantGuild::RootFeedingRosette,
            biomass_g: 4.0,
            health_index: 0.8,
            crowding_index: 0.0,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(0.2),
            substrate_uptake_bias: Some(0.8),
        }];
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::ActivePlanted,
            depth_cm: 4.0,
            nutrient_store_mg_n_total: substrate_n_mg_total,
            nutrient_store_mg_p_total: substrate_p_mg_total,
            cation_exchange_capacity_index: 0.9,
            detritus_trapping_index: 0.4,
            colonizable_area_factor: SubstrateKind::ActivePlanted.default_colonizable_area_factor(),
            colonizable_area_cm2: state.geometry.footprint_area_cm2(),
            low_oxygen_tendency_index: 0.4,
            grazing_surface_index: 0.5,
        }];
        state.water.ammonia_total_mg_n_total = 0.08 * volume_l;
        state.water.nitrate_mg_n_total = 0.24 * volume_l;
        state.water.phosphate_mg_p_total = 0.04 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 18.0 * volume_l;
        state
    };

    let mut rich = build_state(40.0, 6.0);
    let mut depleted = build_state(8.0, 1.2);

    assert!(
        rich.substrate_n_mg_n_per_m2() > depleted.substrate_n_mg_n_per_m2(),
        "rich substrate should start with a higher areal N density"
    );
    assert!(
        rich.substrate_p_mg_p_per_m2() > depleted.substrate_p_mg_p_per_m2(),
        "rich substrate should start with a higher areal P density"
    );

    step_daily_plants(&mut rich);
    step_daily_plants(&mut depleted);

    assert!(
        rich.plant_guilds[0].biomass_g > depleted.plant_guilds[0].biomass_g + 0.03,
        "depleted substrate should materially reduce rooted-plant growth"
    );
}

#[test]
fn algae_water_column_limitation_is_volume_invariant_at_fixed_concentration() {
    let build_state = |substrate_depth_cm: f64| {
        let mut state = base_growth_state(SimSeed(8106));
        state.plant_guilds.clear();
        state.substrate_layers = vec![SubstrateLayerState {
            kind: SubstrateKind::InertSand,
            depth_cm: substrate_depth_cm,
            nutrient_store_mg_n_total: 0.0,
            nutrient_store_mg_p_total: 0.0,
            cation_exchange_capacity_index: 0.1,
            detritus_trapping_index: 0.3,
            colonizable_area_factor: SubstrateKind::InertSand.default_colonizable_area_factor(),
            colonizable_area_cm2: 500.0,
            low_oxygen_tendency_index: 0.2,
            grazing_surface_index: 0.4,
        }];
        let volume_l = state.water_volume_l();
        state.algae.suspended_biomass_g = 0.5;
        state.algae.periphyton_biomass_g = 0.4;
        state.water.ammonia_total_mg_n_total = 0.5 * volume_l;
        state.water.nitrate_mg_n_total = 2.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.3 * volume_l;
        state
    };

    let mut shallow = build_state(1.0);
    let mut deep = build_state(6.0);

    step_daily_algae(&mut shallow);
    step_daily_algae(&mut deep);

    assert!(
        (shallow.algae.suspended_biomass_g - deep.algae.suspended_biomass_g).abs() <= 1e-9,
        "Same nutrient concentrations should yield the same suspended-algae growth regardless of tank volume"
    );
    assert!(
        (shallow.algae.periphyton_biomass_g - deep.algae.periphyton_biomass_g).abs() <= 1e-9,
        "Same nutrient concentrations should yield the same periphyton growth regardless of tank volume"
    );
}

/// Tank-size-independence: identical concentrations in geometrically different
/// tanks (different fill heights → different volumes) produce identical
/// suspended-algae limitation factors and growth.
#[test]
fn algae_size_independence_different_geometries() {
    let build_state = |fill_height_cm: f64| {
        let mut state = base_growth_state(SimSeed(8201));
        state.geometry.fill_height_cm = fill_height_cm;
        state.plant_guilds.clear();
        state.substrate_layers.clear();
        state.microfauna.population_index = 0.0;
        state.microfauna.grazing_pressure_index = 0.0;
        let volume_l = state.water_volume_l();
        state.algae.suspended_biomass_g = 0.5;
        state.algae.periphyton_biomass_g = 0.0;
        state.water.ammonia_total_mg_n_total = 0.4 * volume_l;
        state.water.nitrate_mg_n_total = 1.5 * volume_l;
        state.water.phosphate_mg_p_total = 0.2 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
        state
    };

    let mut small = build_state(8.0);
    let mut large = build_state(20.0);
    assert!(
        (small.water_volume_l() - large.water_volume_l()).abs() > 1.0,
        "tanks must differ in volume for this test to be meaningful"
    );

    step_daily_algae(&mut small);
    step_daily_algae(&mut large);

    assert!(
        (small.algae.suspended_biomass_g - large.algae.suspended_biomass_g).abs() <= 1e-9,
        "Identical concentrations in {:.1}L and {:.1}L tanks must yield \
         equal suspended-algae growth",
        small.water_volume_l(),
        large.water_volume_l(),
    );
}

/// Verify suspended-algae growth from known inputs matches hand-computed
/// values derived from the model formulas:
///   light_factor    = half_sat(intensity × clamp(photoperiod/9, 0, 1), light_ks)
///   temp_factor     = exp(-(T − T_opt)² / (2σ²))
///   nutrient_factor = min(half_sat(DIN, N_ks), half_sat(PO₄, P_ks))
///   gross_growth    = seed × max_rate × light × temp × nutrient
#[test]
fn algae_growth_matches_hand_computed_limitation_product() {
    let mut state = base_growth_state(SimSeed(8202));
    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    let volume_l = state.water_volume_l();
    let tan_conc = 0.5_f64;
    let no3_conc = 2.0_f64;
    let po4_conc = 0.3_f64;
    let dic_conc = 20.0_f64;
    state.water.ammonia_total_mg_n_total = tan_conc * volume_l;
    state.water.nitrate_mg_n_total = no3_conc * volume_l;
    state.water.phosphate_mg_p_total = po4_conc * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = dic_conc * volume_l;

    let initial_biomass = 1.0;
    state.algae.suspended_biomass_g = initial_biomass;
    state.algae.periphyton_biomass_g = 0.0;

    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.8;
    state.hardware.light.photoperiod_hours = 10.0;

    state.water.temperature_c = 27.0;
    state.process_params.algae_temp_optimum_c = 27.0;
    state.process_params.algae_temp_sigma_c = 8.0;
    state.process_params.algae_half_saturation_n_mg_n_per_l = 0.25;
    state.process_params.algae_half_saturation_p_mg_p_per_l = 0.04;
    state.process_params.algae_light_half_saturation = 0.35;
    state.process_params.algae_max_growth_rate_per_day = 0.2;
    state.process_params.algae_respiration_fraction_per_day = 0.03;

    // Hand-computed limitation factors.
    let photoperiod_norm = (10.0_f64 / 9.0).min(1.0); // 1.0
    let eff_light = 0.8 * photoperiod_norm;
    let light_factor = eff_light / (eff_light + 0.35);
    let temp_factor = 1.0_f64; // at optimum
    let din = tan_conc + no3_conc;
    let n_factor = din / (din + 0.25);
    let p_factor = po4_conc / (po4_conc + 0.04);
    let nutrient_factor = n_factor.min(p_factor);

    let gross = initial_biomass * 0.2 * light_factor * temp_factor * nutrient_factor;
    let respiration = initial_biomass * 0.03;
    let expected = initial_biomass + gross - respiration;

    step_daily_algae(&mut state);

    assert!(
        (state.algae.suspended_biomass_g - expected).abs() < 1e-6,
        "Growth must match hand-computed product of limitation factors: \
         expected {expected:.6}, got {:.6} \
         (light={light_factor:.4}, temp={temp_factor:.4}, \
          nutrient={nutrient_factor:.4})",
        state.algae.suspended_biomass_g,
    );
}

/// Temperature one sigma below optimum should reduce gross growth by
/// exp(−0.5) relative to growth at the optimum.
#[test]
fn algae_temperature_one_sigma_below_reduces_growth() {
    let build = |temperature_c: f64| {
        let mut state = base_growth_state(SimSeed(8203));
        state.plant_guilds.clear();
        state.substrate_layers.clear();
        state.microfauna.population_index = 0.0;
        state.microfauna.grazing_pressure_index = 0.0;
        let volume_l = state.water_volume_l();
        state.water.ammonia_total_mg_n_total = 1.0 * volume_l;
        state.water.nitrate_mg_n_total = 5.0 * volume_l;
        state.water.phosphate_mg_p_total = 0.5 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 25.0 * volume_l;
        state.algae.suspended_biomass_g = 1.0;
        state.algae.periphyton_biomass_g = 0.0;
        state.water.temperature_c = temperature_c;
        state.process_params.algae_temp_optimum_c = 27.0;
        state.process_params.algae_temp_sigma_c = 8.0;
        state
    };

    let mut at_optimum = build(27.0);
    let mut one_sigma_below = build(19.0); // 27 − 8

    step_daily_algae(&mut at_optimum);
    step_daily_algae(&mut one_sigma_below);

    let growth_opt = at_optimum.algae.suspended_biomass_g - 1.0;
    let growth_cold = one_sigma_below.algae.suspended_biomass_g - 1.0;

    assert!(growth_opt > 0.0, "growth at optimum should be positive");
    assert!(
        growth_cold > 0.0,
        "growth one sigma below should still be positive"
    );

    // Gross growth scales linearly with temp_factor; respiration is identical.
    // Recover the gross values: gross = net_growth + respiration.
    let resp = 1.0 * at_optimum.process_params.algae_respiration_fraction_per_day;
    let gross_opt = growth_opt + resp;
    let gross_cold = growth_cold + resp;
    let ratio = gross_cold / gross_opt;
    let expected_ratio = (-0.5_f64).exp();

    assert!(
        (ratio - expected_ratio).abs() < 1e-6,
        "Gross-growth ratio should equal exp(−0.5) ≈ {expected_ratio:.6}, \
         got {ratio:.6}"
    );
}

/// Light off → no gross growth; only respiration loss.
#[test]
fn algae_light_off_causes_biomass_decline() {
    let mut state = base_growth_state(SimSeed(8204));
    state.plant_guilds.clear();
    state.substrate_layers.clear();
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;
    let volume_l = state.water_volume_l();
    state.water.ammonia_total_mg_n_total = 1.0 * volume_l;
    state.water.nitrate_mg_n_total = 5.0 * volume_l;
    state.water.phosphate_mg_p_total = 0.5 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 25.0 * volume_l;
    state.algae.suspended_biomass_g = 1.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.hardware.light.enabled = false;
    state.process_params.algae_respiration_fraction_per_day = 0.03;

    let expected = 1.0 - (1.0 * 0.03); // only respiration loss

    step_daily_algae(&mut state);

    assert!(
        (state.algae.suspended_biomass_g - expected).abs() < 1e-9,
        "With light off, biomass should decline by exactly the respiration \
         fraction: expected {expected:.6}, got {:.6}",
        state.algae.suspended_biomass_g,
    );
}

/// Phosphorus depletion makes P the limiting nutrient, reducing growth
/// well below what nitrogen availability alone would allow.
#[test]
fn algae_phosphorus_limits_growth_below_nitrogen_potential() {
    let build = |po4_conc: f64| {
        let mut state = base_growth_state(SimSeed(8205));
        state.plant_guilds.clear();
        state.substrate_layers.clear();
        state.microfauna.population_index = 0.0;
        state.microfauna.grazing_pressure_index = 0.0;
        let volume_l = state.water_volume_l();
        state.water.ammonia_total_mg_n_total = 1.0 * volume_l;
        state.water.nitrate_mg_n_total = 5.0 * volume_l;
        state.water.phosphate_mg_p_total = po4_conc * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 25.0 * volume_l;
        state.algae.suspended_biomass_g = 1.0;
        state.algae.periphyton_biomass_g = 0.0;
        state.water.temperature_c = 27.0;
        state.process_params.algae_temp_optimum_c = 27.0;
        state
    };

    let mut p_rich = build(0.5);
    let mut p_poor = build(0.01);

    step_daily_algae(&mut p_rich);
    step_daily_algae(&mut p_poor);

    let growth_rich = p_rich.algae.suspended_biomass_g - 1.0;
    let growth_poor = p_poor.algae.suspended_biomass_g - 1.0;

    assert!(
        growth_rich > growth_poor * 2.0,
        "P-rich growth ({growth_rich:.6}) should be substantially greater \
         than P-depleted growth ({growth_poor:.6})"
    );
}
