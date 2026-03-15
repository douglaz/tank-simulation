use tank_core::{
    Engine, PlayerAction, ProcessParams, SimSeed, SimulationEngine, TankGeometry, TankState,
};

/// Build a state with a specific volume (adjusting geometry) and initial conditions.
fn state_with_volume(seed: SimSeed, target_volume_l: f64) -> TankState {
    let mut state = TankState::new(seed);

    // Compute geometry: keep square footprint, adjust fill height
    // For a 10L tank: e.g., 20×20cm footprint, fill_height = 10L * 1000 / (20*20) = 25cm
    // For a 100L tank: e.g., 50×40cm footprint, fill_height = 100L * 1000 / (50*40) = 50cm
    let (length, width, fill_h) = if target_volume_l <= 20.0 {
        let side = 20.0;
        let fill = target_volume_l * 1000.0 / (side * side);
        (side, side, fill)
    } else {
        let length = 50.0;
        let width = 40.0;
        let fill = target_volume_l * 1000.0 / (length * width);
        (length, width, fill)
    };

    state.geometry = TankGeometry {
        length_cm: length,
        width_cm: width,
        height_cm: fill_h + 5.0,
        fill_height_cm: fill_h,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
    };

    // Re-initialize water for new geometry
    state.water = tank_core::WaterState::default_for_geometry(&state.geometry);
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    state.hardware.heater.enabled = false;
    state.process_params = ProcessParams::default();

    state
}

#[test]
fn smaller_tank_responds_faster_to_ambient_change() -> Result<(), tank_core::SimError> {
    let state_10l = state_with_volume(SimSeed(2000), 10.0);
    let state_100l = state_with_volume(SimSeed(2000), 100.0);

    let vol_10 = state_10l.geometry.water_volume_l();
    let vol_100 = state_100l.geometry.water_volume_l();
    assert!(
        (vol_10 - 10.0).abs() < 0.1,
        "10L tank should be ~10L, got {vol_10}"
    );
    assert!(
        (vol_100 - 100.0).abs() < 0.1,
        "100L tank should be ~100L, got {vol_100}"
    );

    let mut engine_10l = Engine::from_parts(state_10l, vec![]);
    let mut engine_100l = Engine::from_parts(state_100l, vec![]);

    // Change ambient to 31°C for both
    engine_10l.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;
    engine_100l.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;

    let target_delta = 31.0 - 24.0; // 7°C total change
    let half_delta = target_delta / 2.0; // 3.5°C

    // Find how many ticks each takes to reach half of eventual delta
    let mut ticks_10l = 0u32;
    let mut ticks_100l = 0u32;

    // Run 10L until half delta
    for tick in 1..=500 {
        engine_10l.step_hours(1)?;
        let temp = engine_10l.full_state().water.temperature_c;
        if temp - 24.0 >= half_delta {
            ticks_10l = tick;
            break;
        }
    }

    // Run 100L until half delta
    for tick in 1..=500 {
        engine_100l.step_hours(1)?;
        let temp = engine_100l.full_state().water.temperature_c;
        if temp - 24.0 >= half_delta {
            ticks_100l = tick;
            break;
        }
    }

    assert!(
        ticks_10l > 0,
        "10L tank should reach half delta within 500 ticks"
    );
    assert!(
        ticks_100l > 0,
        "100L tank should reach half delta within 500 ticks"
    );
    assert!(
        ticks_10l * 2 < ticks_100l,
        "10L tank ({ticks_10l} ticks) should reach half delta in fewer than half the ticks of 100L ({ticks_100l} ticks)"
    );

    Ok(())
}

#[test]
fn lid_slows_surface_heat_exchange() -> Result<(), tank_core::SimError> {
    let mut state_open = state_with_volume(SimSeed(2100), 50.0);
    state_open.geometry.open_top = true;

    let mut state_closed = state_with_volume(SimSeed(2100), 50.0);
    state_closed.geometry.open_top = false;

    let mut engine_open = Engine::from_parts(state_open, vec![]);
    let mut engine_closed = Engine::from_parts(state_closed, vec![]);

    // Change ambient for both
    engine_open.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;
    engine_closed.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 31.0 })?;

    engine_open.step_hours(10)?;
    engine_closed.step_hours(10)?;

    let temp_open = engine_open.full_state().water.temperature_c;
    let temp_closed = engine_closed.full_state().water.temperature_c;

    // Open top should warm faster (higher surface exchange)
    assert!(
        temp_open > temp_closed,
        "Open top ({temp_open:.3}°C) should warm faster than closed ({temp_closed:.3}°C)"
    );

    Ok(())
}
