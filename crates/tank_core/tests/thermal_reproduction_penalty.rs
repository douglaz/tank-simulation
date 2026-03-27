use tank_core::{Engine, PlayerAction, ProcessParams, SimSeed, SimulationEngine, TankState};

/// Creates a stocked shrimp scenario at the given ambient temperature.
/// Uses a large, very well-maintained tank to isolate the thermal reproduction
/// effect from secondary dynamics (food depletion, ammonia spikes, DO crashes).
fn stocked_scenario(seed: SimSeed, ambient_temp_c: f64) -> TankState {
    // Large tank (~200 L) for strong dilution
    let geometry = tank_core::TankGeometry {
        length_cm: 80.0,
        width_cm: 50.0,
        height_cm: 55.0,
        fill_height_cm: 50.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = tank_core::WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = ambient_temp_c;
    state.environment.ambient_temp_c = ambient_temp_c;

    // Excellent water quality
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 12.0 * vol; // Very high buffering
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 400.0 * vol;

    // Very abundant periphyton so food is never limiting
    state.algae.periphyton_biomass_g = 30.0;

    // Very strong nitrification to prevent any ammonia buildup
    state.microbe.decomposer_biomass_g = 0.3;
    state.microbe.ammonia_oxidizer_biomass_g = 2.0;
    state.microbe.nitrite_oxidizer_biomass_g = 1.5;
    state.microbe.comammox_biomass_g = 0.5;
    state.filter_state.biofilter_maturity_index = 1.0;

    // Very strong aeration to maintain DO at both temperatures
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 1.0;

    // Good light for periphyton regrowth
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.7;
    state.hardware.light.photoperiod_hours = 10.0;

    // Moderate starting population
    state.animal.adults_count = 10;
    state.animal.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.5;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 10.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 10.0;
    // Very high periphyton capacity: won't be food-limited
    state.process_params.periphyton_capacity_g_per_m2 = 200.0;

    // Use a low spawn rate so population growth doesn't create a food competition
    // differential large enough to mask the direct thermal reproduction penalty.
    state.shrimp_params.base_spawn_rate = 0.03;

    // Initialize stability tracker from actual state
    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = 7.5;

    state
}

#[test]
fn thermal_reproduction_penalty() -> Result<(), tank_core::SimError> {
    let cool_state = stocked_scenario(SimSeed(9000), 25.0);
    let warm_state = stocked_scenario(SimSeed(9000), 30.0);

    let mut cool_engine = Engine::from_parts(cool_state, vec![]);
    let mut warm_engine = Engine::from_parts(warm_state, vec![]);

    // Run for 60 days as required by the spec.
    // Moderate feeding to sustain population without overwhelming nitrification.
    for _ in 0..60 {
        cool_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        warm_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        cool_engine.step_hours(24)?;
        warm_engine.step_hours(24)?;
    }

    let cool_snap = cool_engine.snapshot();
    let warm_snap = warm_engine.snapshot();

    // The 30°C tank must end with at least 20% lower reproductive readiness
    let readiness_reduction = 1.0
        - (warm_snap.shrimp_reproductive_readiness
            / cool_snap.shrimp_reproductive_readiness.max(f64::EPSILON));
    assert!(
        readiness_reduction >= 0.20,
        "30°C tank should have at least 20% lower reproductive readiness than 25°C tank \
         after 60 days. Cool: {:.4}, Warm: {:.4}, Reduction: {:.2}%",
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.shrimp_reproductive_readiness,
        readiness_reduction * 100.0
    );

    // The 30°C tank must end with fewer juveniles than the 25°C tank
    assert!(
        warm_snap.juveniles_count < cool_snap.juveniles_count,
        "30°C tank should have fewer juveniles than 25°C tank after 60 days. \
         Cool juveniles: {}, Warm juveniles: {}",
        cool_snap.juveniles_count,
        warm_snap.juveniles_count
    );

    Ok(())
}
