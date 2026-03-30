use tank_core::{
    Engine, PlayerAction, ProcessParams, SimError, SimSeed, SimulationEngine, SourceWaterProfile,
    TankState, WaterState,
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

    // Start with elevated nitrite (cycling crash) and no chloride protection
    state.water.nitrite_mg_n_total = 4.0 * vol;
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

fn salt_treatment_source_profile(state: &TankState) -> SourceWaterProfile {
    let base = WaterState::default_for_volume_l(1.0);
    SourceWaterProfile {
        temperature_c: state.water.temperature_c,
        ammonia_mg_n_per_l: 0.0,
        nitrite_mg_n_per_l: state.nitrite_mg_n_per_l(),
        nitrate_mg_n_per_l: state.nitrate_mg_n_per_l(),
        phosphate_mg_p_per_l: 0.0,
        dic_mg_c_per_l: base.dissolved_inorganic_carbon_mg_c_total,
        doc_mg_c_per_l: 0.0,
        don_mg_n_per_l: 0.0,
        alkalinity_meq_per_l: base.alkalinity_meq_total,
        calcium_mg_per_l: 40.0,
        magnesium_mg_per_l: 10.0,
        sodium_mg_per_l: base.sodium_mg_total + 130.0,
        potassium_mg_per_l: base.potassium_mg_total,
        bicarbonate_mg_per_l: base.bicarbonate_mg_total,
        chloride_mg_per_l: 200.0,
        sulfate_mg_per_l: base.sulfate_mg_total,
    }
}

/// Integration test: salt-treatment emergency scenario.
///
/// Models a cycling crash with high nitrite. Mortality is high before
/// chloride is raised (no Cl protection). After applying NaCl treatment
/// (raising chloride), mortality rate decreases even though NO2 remains
/// elevated, because chloride inhibits nitrite uptake at the gills.
#[test]
fn test_salt_treatment_emergency_scenario() -> Result<(), SimError> {
    let mut state = nitrite_crash_state(SimSeed(30_001));
    let salt_treatment_profile = salt_treatment_source_profile(&state);
    state
        .source_water_catalog
        .insert("salt_treatment".to_string(), salt_treatment_profile);

    let mut engine = Engine::from_parts(state, vec![]);
    let initial_count = engine.full_state().animal.total_count();
    assert!(initial_count >= 20, "should start with 20 shrimp");

    // Phase 1: run through an active nitrite crash with no chloride protection.
    engine.step_hours(3 * 24)?;
    let count_before_treatment = engine.full_state().animal.total_count();
    let pre_treatment_deaths = initial_count - count_before_treatment;
    assert!(
        pre_treatment_deaths > 0,
        "high nitrite with no salt should cause some mortality (got 0 deaths)"
    );

    // Phase 2: perform a 50% water change with chloride-rich source water that
    // keeps nitrite elevated, so the improvement comes from chloride protection
    // rather than nitrite removal.
    engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    engine.step_hours(1)?;

    let treated_state = engine.full_state();
    let treated_nitrite_mg_l = treated_state.concentrations().nitrite_mg_n_per_l();
    let treated_chloride_mg_l = treated_state.concentrations().chloride_mg_per_l();
    assert!(
        treated_nitrite_mg_l > 2.0,
        "nitrite should remain elevated after salt treatment: {treated_nitrite_mg_l:.2} mg/L"
    );
    assert!(
        treated_chloride_mg_l >= 95.0,
        "salt treatment should raise chloride near 100 mg/L: {treated_chloride_mg_l:.2} mg/L"
    );

    engine.step_hours(71)?;
    let count_after_treatment = engine.full_state().animal.total_count();
    let post_treatment_deaths = count_before_treatment - count_after_treatment;
    let pre_treatment_mortality_rate = pre_treatment_deaths as f64 / initial_count.max(1) as f64;
    let post_treatment_mortality_rate =
        post_treatment_deaths as f64 / count_before_treatment.max(1) as f64;

    assert!(
        post_treatment_mortality_rate < pre_treatment_mortality_rate,
        "salt treatment should lower mortality rate despite persistent nitrite: \
         pre={pre_treatment_mortality_rate:.3}, post={post_treatment_mortality_rate:.3}, \
         pre_deaths={pre_treatment_deaths}, post_deaths={post_treatment_deaths}"
    );

    Ok(())
}
