use tank_core::{
    systems::chemistry::{
        bicarbonate_mg_total_from_mmol_per_l, validate_source_water_carbonate_profile,
    },
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

fn valid_source_profile_carbonate(state: &TankState) -> (f64, f64, f64) {
    let dic_mg_c_per_l = state.dic_mg_c_per_l();
    let alkalinity_meq_per_l = state.alkalinity_meq_per_l();
    if let Ok(eq) = validate_source_water_carbonate_profile(
        dic_mg_c_per_l,
        alkalinity_meq_per_l,
        state.water.temperature_c,
    ) {
        return (
            dic_mg_c_per_l,
            alkalinity_meq_per_l,
            bicarbonate_mg_total_from_mmol_per_l(eq.hco3_mmol_per_l, 1.0),
        );
    }

    let fallback = WaterState::default_for_volume_l(1.0);
    let eq = validate_source_water_carbonate_profile(
        fallback.dissolved_inorganic_carbon_mg_c_total,
        fallback.alkalinity_meq_total,
        state.water.temperature_c,
    )
    .expect("default source-water carbonate profile should validate");
    (
        fallback.dissolved_inorganic_carbon_mg_c_total,
        fallback.alkalinity_meq_total,
        bicarbonate_mg_total_from_mmol_per_l(eq.hco3_mmol_per_l, 1.0),
    )
}

fn emergency_water_change_source_profile(
    state: &TankState,
    sodium_mg_per_l: f64,
    chloride_mg_per_l: f64,
) -> SourceWaterProfile {
    let volume_l = state.water_volume_l().max(f64::EPSILON);
    let (dic_mg_c_per_l, alkalinity_meq_per_l, bicarbonate_mg_per_l) =
        valid_source_profile_carbonate(state);
    SourceWaterProfile {
        temperature_c: state.water.temperature_c,
        ammonia_mg_n_per_l: state.tan_mg_n_per_l(),
        nitrite_mg_n_per_l: state.nitrite_mg_n_per_l(),
        nitrate_mg_n_per_l: state.nitrate_mg_n_per_l(),
        phosphate_mg_p_per_l: state.phosphate_mg_p_per_l(),
        dic_mg_c_per_l,
        doc_mg_c_per_l: state.doc_mg_c_per_l(),
        don_mg_n_per_l: state.don_mg_n_per_l(),
        alkalinity_meq_per_l,
        calcium_mg_per_l: state.calcium_mg_per_l(),
        magnesium_mg_per_l: state.magnesium_mg_per_l(),
        sodium_mg_per_l,
        potassium_mg_per_l: state.water.potassium_mg_total / volume_l,
        bicarbonate_mg_per_l,
        chloride_mg_per_l,
        sulfate_mg_per_l: state.water.sulfate_mg_total / volume_l,
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
    let mut crash_engine = Engine::from_parts(nitrite_crash_state(SimSeed(30_001)), vec![]);
    let initial_count = crash_engine.full_state().animal.total_count();
    assert!(initial_count >= 20, "should start with 20 shrimp");

    // Phase 1: run through an active nitrite crash with no chloride protection.
    crash_engine.step_hours(3 * 24)?;
    let pre_treatment_state = crash_engine.full_state().clone();
    let count_before_treatment = pre_treatment_state.animal.total_count();
    let pre_treatment_deaths = initial_count - count_before_treatment;
    assert!(
        pre_treatment_deaths > 0,
        "high nitrite with no salt should cause some mortality (got 0 deaths)"
    );

    let volume_l = pre_treatment_state.water_volume_l().max(f64::EPSILON);
    let baseline_sodium_mg_per_l = pre_treatment_state.water.sodium_mg_total / volume_l;
    let baseline_chloride_mg_per_l = pre_treatment_state.concentrations().chloride_mg_per_l();
    let salt_treatment_profile = emergency_water_change_source_profile(
        &pre_treatment_state,
        baseline_sodium_mg_per_l + 130.0,
        200.0,
    );
    let control_profile = emergency_water_change_source_profile(
        &pre_treatment_state,
        baseline_sodium_mg_per_l,
        baseline_chloride_mg_per_l,
    );
    salt_treatment_profile.validate("salt_treatment")?;
    control_profile.validate("matched_control")?;

    let mut treated_state = pre_treatment_state.clone();
    treated_state
        .source_water_catalog
        .insert("salt_treatment".to_string(), salt_treatment_profile.clone());
    let mut treated_engine = Engine::from_parts(treated_state, vec![]);

    let mut control_state = pre_treatment_state.clone();
    control_state
        .source_water_catalog
        .insert("matched_control".to_string(), control_profile);
    let mut control_engine = Engine::from_parts(control_state, vec![]);

    let mut salt_without_protection_state = pre_treatment_state;
    salt_without_protection_state
        .shrimp_params
        .chloride_protection_factor = 0.0;
    salt_without_protection_state
        .source_water_catalog
        .insert("salt_treatment".to_string(), salt_treatment_profile);
    let mut salt_without_protection_engine =
        Engine::from_parts(salt_without_protection_state, vec![]);

    // Phase 2: apply matched 50% water changes. Only the salt-treatment branch
    // changes chloride/sodium, and the no-protection branch keeps the same salt
    // chemistry while disabling the chloride protection mechanic itself.
    treated_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    control_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "matched_control".to_string(),
    })?;
    salt_without_protection_engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 50.0,
        source_profile_id: "salt_treatment".to_string(),
    })?;
    treated_engine.step_hours(1)?;
    control_engine.step_hours(1)?;
    salt_without_protection_engine.step_hours(1)?;

    let treated_state = treated_engine.full_state();
    let control_state = control_engine.full_state();
    let treated_nitrite_mg_l = treated_state.concentrations().nitrite_mg_n_per_l();
    let control_nitrite_mg_l = control_state.concentrations().nitrite_mg_n_per_l();
    let treated_chloride_mg_l = treated_state.concentrations().chloride_mg_per_l();
    let control_chloride_mg_l = control_state.concentrations().chloride_mg_per_l();
    assert!(
        treated_nitrite_mg_l > 2.0,
        "nitrite should remain elevated after salt treatment: {treated_nitrite_mg_l:.2} mg/L"
    );
    assert!(
        control_nitrite_mg_l > 2.0,
        "matched control should also keep nitrite elevated: {control_nitrite_mg_l:.2} mg/L"
    );
    assert!(
        (treated_nitrite_mg_l - control_nitrite_mg_l).abs() < 1e-6,
        "salt treatment should not change nitrite relative to the matched water-change control: \
         treated={treated_nitrite_mg_l:.4}, control={control_nitrite_mg_l:.4}"
    );
    assert!(
        treated_chloride_mg_l >= 95.0,
        "salt treatment should raise chloride near 100 mg/L: {treated_chloride_mg_l:.2} mg/L"
    );
    assert!(
        control_chloride_mg_l < 5.0,
        "matched control should leave chloride near the untreated baseline: {control_chloride_mg_l:.2} mg/L"
    );

    treated_engine.step_hours(71)?;
    control_engine.step_hours(71)?;
    salt_without_protection_engine.step_hours(71)?;

    let treated_deaths = count_before_treatment - treated_engine.full_state().animal.total_count();
    let control_deaths = count_before_treatment - control_engine.full_state().animal.total_count();
    let no_protection_deaths = count_before_treatment
        - salt_without_protection_engine
            .full_state()
            .animal
            .total_count();
    let treated_mortality_rate = treated_deaths as f64 / count_before_treatment.max(1) as f64;
    let control_mortality_rate = control_deaths as f64 / count_before_treatment.max(1) as f64;
    let no_protection_mortality_rate =
        no_protection_deaths as f64 / count_before_treatment.max(1) as f64;

    assert!(
        treated_mortality_rate < control_mortality_rate,
        "salt treatment should beat the matched water-change control while nitrite stays elevated: \
         treated={treated_mortality_rate:.3}, control={control_mortality_rate:.3}, \
         treated_deaths={treated_deaths}, control_deaths={control_deaths}, \
         pre_treatment_deaths={pre_treatment_deaths}"
    );
    assert!(
        treated_mortality_rate < no_protection_mortality_rate,
        "the chloride-rich branch should lose its benefit when chloride protection is disabled: \
         treated={treated_mortality_rate:.3}, no_protection={no_protection_mortality_rate:.3}, \
         treated_deaths={treated_deaths}, no_protection_deaths={no_protection_deaths}"
    );

    Ok(())
}
