use tank_core::{
    enforce_invariants, EggCohort, Engine, PlayerAction, SimError, SimSeed, SimulationEngine,
};

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
        engine.apply_action(PlayerAction::TrimPlantsAndRemove { fraction: 1.1 }),
        Err(SimError::FractionOutOfRange {
            field: "fraction",
            value: 1.1,
        })
    );
    assert_eq!(
        engine.apply_action(PlayerAction::TrimPlantsAndLeaveCuttings { fraction: -0.1 }),
        Err(SimError::FractionOutOfRange {
            field: "fraction",
            value: -0.1,
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

    // Add shrimp first, then remove — validates state-aware queuing
    engine.apply_action(PlayerAction::AddShrimp { count: 2 })?;
    engine.apply_action(PlayerAction::RemoveShrimp { count: 1 })?;
    engine.step_hours(1)?;

    assert_eq!(engine.full_state().animal.adult.count, 1);

    Ok(())
}

#[test]
fn apply_action_rejects_bad_source_profile() {
    let mut engine = Engine::new(SimSeed(13));

    let result = engine.apply_action(PlayerAction::WaterChangePercent {
        percent: 25.0,
        source_profile_id: "nonexistent".to_string(),
    });

    assert_eq!(
        result,
        Err(SimError::UnknownSourceProfile {
            id: "nonexistent".to_string(),
        })
    );
    assert!(
        engine.queued_actions().is_empty(),
        "invalid water change should not be enqueued"
    );
}

#[test]
fn step_hours_rejects_invalid_shrimp_assimilation_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(14));
    state.process_params.shrimp_assimilation_efficiency = 1.2;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.shrimp_assimilation_efficiency",
            value: 1.2,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_leaves_state_untouched_when_clamps_precede_a_validation_error() {
    let mut state = tank_core::TankState::new(SimSeed(16));
    state.water.dissolved_oxygen_mg_total = -3.0;
    state.water.ph = 9.2;
    state.hardware.light.intensity_index = 1.4;
    state.animal.egg_progress_days = -2.0;
    state.animal.egg_cohorts.push(tank_core::EggCohort {
        count: 0,
        progress_days: 4.0,
    });
    state.process_params.shrimp_assimilation_efficiency = 1.2;
    state.refresh_habitat_registry();
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.shrimp_assimilation_efficiency",
            value: 1.2,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_zero_shrimp_assimilation_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(16));
    state.process_params.shrimp_assimilation_efficiency = 0.0;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.shrimp_assimilation_efficiency",
            value: 0.0,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_unity_shrimp_assimilation_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(17));
    state.process_params.shrimp_assimilation_efficiency = 1.0;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.shrimp_assimilation_efficiency",
            value: 1.0,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_non_positive_shrimp_o2_ratio_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(15));
    state.process_params.shrimp_o2_per_mg_c_respired = 0.0;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.shrimp_o2_per_mg_c_respired",
            value: 0.0,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_out_of_range_death_biomass_fraction_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(19));
    state.process_params.death_biomass_to_detritus_fraction = 1.2;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.death_biomass_to_detritus_fraction",
            value: 1.2,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_sub_unity_death_biomass_fraction_without_export_path() {
    let mut state = tank_core::TankState::new(SimSeed(20));
    state.process_params.death_biomass_to_detritus_fraction = 0.5;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "process.death_biomass_to_detritus_fraction",
            value: 0.5,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_read_only_validation_does_not_preserve_partial_clamps() {
    let mut state = tank_core::TankState::new(SimSeed(18));
    state.water.ph = 9.2;
    state.animal.adult.reserve_g = -0.01;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "animal.adult.reserve_g",
            value: -0.01,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn enforce_invariants_repairs_shrimp_cohort_bookkeeping() -> Result<(), SimError> {
    let mut state = tank_core::TankState::new(SimSeed(22));
    state.animal.adult.count = 4;
    state.animal.berried_females_count = 3;
    state.animal.egg_progress_days = 6.0;
    state.animal.egg_cohorts = vec![
        EggCohort {
            count: 2,
            progress_days: 4.0,
        },
        EggCohort {
            count: 0,
            progress_days: 9.0,
        },
    ];
    state.animal.juvenile.count = 2;
    state.animal.juvenile.maturation_accum = 9.0;
    state.animal.sub_adult.maturation_accum = 1.5;

    enforce_invariants(&mut state)?;

    assert_eq!(
        state
            .animal
            .egg_cohorts
            .iter()
            .map(|cohort| cohort.count)
            .sum::<u32>(),
        3
    );
    assert!(state
        .animal
        .egg_cohorts
        .iter()
        .any(|cohort| cohort.count == 1 && (cohort.progress_days - 6.0).abs() < 1e-12));
    assert_eq!(state.animal.egg_progress_days, 6.0);
    assert_eq!(state.animal.juvenile.maturation_accum, 2.0);
    assert_eq!(state.animal.sub_adult.maturation_accum, 0.0);

    Ok(())
}
