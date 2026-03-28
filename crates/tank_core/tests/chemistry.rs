use tank_core::{
    systems::chemistry::{
        compute_nh3_mg_l, resolve_carbonate_state, solve_carbonate_equilibrium,
        validate_source_water_carbonate_profile, SourceWaterCarbonateValidationError,
    },
    Engine, ProcessParams, SimSeed, SimulationEngine, TankState, WaterState,
};

fn chemistry_state(seed: SimSeed, hour_of_day: u8) -> TankState {
    let mut state = TankState::new(seed);
    state.environment.hour_of_day = hour_of_day;
    state.hardware.light.photoperiod_hours = 8.0;
    state.hardware.light.intensity_index = 1.0;
    state.algae.periphyton_biomass_g = 2.5;
    state.plant_guilds[0].biomass_g = 14.0;
    state.plant_guilds[1].biomass_g = 8.0;
    state.animal.adults_count = 20;
    // Zero K_LA to isolate biological DIC effects from atmospheric CO2
    // gas exchange, which would otherwise dominate the respiration rate.
    state.process_params = ProcessParams {
        respiration_dic_rate_mg_c_per_g_per_hour: 0.20,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.35,
        reaeration_kla_base: 0.0,
        aeration_kla_boost: 0.0,
        ..ProcessParams::default()
    };
    state
}

#[test]
fn ph_drifts_with_light_driven_dic_changes() -> Result<(), tank_core::SimError> {
    let mut night_engine = Engine::from_parts(chemistry_state(SimSeed(4000), 0), vec![]);
    let mut day_engine = Engine::from_parts(chemistry_state(SimSeed(4000), 12), vec![]);

    let night_ph_before = night_engine.full_state().water.ph;
    let day_ph_before = day_engine.full_state().water.ph;

    night_engine.step_hours(1)?;
    day_engine.step_hours(1)?;

    let night_ph_after = night_engine.full_state().water.ph;
    let day_ph_after = day_engine.full_state().water.ph;
    let night_dic_after = night_engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;
    let day_dic_after = day_engine
        .full_state()
        .water
        .dissolved_inorganic_carbon_mg_c_total;

    assert!(
        night_ph_after < night_ph_before,
        "night respiration should lower pH: before={night_ph_before:.3}, after={night_ph_after:.3}"
    );
    assert!(
        day_ph_after > day_ph_before,
        "day photosynthesis should raise pH: before={day_ph_before:.3}, after={day_ph_after:.3}"
    );
    assert!(
        day_dic_after < night_dic_after,
        "lit hours should consume more DIC than night hours"
    );

    Ok(())
}

#[test]
fn nh3_speciation_matches_reference_formula() {
    let tan_mg_l = 1.2;
    let ph = 7.8;
    let temp_c = 25.0;

    let expected = {
        let pka = 0.09018 + 2729.92 / (273.2 + temp_c);
        let fraction_nh3 = 1.0 / (1.0 + 10.0_f64.powf(pka - ph));
        tan_mg_l * fraction_nh3
    };

    let computed = compute_nh3_mg_l(tan_mg_l, ph, temp_c);
    assert!((computed - expected).abs() < 1e-12);
    assert!(compute_nh3_mg_l(tan_mg_l, 8.4, temp_c) > computed);
}

#[test]
fn ph_formula_uses_state_storage_bounds() {
    // Low alkalinity + high DIC → low pH
    let low = solve_carbonate_equilibrium(500.0, 0.01, 25.0, 20.0).ph;
    // High alkalinity + low DIC → high pH
    let high = solve_carbonate_equilibrium(0.01, 12.0, 25.0, 20.0).ph;

    // The carbonate solver clamps output to the gameplay storage bounds.
    assert!((5.5..=8.5).contains(&low), "low pH {low} out of bounds");
    assert!((5.5..=8.5).contains(&high), "high pH {high} out of bounds");
    assert!(
        low < high,
        "low pH ({low}) should be less than high pH ({high})"
    );
}

#[test]
fn source_water_validation_rejects_nonfinite_neutral_fallback() {
    let err = validate_source_water_carbonate_profile(1.0e308, 1.0, 25.0)
        .expect_err("overflowed carbonate solve should be rejected");

    assert_eq!(
        err,
        SourceWaterCarbonateValidationError::NonFiniteNeutralFallback
    );
}

