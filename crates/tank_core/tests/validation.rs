use tank_core::{Engine, PlayerAction, SimError, SimSeed, SimulationEngine};

#[test]
fn action_validation() {
    let mut engine = Engine::new(SimSeed(9));

    assert_eq!(
        engine.apply_action(PlayerAction::Feed { grams: -0.1 }),
        Err(SimError::NegativeValue {
            field: "grams",
            value: -0.1,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::WaterChangePercent {
            percent: 120.0,
            source_profile_id: "moderate".to_string(),
        }),
        Err(SimError::PercentOutOfRange {
            field: "percent",
            value: 120.0,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::TrimPlants { fraction: 1.1 }),
        Err(SimError::FractionOutOfRange {
            field: "fraction",
            value: 1.1,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::CleanFilter { intensity: -0.5 }),
        Err(SimError::FractionOutOfRange {
            field: "intensity",
            value: -0.5,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::ChangeLightIntensity {
            intensity_index: 1.2,
        }),
        Err(SimError::FractionOutOfRange {
            field: "intensity_index",
            value: 1.2,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 0.0 }),
        Err(SimError::TemperatureTooLow {
            field: "target_c",
            value: 0.0,
        })
    );
}

#[test]
fn action_queuing() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::new(SimSeed(12));

    engine.apply_action(PlayerAction::RemoveShrimp { count: 1 })?;
    engine.apply_action(PlayerAction::AddShrimp { count: 1 })?;
    engine.step_hours(1)?;

    assert_eq!(engine.full_state().animal.adults_count, 1);

    Ok(())
}
