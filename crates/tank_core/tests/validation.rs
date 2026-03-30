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
fn add_shrimp_reseeds_empty_adult_stage() -> Result<(), tank_core::SimError> {
    let mut state = tank_core::TankState::new(SimSeed(15));
    state.animal.adult.count = 0;
    state.animal.adult.reserve_g = 1.25;
    state.animal.adult.condition_index = 0.1;
    state.animal.adult.maturation_accum = 4.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.apply_action(PlayerAction::AddShrimp { count: 2 })?;
    engine.step_hours(1)?;

    let adult = &engine.full_state().animal.adult;
    assert_eq!(adult.count, 2);
    assert_eq!(adult.reserve_g, 0.0);
    assert_eq!(adult.condition_index, 0.8);
    assert_eq!(adult.maturation_accum, 0.0);

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
fn step_hours_rejects_mismatched_shrimp_cohort_bookkeeping() {
    let mut state = tank_core::TankState::new(SimSeed(22));
    state.animal.adult.count = 4;
    state.animal.berried_females_count = 3;
    state.animal.egg_progress_days = 6.0;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 2,
        progress_days: 4.0,
    }];
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "animal.egg_cohort_count_total",
            value: 2.0,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn enforce_invariants_still_trims_zero_count_cohorts_and_clamps_stage_progress(
) -> Result<(), SimError> {
    let mut state = tank_core::TankState::new(SimSeed(23));
    state.animal.adult.count = 4;
    state.animal.berried_females_count = 2;
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

    assert_eq!(state.animal.egg_cohorts.len(), 1);
    assert_eq!(state.animal.egg_cohort_count_total(), 2);
    assert_eq!(state.animal.egg_progress_days, 6.0);
    assert_eq!(state.animal.juvenile.maturation_accum, 2.0);
    assert_eq!(state.animal.sub_adult.maturation_accum, 0.0);

    Ok(())
}

