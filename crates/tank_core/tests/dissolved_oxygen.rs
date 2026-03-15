use tank_core::{
    systems::{light::is_light_on, temperature::do_sat_mg_l},
    Engine, ProcessParams, SimSeed, SimulationEngine, TankState,
};

fn oxygen_test_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.hardware.light.photoperiod_hours = 8.0;
    state.hardware.light.intensity_index = 1.0;
    state.hardware.aeration.enabled = false;
    state.hardware.aeration.intensity = 0.0;
    state.animal.adults_count = 30;
    state.algae.periphyton_biomass_g = 3.0;
    state.plant_guilds[0].biomass_g = 18.0;
    state.plant_guilds[1].biomass_g = 10.0;
    state.process_params = ProcessParams {
        reaeration_kla_base: 0.08,
        aeration_kla_boost: 1.2,
        background_bod_mg_o2_per_g_biomass_per_hour: 0.24,
        plant_photosynthesis_o2_mg_per_g_per_hour: 0.65,
        respiration_dic_rate_mg_c_per_g_per_hour: 0.10,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.18,
        ..ProcessParams::default()
    };
    state
}

#[test]
fn dissolved_oxygen_dips_at_night_relative_to_lit_hours() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(oxygen_test_state(SimSeed(3000)), vec![]);
    let photoperiod = engine.full_state().hardware.light.photoperiod_hours;

    let mut night_values = Vec::new();
    let mut lit_values = Vec::new();

    for _ in 0..48 {
        let hour = engine.full_state().environment.hour_of_day;
        engine.step_hours(1)?;
        let do_mg_l = engine.snapshot().do_mg_l;
        if is_light_on(hour, photoperiod) {
            lit_values.push(do_mg_l);
        } else {
            night_values.push(do_mg_l);
        }
    }

    let lit_mean = lit_values.iter().sum::<f64>() / lit_values.len() as f64;
    let night_mean = night_values.iter().sum::<f64>() / night_values.len() as f64;

    assert!(
        night_mean < lit_mean,
        "night DO should be lower than lit DO: night={night_mean:.2}, lit={lit_mean:.2}"
    );

    Ok(())
}

#[test]
fn aeration_recovers_do_faster_than_passive_exchange() -> Result<(), tank_core::SimError> {
    let mut base_state = oxygen_test_state(SimSeed(3100));
    base_state.water.dissolved_oxygen_mg_total = 2.0 * base_state.geometry.water_volume_l();
    base_state.process_params.background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    base_state.process_params.plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    base_state.algae.periphyton_biomass_g = 0.0;
    for plant in &mut base_state.plant_guilds {
        plant.biomass_g = 0.0;
    }
    base_state.animal.adults_count = 0;

    let mut no_aeration = base_state.clone();
    no_aeration.hardware.aeration.enabled = false;

    let mut high_aeration = base_state;
    high_aeration.hardware.aeration.enabled = true;
    high_aeration.hardware.aeration.intensity = 1.0;

    let mut passive_engine = Engine::from_parts(no_aeration, vec![]);
    let mut aerated_engine = Engine::from_parts(high_aeration, vec![]);

    passive_engine.step_hours(6)?;
    aerated_engine.step_hours(6)?;

    assert!(
        aerated_engine.snapshot().do_mg_l > passive_engine.snapshot().do_mg_l,
        "aeration should accelerate recovery: passive={:.2}, aerated={:.2}",
        passive_engine.snapshot().do_mg_l,
        aerated_engine.snapshot().do_mg_l
    );

    Ok(())
}

#[test]
fn reaeration_converges_toward_saturation() -> Result<(), tank_core::SimError> {
    let mut state = oxygen_test_state(SimSeed(3200));
    state.water.dissolved_oxygen_mg_total = 0.0;
    state.process_params.background_bod_mg_o2_per_g_biomass_per_hour = 0.0;
    state.process_params.plant_photosynthesis_o2_mg_per_g_per_hour = 0.0;
    state.process_params.reaeration_kla_base = 0.5;
    state.algae.periphyton_biomass_g = 0.0;
    for plant in &mut state.plant_guilds {
        plant.biomass_g = 0.0;
    }
    state.animal.adults_count = 0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let do_sat = do_sat_mg_l(engine.full_state().water.temperature_c);
    let do_now = engine.snapshot().do_mg_l;
    assert!(do_now > 0.0, "reaeration should raise DO above zero");
    assert!(
        (do_sat - do_now).abs() < 0.25,
        "DO should converge toward saturation: do={do_now:.2}, sat={do_sat:.2}"
    );

    Ok(())
}
