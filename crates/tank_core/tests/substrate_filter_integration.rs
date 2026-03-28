use tank_core::{
    Engine, ProcessParams, SimSeed, SimulationEngine, SubstrateKind, SubstrateLayerState,
    TankGeometry, TankState, WaterState,
};

fn medium_geometry() -> TankGeometry {
    TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    }
}

fn large_geometry() -> TankGeometry {
    TankGeometry {
        length_cm: 100.0,
        width_cm: 40.0,
        height_cm: 30.0,
        fill_height_cm: 25.0,
        glass_thickness_mm: 6.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    }
}

fn base_state(seed: SimSeed, geometry: TankGeometry) -> TankState {
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.substrate_layers = vec![SubstrateLayerState {
        kind: SubstrateKind::InertGravel,
        depth_cm: 4.0,
        nutrient_store_mg_n_total: 0.0,
        nutrient_store_mg_p_total: 0.0,
        cation_exchange_capacity_index: 0.1,
        detritus_trapping_index: 0.2,
        colonizable_area_cm2: 500.0,
        low_oxygen_tendency_index: 0.2,
        grazing_surface_index: 0.4,
    }];
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.environment.ambient_temp_c = 24.0;
    state.water.temperature_c = 24.0;
    state.process_params = ProcessParams::default();
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.periphyton_capacity_g_per_m2 = 16.0;
    state.hardware.light.enabled = false;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.periphyton_biomass_g = 0.0;
    state.algae.nuisance_index = 0.0;
    state.microfauna.population_index = 0.0;
    state.microfauna.grazing_pressure_index = 0.0;

    let volume_l = state.water_volume_l();
    state.water.alkalinity_meq_total = 3.0 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 40.0 * volume_l;
    state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
    state.water.calcium_mg_total = 35.0 * volume_l;
    state.water.magnesium_mg_total = 10.0 * volume_l;
    state.water.bicarbonate_mg_total = 183.0 * volume_l;
    tank_core::systems::chemistry::resolve_carbonate_state(&mut state.water, volume_l);

    state.microbe.decomposer_biomass_g = 0.12;
    state.microbe.ammonia_oxidizer_biomass_g = 0.05;
    state.microbe.nitrite_oxidizer_biomass_g = 0.04;
    state.microbe.comammox_biomass_g = 0.01;
    state.filter_state.biofilter_maturity_index = 0.3;
    state.hardware.filter.enabled = true;
    state.hardware.filter.flow_lph = 200.0;
    state.hardware.filter.cleanliness_index = 0.4;

    state
}

fn run_daily_feed(
    engine: &mut Engine,
    days: u32,
    grams: f64,
) -> Result<(f64, f64), tank_core::SimError> {
    let mut peak_tan_mg_l = 0.0_f64;
    let mut tan_exposure = 0.0_f64;

    for _ in 0..days {
        engine.apply_action(tank_core::PlayerAction::Feed { grams })?;
        for _ in 0..24 {
            engine.step_hours(1)?;
            let tan_mg_l = engine.full_state().tan_mg_n_per_l();
            peak_tan_mg_l = peak_tan_mg_l.max(tan_mg_l);
            tan_exposure += tan_mg_l;
        }
    }

    Ok((peak_tan_mg_l, tan_exposure))
}