#[test]
fn source_water_validation_accepts_zero_dic_buffered_limit_case() {
    let eq = validate_source_water_carbonate_profile(0.0, 2.0, 25.0)
        .expect("zero-DIC buffered source water should remain valid at the alkaline ceiling");

    assert_eq!(eq.ph, 8.5);
    assert_eq!(eq.co2_aq_mmol_per_l, 0.0);
    assert_eq!(eq.hco3_mmol_per_l, 0.0);
    assert_eq!(eq.co3_mmol_per_l, 0.0);
}

#[test]
fn dic_changes_do_not_directly_shift_alkalinity() -> Result<(), tank_core::SimError> {
    let mut lit = Engine::from_parts(chemistry_state(SimSeed(4_050), 12), vec![]);
    let mut dark = Engine::from_parts(chemistry_state(SimSeed(4_051), 0), vec![]);

    let lit_alk_before = lit.full_state().water.alkalinity_meq_total;
    let dark_alk_before = dark.full_state().water.alkalinity_meq_total;

    lit.step_hours(1)?;
    dark.step_hours(1)?;

    assert!(
        (lit.full_state().water.alkalinity_meq_total - lit_alk_before).abs() < 1e-12,
        "lit-hour DIC changes should not directly change alkalinity"
    );
    assert!(
        (dark.full_state().water.alkalinity_meq_total - dark_alk_before).abs() < 1e-12,
        "dark-hour DIC changes should not directly change alkalinity"
    );

    Ok(())
}

#[test]
fn nitrification_lowers_alkalinity_and_ph() -> Result<(), tank_core::SimError> {
    // Tank with active nitrification
    let mut nitrifying_state = TankState::new(SimSeed(4100));
    let vol = nitrifying_state.water_volume_l();
    nitrifying_state.water.ammonia_total_mg_n_total = 2.0 * vol; // 2 mg/L TAN
    nitrifying_state.microbe.ammonia_oxidizer_biomass_g = 0.2;
    nitrifying_state.microbe.nitrite_oxidizer_biomass_g = 0.15;
    nitrifying_state.microbe.comammox_biomass_g = 0.03;
    nitrifying_state.filter_state.biofilter_maturity_index = 0.6;
    nitrifying_state.process_params = ProcessParams::default();

    // Control: identical but with zero nitrifier biomass
    let mut control_state = nitrifying_state.clone();
    control_state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    control_state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    control_state.microbe.comammox_biomass_g = 0.0;

    let mut nitrifying = Engine::from_parts(nitrifying_state, vec![]);
    let mut control = Engine::from_parts(control_state, vec![]);

    let alk_before = nitrifying.full_state().water.alkalinity_meq_total;

    nitrifying.step_hours(48)?;
    control.step_hours(48)?;

    let alk_nitrifying = nitrifying.full_state().water.alkalinity_meq_total;
    let alk_control = control.full_state().water.alkalinity_meq_total;
    let ph_nitrifying = nitrifying.full_state().water.ph;
    let ph_control = control.full_state().water.ph;

    // Active nitrification should lower alkalinity relative to control
    assert!(
        alk_nitrifying < alk_control,
        "Nitrifying tank ({alk_nitrifying:.4}) should have lower alkalinity than control ({alk_control:.4})"
    );
    assert!(
        alk_nitrifying < alk_before,
        "Alkalinity should decrease from nitrification"
    );

    // Active nitrification should lower pH relative to control
    assert!(
        ph_nitrifying < ph_control,
        "Nitrifying tank pH ({ph_nitrifying:.3}) should be lower than control ({ph_control:.3})"
    );

    // pH must still be within invariant bounds
    assert!((5.5..=8.5).contains(&ph_nitrifying));

    Ok(())
}

// ---------------------------------------------------------------------------
// Additional solver unit tests
// ---------------------------------------------------------------------------

#[test]
fn solver_reference_case_25c() -> Result<(), tank_core::SimError> {
    // DIC = 20 mg C/L, Alk = 1.5 meq/L, T = 25°C in a 20L tank.
    // Reference: Henderson-Hasselbalch with pKa1 ≈ 6.351 gives pH ≈ 7.30.
    let volume_l = 20.0;
    let dic_total = 20.0 * volume_l;
    let alk_total = 1.5 * volume_l;
    let eq = solve_carbonate_equilibrium(dic_total, alk_total, 25.0, volume_l);

    assert!(
        (eq.ph - 7.30).abs() < 0.05,
        "expected pH ≈ 7.30, got {:.3}",
        eq.ph
    );

    // Species should sum to DIC (in mmol/L).
    let dic_mmol = 20.0 / 12.0;
    let species_sum = eq.co2_aq_mmol_per_l + eq.hco3_mmol_per_l + eq.co3_mmol_per_l;
    assert!(
        (species_sum - dic_mmol).abs() < 0.001,
        "species sum {species_sum:.4} should equal DIC {dic_mmol:.4}"
    );

    // HCO3- should dominate at pH ~7.3.
    assert!(eq.hco3_mmol_per_l > eq.co2_aq_mmol_per_l);
    assert!(eq.hco3_mmol_per_l > eq.co3_mmol_per_l);

    Ok(())
}

