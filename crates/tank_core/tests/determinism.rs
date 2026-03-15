use tank_core::{Engine, SimSeed, SimulationEngine};

#[test]
fn determinism_basic() -> Result<(), tank_core::SimError> {
    let mut left = Engine::new(SimSeed(7));
    let mut right = Engine::new(SimSeed(7));

    left.step_hours(24)?;
    right.step_hours(24)?;

    assert_eq!(left.snapshot(), right.snapshot());
    assert_eq!(left.full_state(), right.full_state());

    Ok(())
}