#[test]
fn substrate_trapping_divergence() -> Result<(), tank_core::SimError> {
    let mut gravel_state = base_state(SimSeed(11_001), medium_geometry());
    gravel_state.substrate_layers[0].kind = SubstrateKind::InertGravel;
    gravel_state.substrate_layers[0].detritus_trapping_index = 0.2;

    let mut porous_state = gravel_state.clone();
    porous_state.substrate_layers[0].kind = SubstrateKind::CoarsePorous;
    porous_state.substrate_layers[0].detritus_trapping_index = 0.7;

    let mut gravel = Engine::from_parts(gravel_state, vec![]);
    let mut porous = Engine::from_parts(porous_state, vec![]);

    let (gravel_peak_tan, _) = run_daily_feed(&mut gravel, 14, 0.35)?;
    let (porous_peak_tan, _) = run_daily_feed(&mut porous, 14, 0.35)?;

    let gravel_particulate = gravel.full_state().detritus.particulate_organics_g_total;
    let porous_particulate = porous.full_state().detritus.particulate_organics_g_total;

    assert!(
        porous_particulate >= gravel_particulate * 1.10,
        "coarse porous substrate should retain at least 10% more particulate detritus: porous={porous_particulate:.3}, gravel={gravel_particulate:.3}"
    );
    assert!(
        porous_peak_tan < gravel_peak_tan,
        "coarse porous substrate should reduce peak TAN via slower release: porous={porous_peak_tan:.3} mg/L, gravel={gravel_peak_tan:.3} mg/L"
    );

    Ok(())
}

#[test]
fn substrate_grazing_divergence() -> Result<(), tank_core::SimError> {
    let mut low_grazing = base_state(SimSeed(11_002), medium_geometry());
    low_grazing.hardware.light.enabled = true;
    low_grazing.hardware.light.intensity_index = 0.8;
    low_grazing.hardware.light.photoperiod_hours = 10.0;
    low_grazing.hardware.aeration.enabled = true;
    low_grazing.hardware.aeration.intensity = 0.8;
    low_grazing.filter_state.biofilter_maturity_index = 1.0;
    low_grazing.process_params.shrimp_stress_mortality_scale = 0.0;
    low_grazing.microbe.ammonia_oxidizer_biomass_g = 0.25;
    low_grazing.microbe.nitrite_oxidizer_biomass_g = 0.2;
    low_grazing.microbe.comammox_biomass_g = 0.06;
    low_grazing.water.ammonia_total_mg_n_total = 0.0;
    low_grazing.water.nitrate_mg_n_total = 3.0 * low_grazing.water_volume_l();
    low_grazing.water.phosphate_mg_p_total = 0.6 * low_grazing.water_volume_l();
    low_grazing.algae.periphyton_biomass_g = 25.0;
    low_grazing.detritus.fine_detritus_g_total = 18.0;
    low_grazing.animal.adult.count = 8;
    low_grazing.animal.adult.condition_index = 0.55;
    low_grazing.animal.molt_stress_index = 0.1;
    low_grazing.animal.reproductive_readiness_index = 0.4;
    low_grazing.shrimp_params.base_spawn_rate = 0.0;
    low_grazing.substrate_layers[0].grazing_surface_index = 0.2;

    let mut high_grazing = low_grazing.clone();
    high_grazing.substrate_layers[0].kind = SubstrateKind::CoarsePorous;
    high_grazing.substrate_layers[0].grazing_surface_index = 0.9;

    let mut low = Engine::from_parts(low_grazing, vec![]);
    let mut high = Engine::from_parts(high_grazing, vec![]);

    for _ in 0..30 {
        low.step_hours(24)?;
        high.step_hours(24)?;
    }

    let low_reserve = low.full_state().animal.adult.reserve_g;
    let high_reserve = high.full_state().animal.adult.reserve_g;

    // Higher grazing-surface substrate increases the amount of food shrimp can
    // actually route into retained biomass. Terminal condition is now heavily
    // flattened by the shared late-stage chemistry crash in this 30-day
    // no-water-change fixture, so reserve is the more direct deterministic
    // signal for the grazing-surface divergence this test is meant to cover.
    assert!(
        high_reserve >= low_reserve * 1.40,
        "high-grazing substrate should retain at least 40% more shrimp reserve after 30 days: high={high_reserve:.3} g, low={low_reserve:.3} g"
    );

    Ok(())
}

