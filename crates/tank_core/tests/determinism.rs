use tank_core::{Engine, PlayerAction, SimSeed, SimulationEngine};

#[test]
fn determinism_same_seed() -> Result<(), tank_core::SimError> {
    let left_state = tank_scenarios::seeded_state(SimSeed(7), "medium_planted")
        .expect("scenario should materialize");
    let right_state = tank_scenarios::seeded_state(SimSeed(7), "medium_planted")
        .expect("scenario should materialize");

    let mut left = Engine::from_parts(left_state, vec![]);
    let mut right = Engine::from_parts(right_state, vec![]);

    run_schedule(&mut left)?;
    run_schedule(&mut right)?;

    let left_json =
        serde_json::to_string(&left.snapshot()).expect("snapshot serialization should succeed");
    let right_json =
        serde_json::to_string(&right.snapshot()).expect("snapshot serialization should succeed");

    assert_eq!(left_json, right_json);
    assert_eq!(left.full_state(), right.full_state());

    Ok(())
}

fn run_schedule(engine: &mut Engine) -> Result<(), tank_core::SimError> {
    for hour in 0..720_u32 {
        if hour % 12 == 0 {
            engine.apply_action(PlayerAction::Feed {
                grams: if hour % 48 == 0 { 0.28 } else { 0.18 },
            })?;
        }

        if hour > 0 && hour % (24 * 5) == 0 {
            engine.apply_action(PlayerAction::WaterChangePercent {
                percent: if hour % (24 * 10) == 0 { 30.0 } else { 18.0 },
                source_profile_id: if hour % (24 * 15) == 0 {
                    "hard_shrimp".to_string()
                } else {
                    "moderate".to_string()
                },
            })?;
        }

        match hour {
            96 => engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 28.0 })?,
            240 => {
                engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 25.0 })?
            }
            420 => {
                engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 27.5 })?
            }
            600 => {
                engine.apply_action(PlayerAction::ChangeAmbientTemperature { target_c: 24.5 })?
            }
            _ => {}
        }

        engine.step_hours(1)?;
    }

    Ok(())
}
