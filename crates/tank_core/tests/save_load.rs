use tank_core::{Engine, PlayerAction, SaveFile, SimSeed, SimulationEngine};

#[test]
fn save_load_roundtrip() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::new(SimSeed(42));
    engine.apply_action(PlayerAction::Feed { grams: 1.25 })?;
    engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 25.5 })?;
    engine.step_hours(1)?;
    engine.apply_action(PlayerAction::ChangeLightIntensity {
        intensity_index: 0.6,
    })?;

    let save = SaveFile::from_engine(&engine);
    let json = save.to_json_pretty()?;
    let restored = SaveFile::from_json(&json)?;
    let restored_engine = restored.clone().into_engine();

    assert_eq!(save, restored);
    assert_eq!(engine.full_state(), restored_engine.full_state());
    assert_eq!(engine.queued_actions(), restored_engine.queued_actions());

    Ok(())
}