#[test]
fn filter_enabled_vs_disabled() -> Result<(), tank_core::SimError> {
    let mut filtered_state = base_state(SimSeed(11_003), medium_geometry());
    filtered_state.hardware.filter.enabled = true;
    filtered_state.hardware.filter.flow_lph = 200.0;
    filtered_state.filter_state.biofilter_maturity_index = 0.2;
    filtered_state.process_params.aob_vmax_mg_n_per_g_per_hour = 1.2;
    filtered_state.process_params.nob_vmax_mg_n_per_g_per_hour = 4.0;
    filtered_state.microbe.ammonia_oxidizer_biomass_g = 0.02;
    filtered_state.microbe.nitrite_oxidizer_biomass_g = 0.12;
    filtered_state.microbe.comammox_biomass_g = 0.002;
    filtered_state.water.ammonia_total_mg_n_total = 1.5 * filtered_state.water_volume_l();
    filtered_state.water.nitrite_mg_n_total = 1.0 * filtered_state.water_volume_l();

    let mut unfiltered_state = filtered_state.clone();
    unfiltered_state.hardware.filter.enabled = false;

    let mut filtered = Engine::from_parts(filtered_state, vec![]);
    let mut unfiltered = Engine::from_parts(unfiltered_state, vec![]);

    filtered.step_hours(24 * 30)?;
    unfiltered.step_hours(24 * 30)?;

    let filtered_tan = filtered.full_state().tan_mg_n_per_l();
    let filtered_nitrite = filtered.full_state().nitrite_mg_n_per_l();
    let unfiltered_tan = unfiltered.full_state().tan_mg_n_per_l();
    let unfiltered_nitrite = unfiltered.full_state().nitrite_mg_n_per_l();

    assert!(
        filtered_tan < unfiltered_tan,
        "enabled filter should reach lower TAN after 30 days: filtered={filtered_tan:.3} mg/L, disabled={unfiltered_tan:.3} mg/L"
    );
    assert!(
        filtered_nitrite < unfiltered_nitrite,
        "enabled filter should reach lower nitrite after 30 days: filtered={filtered_nitrite:.3} mg/L, disabled={unfiltered_nitrite:.3} mg/L"
    );
    assert!(
        filtered.full_state().filter_state.biofilter_maturity_index
            > unfiltered
                .full_state()
                .filter_state
                .biofilter_maturity_index,
        "enabled filter should build more biofilter maturity"
    );

    Ok(())
}

#[test]
fn filter_flow_affects_cycling() -> Result<(), tank_core::SimError> {
    let mut low_flow_state = base_state(SimSeed(11_004), large_geometry());
    low_flow_state.hardware.filter.flow_lph = 50.0;
    low_flow_state.filter_state.biofilter_maturity_index = 0.25;
    low_flow_state.microbe.ammonia_oxidizer_biomass_g = 0.04;
    low_flow_state.microbe.nitrite_oxidizer_biomass_g = 0.03;
    low_flow_state.microbe.comammox_biomass_g = 0.008;

    let mut high_flow_state = low_flow_state.clone();
    high_flow_state.hardware.filter.flow_lph = 400.0;

    let mut low_flow = Engine::from_parts(low_flow_state, vec![]);
    let mut high_flow = Engine::from_parts(high_flow_state, vec![]);

    let (_, low_exposure) = run_daily_feed(&mut low_flow, 30, 0.60)?;
    let (_, high_exposure) = run_daily_feed(&mut high_flow, 30, 0.60)?;

    let low_final_tan = low_flow.full_state().tan_mg_n_per_l();
    let high_final_tan = high_flow.full_state().tan_mg_n_per_l();

    assert!(
        high_exposure < low_exposure,
        "higher filter flow should reduce cumulative TAN exposure: high={high_exposure:.3}, low={low_exposure:.3}"
    );
    // Higher flow should have lower or equal final TAN.
    assert!(
        high_final_tan <= low_final_tan,
        "higher flow should not have more TAN: high={high_final_tan:.3} mg/L, low={low_final_tan:.3} mg/L"
    );
    // Both should remain below 20 mg/L — cycling is active even if not fully cleared.
    assert!(
        high_final_tan < 20.0 && low_final_tan < 20.0,
        "TAN should stay manageable: high={high_final_tan:.3} mg/L, low={low_final_tan:.3} mg/L"
    );

    Ok(())
}
