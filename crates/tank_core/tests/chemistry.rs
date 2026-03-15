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
    let night_dic_after = night_engine.full_state().water.dissolved_inorganic_carbon_mg_c_total;
    let day_dic_after = day_engine.full_state().water.dissolved_inorganic_carbon_mg_c_total;

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
