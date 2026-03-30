use tank_core::{
    Engine, ProcessParams, SimError, SimSeed, SimulationEngine, TankState, WaterState,
};

/// Creates a tank with cycling crash conditions: high nitrite, established shrimp.
fn nitrite_crash_state(seed: SimSeed) -> TankState {
    let geometry = tank_core::TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.water.bicarbonate_mg_total = 300.0 * vol;
    state.algae.set_periphyton_total(5.0);

    // Cycling crash: AOB active but NOB insufficient → nitrite accumulates
    state.microbe.set_decomposer_total(0.1);
    state.microbe.ammonia_oxidizer_biomass_g = 1.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.05; // very low NOB
    state.microbe.comammox_biomass_g = 0.05;
    state.filter_state.biofilter_maturity_index = 0.4;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    // Start with elevated nitrite (cycling crash)
    state.water.nitrite_mg_n_total = 4.0 * vol; // 4 mg/L — dangerous
    // No chloride protection initially
    state.water.chloride_mg_total = 0.0;

    state.animal.adult.count = 20;
    state.animal.set_population_condition_index(0.7);
    state.animal.molt_stress_index = 0.1;
    state.animal.reproductive_readiness_index = 0.3;

    state.process_params = ProcessParams::default();
    // Slow NOB growth so nitrite stays elevated
    state.process_params.nob_vmax_mg_n_per_g_per_hour = 0.3;
    // Disable spawning to isolate mortality effects
    state.shrimp_params.base_spawn_rate = 0.0;

    state.stability_tracker.prev_temp_c = state.water.temperature_c;
    state.stability_tracker.prev_gh_d = state.gh_d();
    state.stability_tracker.prev_do_mg_l = state.do_mg_per_l();
    state.stability_tracker.prev_ph = 7.5;

    state
}

/// Integration test: salt-treatment emergency scenario.
///
/// Models a cycling crash with high nitrite. Mortality is high before
/// chloride is raised (no Cl protection). After applying NaCl treatment
/// (raising chloride), mortality rate decreases even though NO2 remains
/// elevated, because chloride inhibits nitrite uptake at the gills.
#[test]
fn test_salt_treatment_emergency_scenario() -> Result<(), SimError> {
    // Phase 1: High nitrite, no chloride — observe mortality
    let mut state_before = nitrite_crash_state(SimSeed(30_001));
    let mut engine_before = Engine::from_parts(state_before.clone(), vec![]);

    let initial_count = engine_before.full_state().animal.total_count();
    assert!(initial_count >= 20, "should start with 20 shrimp");

    // Run 7 days with no chloride protection
    engine_before.step_hours(7 * 24)?;

    let count_after_no_salt = engine_before.full_state().animal.total_count();
    let deaths_no_salt = initial_count - count_after_no_salt;

    // Phase 2: Same initial conditions but with salt treatment from day 0
    let mut state_with_salt = nitrite_crash_state(SimSeed(30_001));
    let vol = state_with_salt.water_volume_l();
    // Apply NaCl treatment: raise chloride to 100 mg/L
    // (NaCl is ~60% Cl by weight, so this represents ~167 mg/L NaCl added)
    state_with_salt.water.chloride_mg_total = 100.0 * vol;
    // Also add the sodium from NaCl
    state_with_salt.water.sodium_mg_total += 65.0 * vol;

    let mut engine_with_salt = Engine::from_parts(state_with_salt, vec![]);
    engine_with_salt.step_hours(7 * 24)?;

    let count_after_salt = engine_with_salt.full_state().animal.total_count();
    let deaths_with_salt = initial_count - count_after_salt;

    // Salt treatment should measurably reduce mortality even while NO2 is still elevated
    assert!(
        deaths_no_salt > 0,
        "high nitrite with no salt should cause some mortality (got 0 deaths)"
    );
    assert!(
        deaths_with_salt < deaths_no_salt,
        "salt treatment should reduce mortality: without_salt={deaths_no_salt} deaths, \
         with_salt={deaths_with_salt} deaths"
    );

    // Verify nitrite is still elevated in the salt-treated tank
    // (the protection is from chloride competition, not from nitrite removal)
    let final_nitrite_mg_l = engine_with_salt.full_state().concentrations().nitrite_mg_n_per_l();
    assert!(
        final_nitrite_mg_l > 0.5,
        "nitrite should still be elevated after salt treatment: {final_nitrite_mg_l:.2} mg/L"
    );

    Ok(())
}