#[test]
fn step_hours_rejects_invalid_molt_mineral_parameters_before_simulation() {
    let invalid_cases = [
        ("shrimp_params.ca_min_mg_per_l", -1.0),
        ("shrimp_params.mg_min_mg_per_l", 0.0),
        ("shrimp_params.molt_reserve_fraction", 0.0),
        ("shrimp_params.molt_condition_weight", 1.2),
        ("shrimp_params.molt_reserve_weight", -0.1),
        (
            "shrimp_params.failed_molt_accum_increase_per_failed_stage",
            -0.1,
        ),
        (
            "shrimp_params.failed_molt_accum_recovery_per_successful_stage",
            -0.1,
        ),
        ("shrimp_params.failed_molt_stress_blend", 1.2),
        ("shrimp_params.molt_failure_poor_condition_threshold", 1.2),
        ("shrimp_params.molt_failure_instability_threshold", -0.1),
        ("shrimp_params.temp_condition_low_divisor_c", 0.0),
        ("shrimp_params.temp_condition_high_divisor_c", 0.0),
        ("shrimp_params.temp_condition_min_factor", 1.2),
        ("shrimp_params.molt_stress_warning_threshold", 1.2),
        ("shrimp_params.molt_stress_mortality_threshold", 1.2),
        ("shrimp_params.molt_stress_mineral_gh_weight", 1.2),
        ("shrimp_params.molt_stress_mineral_ca_weight", -0.1),
        ("shrimp_params.molt_stress_mineral_mg_weight", 1.2),
        ("shrimp_params.molt_stress_pressure_hourly_weight", 1.2),
        ("shrimp_params.molt_stress_condition_midpoint", 1.2),
        ("shrimp_params.molt_stress_thermal_cap", 1.2),
        ("shrimp_params.molt_stress_rise_smoothing", 1.2),
        ("shrimp_params.molt_stress_decay_smoothing", -0.1),
        ("shrimp_params.molt_gh_excess_penalty_divisor", 0.0),
        ("shrimp_params.molt_mineral_factor_floor", 1.2),
        ("shrimp_params.juvenile_molt_interval_days", 0.0),
        ("shrimp_params.sub_adult_molt_interval_days", 0.0),
        ("shrimp_params.molt_success_threshold", 1.2),
        ("shrimp_params.critical_molt_gh_ratio", 1.2),
        ("shrimp_params.chloride_protection_factor", -1.0),
        ("shrimp_params.nh3_stress_threshold_mg_n_per_l", -0.1),
        ("shrimp_params.nh3_stress_response_scale", -0.1),
        ("shrimp_params.condition_do_reference_mg_l", 0.0),
        ("shrimp_params.condition_nh3_sensitivity", -0.1),
        ("shrimp_params.condition_nitrite_sensitivity", -0.1),
        ("shrimp_params.condition_hourly_stress_penalty_weight", 1.2),
        ("shrimp_params.low_temp_repro_ramp_width_c", 0.0),
        ("shrimp_params.tan_repro_full_suppression_mg_n_per_l", 0.0),
        ("shrimp_params.no2_repro_full_suppression_mg_n_per_l", 0.0),
        ("shrimp_params.egg_drop_max_probability", 1.2),
        ("shrimp_params.egg_oxygen_reference_mg_l", 0.0),
        ("shrimp_params.reproductive_readiness_smoothing", -0.1),
        ("shrimp_params.full_clutch_condition_threshold", 1.2),
    ];

    for (index, (field, value)) in invalid_cases.into_iter().enumerate() {
        let mut state = tank_core::TankState::new(SimSeed(30 + index as u64));
        match field {
            "shrimp_params.ca_min_mg_per_l" => state.shrimp_params.ca_min_mg_per_l = value,
            "shrimp_params.mg_min_mg_per_l" => state.shrimp_params.mg_min_mg_per_l = value,
            "shrimp_params.molt_reserve_fraction" => {
                state.shrimp_params.molt_reserve_fraction = value;
            }
            "shrimp_params.molt_condition_weight" => {
                state.shrimp_params.molt_condition_weight = value;
            }
            "shrimp_params.molt_reserve_weight" => {
                state.shrimp_params.molt_reserve_weight = value;
            }
            "shrimp_params.failed_molt_accum_increase_per_failed_stage" => {
                state
                    .shrimp_params
                    .failed_molt_accum_increase_per_failed_stage = value;
            }
            "shrimp_params.failed_molt_accum_recovery_per_successful_stage" => {
                state
                    .shrimp_params
                    .failed_molt_accum_recovery_per_successful_stage = value;
            }
            "shrimp_params.failed_molt_stress_blend" => {
                state.shrimp_params.failed_molt_stress_blend = value;
            }
            "shrimp_params.molt_failure_poor_condition_threshold" => {
                state.shrimp_params.molt_failure_poor_condition_threshold = value;
            }
            "shrimp_params.molt_failure_instability_threshold" => {
                state.shrimp_params.molt_failure_instability_threshold = value;
            }
            "shrimp_params.temp_condition_low_divisor_c" => {
                state.shrimp_params.temp_condition_low_divisor_c = value;
            }
            "shrimp_params.temp_condition_high_divisor_c" => {
                state.shrimp_params.temp_condition_high_divisor_c = value;
            }
            "shrimp_params.temp_condition_min_factor" => {
                state.shrimp_params.temp_condition_min_factor = value;
            }
            "shrimp_params.molt_stress_warning_threshold" => {
                state.shrimp_params.molt_stress_warning_threshold = value;
            }
            "shrimp_params.molt_stress_mortality_threshold" => {
                state.shrimp_params.molt_stress_mortality_threshold = value;
            }
            "shrimp_params.molt_stress_mineral_gh_weight" => {
                state.shrimp_params.molt_stress_mineral_gh_weight = value;
            }
            "shrimp_params.molt_stress_mineral_ca_weight" => {
                state.shrimp_params.molt_stress_mineral_ca_weight = value;
            }
            "shrimp_params.molt_stress_mineral_mg_weight" => {
                state.shrimp_params.molt_stress_mineral_mg_weight = value;
            }
            "shrimp_params.molt_stress_pressure_hourly_weight" => {
                state.shrimp_params.molt_stress_pressure_hourly_weight = value;
            }
            "shrimp_params.molt_stress_condition_midpoint" => {
                state.shrimp_params.molt_stress_condition_midpoint = value;
            }
            "shrimp_params.molt_stress_thermal_cap" => {
                state.shrimp_params.molt_stress_thermal_cap = value;
            }
            "shrimp_params.molt_stress_rise_smoothing" => {
                state.shrimp_params.molt_stress_rise_smoothing = value;
            }
            "shrimp_params.molt_stress_decay_smoothing" => {
                state.shrimp_params.molt_stress_decay_smoothing = value;
            }
            "shrimp_params.molt_gh_excess_penalty_divisor" => {
                state.shrimp_params.molt_gh_excess_penalty_divisor = value;
            }
            "shrimp_params.molt_mineral_factor_floor" => {
                state.shrimp_params.molt_mineral_factor_floor = value;
            }
            "shrimp_params.juvenile_molt_interval_days" => {
                state.shrimp_params.juvenile_molt_interval_days = value;
            }
            "shrimp_params.sub_adult_molt_interval_days" => {
                state.shrimp_params.sub_adult_molt_interval_days = value;
            }
            "shrimp_params.molt_success_threshold" => {
                state.shrimp_params.molt_success_threshold = value;
            }
            "shrimp_params.critical_molt_gh_ratio" => {
                state.shrimp_params.critical_molt_gh_ratio = value;
            }
            "shrimp_params.chloride_protection_factor" => {
                state.shrimp_params.chloride_protection_factor = value;
            }
            "shrimp_params.nh3_stress_threshold_mg_n_per_l" => {
                state.shrimp_params.nh3_stress_threshold_mg_n_per_l = value;
            }
            "shrimp_params.nh3_stress_response_scale" => {
                state.shrimp_params.nh3_stress_response_scale = value;
            }
            "shrimp_params.condition_do_reference_mg_l" => {
                state.shrimp_params.condition_do_reference_mg_l = value;
            }
            "shrimp_params.condition_nh3_sensitivity" => {
                state.shrimp_params.condition_nh3_sensitivity = value;
            }
            "shrimp_params.condition_nitrite_sensitivity" => {
                state.shrimp_params.condition_nitrite_sensitivity = value;
            }
            "shrimp_params.condition_hourly_stress_penalty_weight" => {
                state.shrimp_params.condition_hourly_stress_penalty_weight = value;
            }
            "shrimp_params.low_temp_repro_ramp_width_c" => {
                state.shrimp_params.low_temp_repro_ramp_width_c = value;
            }
            "shrimp_params.tan_repro_full_suppression_mg_n_per_l" => {
                state.shrimp_params.tan_repro_full_suppression_mg_n_per_l = value;
            }
            "shrimp_params.no2_repro_full_suppression_mg_n_per_l" => {
                state.shrimp_params.no2_repro_full_suppression_mg_n_per_l = value;
            }
            "shrimp_params.egg_drop_max_probability" => {
                state.shrimp_params.egg_drop_max_probability = value;
            }
            "shrimp_params.egg_oxygen_reference_mg_l" => {
                state.shrimp_params.egg_oxygen_reference_mg_l = value;
            }
            "shrimp_params.reproductive_readiness_smoothing" => {
                state.shrimp_params.reproductive_readiness_smoothing = value;
            }
            "shrimp_params.full_clutch_condition_threshold" => {
                state.shrimp_params.full_clutch_condition_threshold = value;
            }
            _ => unreachable!(),
        }
        let expected = state.clone();

        let mut engine = Engine::from_parts(state, vec![]);
        let result = engine.step_hours(1);

        assert_eq!(result, Err(SimError::InvariantViolation { field, value }));
        assert_eq!(engine.full_state(), &expected);
    }
}

