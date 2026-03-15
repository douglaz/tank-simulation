use tank_core::{Engine, PlayerAction, ProcessParams, SimSeed, SimulationEngine, TankState};

/// Creates a stocked shrimp scenario at the given ambient temperature.
fn stocked_scenario(seed: SimSeed, ambient_temp_c: f64) -> TankState {
    let geometry = tank_core::TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };
    let water = tank_core::WaterState::default_for_geometry(&geometry);

    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = water;
    state.water.temperature_c = ambient_temp_c;
    state.environment.ambient_temp_c = ambient_temp_c;

    // Good water quality for shrimp
    let vol = state.geometry.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;

    // Good periphyton for food
    state.algae.periphyton_biomass_g = 5.0;

    // Strong nitrification (mature filter)
    state.microbe.decomposer_biomass_g = 0.1;
    state.microbe.ammonia_oxidizer_biomass_g = 0.8;
    state.microbe.nitrite_oxidizer_biomass_g = 0.5;
    state.microbe.comammox_biomass_g = 0.15;
    state.filter_state.biofilter_maturity_index = 1.0;

    // Enable aeration
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;

    // Good light for periphyton growth
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    // Stock with 10 adult shrimp
    state.animal.adults_count = 10;
    state.animal.condition_index = 0.8;
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.5;

    state.process_params = ProcessParams::default();
    state.process_params.aob_vmax_mg_n_per_g_per_hour = 5.0;
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 5.0;
    state.process_params.periphyton_capacity_g_per_m2 = 30.0;

    // Initialize stability tracker from actual state
    let ca_mg_l = state.water.calcium_mg_total / vol;
    let mg_mg_l = state.water.magnesium_mg_total / vol;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = gh_d;
    state.stability_tracker.prev_do_mg_l = state.water.dissolved_oxygen_mg_total / vol;
    state.stability_tracker.prev_ph = 7.5;

    state
}

#[test]
fn thermal_reproduction_penalty() -> Result<(), tank_core::SimError> {
    let cool_state = stocked_scenario(SimSeed(9000), 25.0);
    let warm_state = stocked_scenario(SimSeed(9000), 30.0);

    let mut cool_engine = Engine::from_parts(cool_state, vec![]);
    let mut warm_engine = Engine::from_parts(warm_state, vec![]);

    // Run for 20 days — long enough for readiness to stabilize but before
    // hatching (21-day egg cycle) disrupts the comparison via population dynamics.
    for _ in 0..20 {
        cool_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        warm_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        cool_engine.step_hours(24)?;
        warm_engine.step_hours(24)?;
    }

    let cool_snap = cool_engine.snapshot();
    let warm_snap = warm_engine.snapshot();

    // The 30°C tank must have at least 20% lower reproductive readiness
    let readiness_reduction = 1.0 - (warm_snap.shrimp_reproductive_readiness
        / cool_snap.shrimp_reproductive_readiness.max(f64::EPSILON));
    assert!(
        readiness_reduction >= 0.20,
        "30°C tank should have at least 20% lower reproductive readiness than 25°C tank. \
         Cool: {:.4}, Warm: {:.4}, Reduction: {:.2}%",
        cool_snap.shrimp_reproductive_readiness,
        warm_snap.shrimp_reproductive_readiness,
        readiness_reduction * 100.0
    );

    // Continue to 60 days for juvenile comparison
    for _ in 0..40 {
        cool_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        warm_engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        cool_engine.step_hours(24)?;
        warm_engine.step_hours(24)?;
    }

    let cool_snap_60 = cool_engine.snapshot();
    let warm_snap_60 = warm_engine.snapshot();

    // Higher-temperature tank should produce fewer juveniles (or equal)
    // due to reduced spawning from thermal reproduction penalty
    let cool_total = cool_snap_60.adult_shrimp_count + cool_snap_60.juveniles_count;
    let warm_total = warm_snap_60.adult_shrimp_count + warm_snap_60.juveniles_count;
    assert!(
        warm_total <= cool_total || warm_snap_60.juveniles_count <= cool_snap_60.juveniles_count,
        "30°C tank should have fewer or equal total population. Cool: {}, Warm: {}",
        cool_total,
        warm_total
    );

    Ok(())
}
