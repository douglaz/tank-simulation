use tank_core::{Engine, PlayerAction, SimSeed, SimulationEngine, TankState};

#[test]
fn filter_cleanliness_decay() -> Result<(), tank_core::SimError> {
    let mut fed_state = TankState::new(SimSeed(10_601));
    fed_state.hardware.filter.cleanliness_index = 1.0;
    fed_state.hardware.filter.enabled = true;
    fed_state.hardware.filter.flow_lph = 200.0;
    fed_state.animal.adults_count = 12;

    let unfed_state = fed_state.clone();

    let mut fed = Engine::from_parts(fed_state, vec![]);
    let mut unfed = Engine::from_parts(unfed_state, vec![]);

    for _ in 0..30 {
        fed.apply_action(PlayerAction::Feed { grams: 0.3 })?;
        fed.step_hours(24)?;
        unfed.step_hours(24)?;
    }

    let fed_cleanliness = fed.full_state().hardware.filter.cleanliness_index;
    let unfed_cleanliness = unfed.full_state().hardware.filter.cleanliness_index;
    let fed_clogging = fed.full_state().filter_state.clogging_index;

    assert!(
        fed_cleanliness < 0.99,
        "moderate feeding over 30 days should measurably reduce cleanliness: {fed_cleanliness:.4}"
    );
    assert!(
        fed_cleanliness < unfed_cleanliness,
        "feeding should foul the filter faster than an unfed tank: fed={fed_cleanliness:.4}, unfed={unfed_cleanliness:.4}"
    );
    assert!(
        fed_clogging > 0.0,
        "clogging should accumulate during normal simulation"
    );

    Ok(())
}