#[test]
fn solver_high_dic_low_ph() -> Result<(), tank_core::SimError> {
    // DIC = 40 mg C/L, Alk = 2.0 meq/L, T = 25°C → more CO2, lower pH.
    let volume_l = 20.0;
    let eq = solve_carbonate_equilibrium(40.0 * volume_l, 2.0 * volume_l, 25.0, volume_l);

    assert!(
        eq.ph < 7.0,
        "high DIC relative to Alk should give pH < 7.0, got {:.3}",
        eq.ph
    );
    assert!(eq.co2_aq_mmol_per_l > 0.0);

    Ok(())
}

#[test]
fn solver_temperature_response() -> Result<(), tank_core::SimError> {
    // Same DIC/Alk at 15°C vs 35°C. Higher temp → slightly lower pH.
    let volume_l = 20.0;
    let dic_total = 20.0 * volume_l;
    let alk_total = 1.5 * volume_l;

    let eq_cold = solve_carbonate_equilibrium(dic_total, alk_total, 15.0, volume_l);
    let eq_warm = solve_carbonate_equilibrium(dic_total, alk_total, 35.0, volume_l);

    assert!(
        eq_cold.ph > eq_warm.ph,
        "cold pH ({:.3}) should be > warm pH ({:.3})",
        eq_cold.ph,
        eq_warm.ph
    );

    assert!((6.0..=9.0).contains(&eq_cold.ph));
    assert!((6.0..=9.0).contains(&eq_warm.ph));

    Ok(())
}

#[test]
fn solver_edge_case_zero_volume() {
    let eq = solve_carbonate_equilibrium(100.0, 10.0, 25.0, 0.0);
    assert_eq!(eq.ph, 7.0);
    assert_eq!(eq.co2_aq_mmol_per_l, 0.0);
}

#[test]
fn solver_edge_case_zero_dic() {
    let eq = solve_carbonate_equilibrium(0.0, 30.0, 25.0, 20.0);
    assert_eq!(eq.ph, 8.5);
    assert_eq!(eq.co2_aq_mmol_per_l, 0.0);
    assert_eq!(eq.hco3_mmol_per_l, 0.0);
    assert_eq!(eq.co3_mmol_per_l, 0.0);
}

#[test]
fn solver_edge_case_zero_dic_and_zero_alkalinity_stays_neutral() {
    let eq = solve_carbonate_equilibrium(0.0, 0.0, 25.0, 20.0);
    assert_eq!(eq.ph, 7.0);
    assert_eq!(eq.co2_aq_mmol_per_l, 0.0);
}

#[test]
fn solver_edge_case_zero_alkalinity() {
    let eq = solve_carbonate_equilibrium(400.0, 0.0, 25.0, 20.0);
    assert_eq!(eq.ph, 5.5);
    let dic_mmol = 20.0 / 12.0;
    let species_sum = eq.co2_aq_mmol_per_l + eq.hco3_mmol_per_l + eq.co3_mmol_per_l;
    assert!((species_sum - dic_mmol).abs() < 0.001);
    assert!(eq.co2_aq_mmol_per_l > eq.hco3_mmol_per_l);
    assert!(eq.hco3_mmol_per_l > 0.0);
}

#[test]
fn solver_edge_case_extreme_high_buffer_clamps_to_storage_ceiling() {
    let volume_l = 20.0;
    let eq = solve_carbonate_equilibrium(5.0 * volume_l, 8.0 * volume_l, 25.0, volume_l);
    assert_eq!(eq.ph, 8.5);
    let species_sum = eq.co2_aq_mmol_per_l + eq.hco3_mmol_per_l + eq.co3_mmol_per_l;
    let dic_mmol = 5.0 / 12.0;
    assert!((species_sum - dic_mmol).abs() < 1e-6);
}

#[test]
fn solver_deterministic() {
    let eq1 = solve_carbonate_equilibrium(400.0, 30.0, 24.0, 20.0);
    let eq2 = solve_carbonate_equilibrium(400.0, 30.0, 24.0, 20.0);
    assert_eq!(eq1, eq2);
}

