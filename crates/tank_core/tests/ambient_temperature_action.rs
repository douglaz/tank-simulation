use tank_core::{Engine, PlayerAction, ProcessParams, SimSeed, SimulationEngine, TankState};

/// Helper: build a state with heater enabled at setpoint 24°C, ambient 24°C.
fn stable_24c_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    state.hardware.heater.enabled = true;
    state.hardware.heater.setpoint_c = 24.0;
    state.hardware.heater.max_watts = 50.0;
    state.process_params = ProcessParams::default();
    state
}

#[test]
fn ambient_change_causes_gradual_warming() -> Result<(), tank_core::SimError> {
    let state = stable_24c_state(SimSeed(1000));
    let mut engine = Engine::from_parts(state, vec![]);

    // Change ambient from 24°C to 31°C
    engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;
    engine.step_hours(1)?;

    let snap_1h = engine.snapshot();
    // After 1 hour, water should have warmed but NOT jumped to 31°C
    assert!(
        snap_1h.water_temp_c > 24.0,
        "Water should warm: got {:.3}",
        snap_1h.water_temp_c
    );
    assert!(
        snap_1h.water_temp_c < 31.0,
        "Water should NOT jump to ambient: got {:.3}",
        snap_1h.water_temp_c
    );

    // After many hours, it should approach 31°C
    engine.step_hours(200)?;
    let snap_200h = engine.snapshot();
    assert!(
        snap_200h.water_temp_c > 30.0,
        "After 200h, water should be near ambient 31°C: got {:.3}",
        snap_200h.water_temp_c
    );

    Ok(())
}

#[test]
fn do_saturation_decreases_with_warming() -> Result<(), tank_core::SimError> {
    let state = stable_24c_state(SimSeed(1100));
    let mut engine = Engine::from_parts(state, vec![]);

    let do_sat_before = engine.snapshot().do_sat_mg_l;

    // Warm to 31°C and run
    engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;
    engine.step_hours(200)?;

    let do_sat_after = engine.snapshot().do_sat_mg_l;
    assert!(
        do_sat_after < do_sat_before,
        "DO saturation should decrease with warming: before={do_sat_before:.2}, after={do_sat_after:.2}"
    );

    Ok(())
}

#[test]
fn heater_output_zero_when_above_setpoint() -> Result<(), tank_core::SimError> {
    let state = stable_24c_state(SimSeed(1200));
    let mut engine = Engine::from_parts(state, vec![]);

    // Change ambient above heater setpoint
    engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;
    engine.step_hours(200)?;

    let snap = engine.snapshot();
    // Water should be well above setpoint by now
    assert!(
        snap.water_temp_c > 24.0,
        "Water should be above setpoint"
    );
    assert!(
        snap.last_heater_output_w == 0.0,
        "Heater should be off when water > setpoint: got {:.2}",
        snap.last_heater_output_w
    );

    Ok(())
}

#[test]
fn heater_never_cools_tank() -> Result<(), tank_core::SimError> {
    let mut state = stable_24c_state(SimSeed(1300));
    // Set water above setpoint
    state.water.temperature_c = 28.0;
    state.environment.ambient_temp_c = 28.0;
    state.hardware.heater.setpoint_c = 24.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let snap = engine.snapshot();
    assert!(
        snap.last_heater_output_w == 0.0,
        "Heater should not fire when water is above setpoint"
    );
    // Temperature should not decrease due to heater (ambient = water temp)
    assert!(
        snap.water_temp_c >= 27.9,
        "Heater should never cool: got {:.3}",
        snap.water_temp_c
    );

    Ok(())
}

#[test]
fn heater_fires_when_below_setpoint() -> Result<(), tank_core::SimError> {
    let mut state = stable_24c_state(SimSeed(1400));
    state.water.temperature_c = 20.0;
    state.environment.ambient_temp_c = 20.0;
    state.hardware.heater.setpoint_c = 24.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let snap = engine.snapshot();
    assert!(
        snap.last_heater_output_w > 0.0,
        "Heater should fire when water is below setpoint - deadband/2"
    );
    assert!(
        snap.water_temp_c > 20.0,
        "Water should warm from heater: got {:.3}",
        snap.water_temp_c
    );

    Ok(())
}