#[test]
fn step_hours_rejects_invalid_molt_condition_weight_sum_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(44));
    state.shrimp_params.molt_condition_weight = 0.8;
    state.shrimp_params.molt_reserve_weight = 0.3;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "shrimp_params.molt_condition_weight + shrimp_params.molt_reserve_weight",
            value: 1.1,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_accepts_zero_temp_condition_min_factor() {
    let mut state = tank_core::TankState::new(SimSeed(45));
    state.shrimp_params.temp_condition_min_factor = 0.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine
        .step_hours(1)
        .expect("zero temp_condition_min_factor should remain valid at runtime");

    assert_eq!(
        engine.full_state().shrimp_params.temp_condition_min_factor,
        0.0
    );
}

#[test]
fn step_hours_rejects_invalid_molt_stress_mineral_weight_sum_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(46));
    state.shrimp_params.molt_stress_mineral_gh_weight = 0.6;
    state.shrimp_params.molt_stress_mineral_ca_weight = 0.3;
    state.shrimp_params.molt_stress_mineral_mg_weight = 0.3;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::InvariantViolation {
            field: "shrimp_params.molt_stress_mineral_gh_weight + shrimp_params.molt_stress_mineral_ca_weight + shrimp_params.molt_stress_mineral_mg_weight",
            value: 1.2,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}

#[test]
fn step_hours_rejects_inverted_stage_molt_intervals_before_simulation() {
    let mut state = tank_core::TankState::new(SimSeed(35));
    state.shrimp_params.juvenile_molt_interval_days = 12.0;
    state.shrimp_params.sub_adult_molt_interval_days = 10.0;
    let expected = state.clone();

    let mut engine = Engine::from_parts(state, vec![]);
    let result = engine.step_hours(1);

    assert_eq!(
        result,
        Err(SimError::OrderingViolation {
            lower_field: "shrimp_params.juvenile_molt_interval_days",
            lower_value: 12.0,
            upper_field: "shrimp_params.sub_adult_molt_interval_days",
            upper_value: 10.0,
        })
    );
    assert_eq!(engine.full_state(), &expected);
}