#[test]
fn solver_species_conservation_sweep() -> Result<(), tank_core::SimError> {
    let volume_l = 20.0;
    for dic_mg_l in [2.0, 10.0, 20.0, 40.0] {
        for alk_meq_l in [0.25, 1.0, 1.5, 3.0] {
            for temp in [15.0, 25.0, 35.0] {
                let eq = solve_carbonate_equilibrium(
                    dic_mg_l * volume_l,
                    alk_meq_l * volume_l,
                    temp,
                    volume_l,
                );
                let dic_mmol = dic_mg_l / 12.0;
                let species_sum = eq.co2_aq_mmol_per_l + eq.hco3_mmol_per_l + eq.co3_mmol_per_l;

                assert!(
                    (species_sum - dic_mmol).abs() < 0.01,
                    "conservation failed: DIC={dic_mg_l}, Alk={alk_meq_l}, T={temp}: \
                     sum={species_sum:.4} vs DIC={dic_mmol:.4}"
                );

                assert!(
                    eq.ph.is_finite() && eq.ph >= 5.5 && eq.ph <= 8.5,
                    "pH out of storage bounds: DIC={dic_mg_l}, Alk={alk_meq_l}, T={temp}: pH={:.3}",
                    eq.ph
                );
            }
        }
    }

    Ok(())
}

#[test]
fn solver_no_nan_or_inf_in_sweep() {
    let volume_l = 20.0;
    let extremes = [
        (0.01, 0.01, 15.0),
        (0.01, 10.0, 35.0),
        (100.0, 0.01, 15.0),
        (100.0, 10.0, 35.0),
        (0.001, 0.001, 25.0),
    ];
    for (dic_mg_l, alk_meq_l, temp) in extremes {
        let eq =
            solve_carbonate_equilibrium(dic_mg_l * volume_l, alk_meq_l * volume_l, temp, volume_l);
        assert!(
            eq.ph.is_finite(),
            "pH NaN/Inf at DIC={dic_mg_l}, Alk={alk_meq_l}"
        );
        assert!(eq.co2_aq_mmol_per_l.is_finite());
        assert!(eq.hco3_mmol_per_l.is_finite());
        assert!(eq.co3_mmol_per_l.is_finite());
    }
}

#[test]
fn resolve_updates_ph_and_bicarbonate() {
    let volume_l = 20.0;
    let mut water = WaterState::default_for_volume_l(volume_l);

    water.dissolved_inorganic_carbon_mg_c_total = 20.0 * volume_l;
    water.alkalinity_meq_total = 1.5 * volume_l;
    water.temperature_c = 25.0;
    water.ph = 0.0;
    water.bicarbonate_mg_total = 0.0;

    resolve_carbonate_state(&mut water, volume_l);

    assert!(
        (water.ph - 7.30).abs() < 0.05,
        "resolve should set pH ≈ 7.30, got {:.3}",
        water.ph
    );
    assert!(
        water.bicarbonate_mg_total > 0.0,
        "resolve should set positive bicarbonate"
    );
}

#[test]
fn volume_rescale_rederives_cached_bicarbonate() {
    let old_volume_l = 20.0;
    let new_volume_l = 10.0;
    let mut water = WaterState::default_for_volume_l(old_volume_l);
    water.dissolved_inorganic_carbon_mg_c_total = 18.0 * old_volume_l;
    water.alkalinity_meq_total = 1.8 * old_volume_l;
    water.temperature_c = 25.0;
    water.bicarbonate_mg_total = 1.0;

    water.rescale_totals_for_volume(old_volume_l, new_volume_l);

    let expected = solve_carbonate_equilibrium(
        water.dissolved_inorganic_carbon_mg_c_total,
        water.alkalinity_meq_total,
        water.temperature_c,
        new_volume_l,
    );
    assert!((water.ph - expected.ph).abs() < 1e-12);
    assert!(
        (water.bicarbonate_mg_total - expected.hco3_mmol_per_l * 61.0 * new_volume_l).abs() < 1e-9
    );
}

#[test]
fn photosynthesis_raises_ph_without_changing_alkalinity() -> Result<(), tank_core::SimError> {
    let state = chemistry_state(SimSeed(4200), 12);
    let alk_before = state.water.alkalinity_meq_total;
    let ph_before = state.water.ph;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let alk_after = engine.full_state().water.alkalinity_meq_total;
    let ph_after = engine.full_state().water.ph;

    assert!(
        (alk_after - alk_before).abs() < 1e-12,
        "photosynthesis should not change alkalinity"
    );
    assert!(
        ph_after > ph_before,
        "photosynthesis should raise pH: before={ph_before:.3}, after={ph_after:.3}"
    );

    Ok(())
}
