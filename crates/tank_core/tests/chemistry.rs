use tank_core::{
    systems::chemistry::{compute_nh3_mg_l, compute_ph_from_totals},
    Engine, ProcessParams, SimSeed, SimulationEngine, TankState,
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
    state.process_params = ProcessParams {
        respiration_dic_rate_mg_c_per_g_per_hour: 0.20,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.35,
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
    let low = compute_ph_from_totals(0.01, 500.0, 20.0);
    let high = compute_ph_from_totals(12.0, 0.01, 20.0);

    assert!((5.5..=8.5).contains(&low));
    assert!((5.5..=8.5).contains(&high));
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
    let vol = nitrifying_state.geometry.water_volume_l();
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
