use tank_core::{
    EggCohort, Engine, EventCause, EventKind, PlayerAction, ProcessParams, SimSeed,
    SimulationEngine, TankState,
};

fn threshold_state(seed: SimSeed) -> TankState {
    let mut state = TankState::new(seed);
    let volume_l = state.water_volume_l();
    // Default DIC/Alk (20/1.5) gives pH ~7.3 from the carbonate solver.
    // Use elevated TAN (3.0 mg/L) so free-NH3 exceeds the 0.02 threshold.
    state.water.ammonia_total_mg_n_total = 3.0 * volume_l;
    state.water.nitrite_mg_n_total = 0.7 * volume_l;
    state.water.dissolved_oxygen_mg_total = 3.0 * volume_l;
    state.process_params = ProcessParams {
        reaeration_kla_base: 0.0,
        aeration_kla_boost: 0.0,
        background_bod_mg_o2_per_g_biomass_per_hour: 0.0,
        plant_photosynthesis_o2_mg_per_g_per_hour: 0.0,
        respiration_dic_rate_mg_c_per_g_per_hour: 0.0,
        photosynthesis_dic_rate_mg_c_per_g_per_hour: 0.0,
        ..ProcessParams::default()
    };
    state
}

#[test]
fn threshold_events_emit_with_cause_codes() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(threshold_state(SimSeed(5000)), vec![]);
    engine.step_hours(1)?;

    let events = &engine.full_state().event_log;
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::AmmoniaWarning));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::NitriteWarning));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::OxygenDip));
    assert!(events.iter().all(|event| !event.cause_codes.is_empty()));

    Ok(())
}

#[test]
fn threshold_event_summaries_use_explicit_chemistry_units() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(threshold_state(SimSeed(5050)), vec![]);
    engine.step_hours(1)?;

    let events = &engine.full_state().event_log;
    let ammonia = events
        .iter()
        .find(|event| event.kind == EventKind::AmmoniaWarning)
        .expect("expected ammonia warning");
    let nitrite = events
        .iter()
        .find(|event| event.kind == EventKind::NitriteWarning)
        .expect("expected nitrite warning");

    assert!(
        ammonia.summary.contains("NH3-N") && ammonia.summary.contains("mg NH3-N/L"),
        "ammonia summary should spell out the NH3-N basis: {}",
        ammonia.summary
    );
    assert!(
        nitrite.summary.contains("Nitrite-N") && nitrite.summary.contains("mg N/L"),
        "nitrite summary should spell out the N basis: {}",
        nitrite.summary
    );

    Ok(())
}

#[test]
fn nitrite_warning_summary_reports_effective_hazard_and_stress_increment(
) -> Result<(), tank_core::SimError> {
    let mut state = threshold_state(SimSeed(5075));
    let volume_l = state.water_volume_l();
    state.water.nitrite_mg_n_total = 3.0 * volume_l;
    state.water.chloride_mg_total = 30.0 * volume_l;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(1)?;

    let nitrite = engine
        .full_state()
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::NitriteWarning)
        .expect("expected nitrite warning");

    assert!(
        nitrite.summary.contains("effective hazard")
            && nitrite.summary.contains("nitrite stress +"),
        "nitrite warning should report effective hazard and stress increment: {}",
        nitrite.summary
    );

    Ok(())
}

#[test]
fn threshold_events_are_deduplicated_per_day() -> Result<(), tank_core::SimError> {
    let mut engine = Engine::from_parts(threshold_state(SimSeed(5100)), vec![]);

    engine.step_hours(6)?;
    let same_day_count = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::AmmoniaWarning)
        .count();
    assert_eq!(
        same_day_count, 1,
        "ammonia warning should dedupe within a day"
    );

    engine.step_hours(24)?;
    let next_day_count = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::AmmoniaWarning)
        .count();
    assert_eq!(next_day_count, 2, "warning should emit again on a new day");

    Ok(())
}

#[test]
fn event_generation_warm_overfed_weak_aeration() -> Result<(), tank_core::SimError> {
    // Warm, overfed, weak-aeration setup — designed to trigger AmmoniaWarning or OxygenDip
    let mut state = TankState::new(SimSeed(5200));
    let vol = state.water_volume_l();
    state.water.temperature_c = 30.0;
    state.environment.ambient_temp_c = 30.0;
    state.water.dissolved_oxygen_mg_total = 3.5 * vol;
    // Higher pH pushes NH3 fraction up at 30°C
    state.water.alkalinity_meq_total = 4.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    // Weak aeration
    state.hardware.aeration.enabled = false;
    state.hardware.light.enabled = false;
    state.plant_guilds.clear();
    state.algae.suspended_biomass_g = 0.0;
    state.algae.set_periphyton_total(0.0);
    // Low reaeration and elevated BOD allow oxygen to sag while feed mineralizes.
    state.process_params = ProcessParams {
        reaeration_kla_base: 0.0,
        aeration_kla_boost: 0.1,
        background_bod_mg_o2_per_g_biomass_per_hour: 0.15,
        ..ProcessParams::default()
    };
    // Strong decomposer biomass to mineralize feed quickly into TAN
    state.microbe.set_decomposer_total(0.5);
    // Minimal nitrification so TAN accumulates
    state.microbe.ammonia_oxidizer_biomass_g = 0.0;
    state.microbe.nitrite_oxidizer_biomass_g = 0.0;
    state.microbe.comammox_biomass_g = 0.0;
    state.filter_state.biofilter_maturity_index = 0.05;

    let mut engine = Engine::from_parts(state, vec![]);

    // Heavy overfeeding for 10 days
    for _ in 0..10 {
        engine.apply_action(PlayerAction::Feed { grams: 4.0 })?;
        engine.step_hours(24)?;
    }

    let events = &engine.full_state().event_log;
    let final_state = engine.full_state();
    let final_view = final_state.concentrations();

    // Must emit at least one of AmmoniaWarning or OxygenDip
    let has_ammonia = events.iter().any(|e| e.kind == EventKind::AmmoniaWarning);
    let has_oxygen = events.iter().any(|e| e.kind == EventKind::OxygenDip);
    assert!(
        has_ammonia || has_oxygen,
        "Warm overfed tank should emit AmmoniaWarning or OxygenDip. Events: {:?}; TAN={:.3} mg/L, DO={:.3} mg/L, pH={:.3}",
        events.iter().map(|e| &e.kind).collect::<Vec<_>>()
            ,
        final_view.tan_mg_n_per_l(),
        final_view.do_mg_per_l(),
        final_state.water.ph,
    );

    // All emitted events must have non-empty cause_codes
    let relevant: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::AmmoniaWarning | EventKind::OxygenDip | EventKind::NitriteWarning
            )
        })
        .collect();
    assert!(
        relevant.iter().all(|e| !e.cause_codes.is_empty()),
        "Warning events must have non-empty cause_codes"
    );

    Ok(())
}

#[test]
fn shrimp_berried_event_has_cause_codes() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5300));
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 3.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;
    state.algae.set_periphyton_total(3.0);
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.8;
    state.animal.reproductive_readiness_index = 0.8;
    state.microbe.set_decomposer_total(0.2);
    state.microbe.ammonia_oxidizer_biomass_g = 0.15;
    state.microbe.nitrite_oxidizer_biomass_g = 0.1;
    state.filter_state.biofilter_maturity_index = 0.5;

    let mut engine = Engine::from_parts(state, vec![]);

    for _ in 0..30 {
        engine.apply_action(PlayerAction::Feed { grams: 0.1 })?;
        engine.step_hours(24)?;
    }

    let berried_events: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::ShrimpBerried)
        .collect();

    assert!(
        !berried_events.is_empty(),
        "Should emit ShrimpBerried events in good conditions"
    );
    for e in &berried_events {
        assert!(
            !e.cause_codes.is_empty(),
            "ShrimpBerried events must have non-empty cause_codes"
        );
    }

    Ok(())
}

#[test]
fn shrimp_hatched_event_has_cause_codes() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5350));
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    let volume_l = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
    state.water.calcium_mg_total = 40.0 * volume_l;
    state.water.magnesium_mg_total = 10.0 * volume_l;
    state.water.alkalinity_meq_total = 10.0 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * volume_l;

    state.animal.adult.count = 6;
    state.animal.adult.condition_index = 0.95;
    state.animal.adult.reserve_g = 1.0;
    state.animal.berried_females_count = 2;
    state.animal.egg_progress_days = 20.0;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 2,
        progress_days: 20.0,
    }];
    state.animal.molt_stress_index = 0.0;
    state.animal.reproductive_readiness_index = 1.0;
    state.stability_tracker.instability_index = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.shrimp_params.hatch_success_base = 1.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let hatch_events: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::ShrimpHatched)
        .collect();

    assert!(
        !hatch_events.is_empty(),
        "Should emit ShrimpHatched under favorable hatch conditions"
    );
    for event in &hatch_events {
        assert!(
            !event.cause_codes.is_empty(),
            "ShrimpHatched events must have non-empty cause_codes"
        );
    }

    Ok(())
}

#[test]
fn shrimp_hatched_event_reports_limiting_causes_when_suppressed() -> Result<(), tank_core::SimError>
{
    let mut state = TankState::new(SimSeed(5351));
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    let volume_l = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 3.0 * volume_l;
    state.water.calcium_mg_total = 10.0 * volume_l;
    state.water.magnesium_mg_total = 2.0 * volume_l;
    state.water.alkalinity_meq_total = 10.0 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * volume_l;

    state.animal.adult.count = 12;
    state.animal.adult.condition_index = 0.95;
    state.animal.adult.reserve_g = 4.0;
    state.animal.berried_females_count = 6;
    state.animal.egg_progress_days = 20.0;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 6,
        progress_days: 20.0,
    }];
    state.animal.molt_stress_index = 0.0;
    state.animal.reproductive_readiness_index = 1.0;
    state.stability_tracker.instability_index = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.shrimp_params.hatch_success_base = 1.0;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let hatch_event = engine
        .full_state()
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::ShrimpHatched)
        .expect("expected a partially suppressed ShrimpHatched event");

    assert!(
        hatch_event.cause_codes.contains(&EventCause::LowOxygen),
        "Partial hatch should surface low-oxygen suppression: {:?}",
        hatch_event.cause_codes
    );
    assert!(
        hatch_event.cause_codes.contains(&EventCause::LowMinerals),
        "Partial hatch should surface low-mineral suppression: {:?}",
        hatch_event.cause_codes
    );
    assert!(
        hatch_event.summary.contains("limited by")
            && hatch_event.summary.contains("low oxygen")
            && hatch_event.summary.contains("low minerals"),
        "Partial hatch summary should describe the limiting factors: {}",
        hatch_event.summary
    );

    Ok(())
}

#[test]
fn egg_failure_event_has_cause_codes() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5400));
    state.water.temperature_c = 32.0;
    state.environment.ambient_temp_c = 32.0;
    let vol = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 2.0 * vol;
    state.water.calcium_mg_total = 5.0 * vol;
    state.water.magnesium_mg_total = 1.0 * vol;
    state.water.alkalinity_meq_total = 2.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;

    state.animal.adult.count = 5;
    state.animal.berried_females_count = 3;
    state.animal.egg_progress_days = 20.0;
    state.animal.egg_cohorts = vec![EggCohort {
        count: 3,
        progress_days: 20.0,
    }];
    state.animal.adult.condition_index = 0.3;
    state.hardware.aeration.enabled = false;

    let mut engine = Engine::from_parts(state, vec![]);
    engine.step_hours(24)?;

    let egg_failures: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::EggFailure)
        .collect();

    assert!(
        !egg_failures.is_empty(),
        "Should emit EggFailure under stressful conditions"
    );
    for e in &egg_failures {
        assert!(
            !e.cause_codes.is_empty(),
            "EggFailure events must have non-empty cause_codes"
        );
    }

    Ok(())
}

#[test]
fn molt_stress_warning_has_cause_codes() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5500));
    state.water.temperature_c = 30.0;
    state.environment.ambient_temp_c = 30.0;
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 2.0 * vol;
    state.water.magnesium_mg_total = 0.5 * vol;
    state.water.alkalinity_meq_total = 2.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 6.0 * vol;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.3;
    state.animal.molt_stress_index = 0.5;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.2;
    state.algae.set_periphyton_total(0.1);

    let mut engine = Engine::from_parts(state, vec![]);

    for _ in 0..14 {
        engine.step_hours(24)?;
    }

    let molt_warnings: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::MoltStressWarning)
        .collect();

    assert!(
        !molt_warnings.is_empty(),
        "Should emit MoltStressWarning under low-mineral, high-temp conditions"
    );
    for e in &molt_warnings {
        assert!(
            !e.cause_codes.is_empty(),
            "MoltStressWarning events must have non-empty cause_codes"
        );
    }

    Ok(())
}

#[test]
fn molt_stress_warning_reports_low_temperature() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5501));
    state.water.temperature_c = 12.0;
    state.environment.ambient_temp_c = 12.0;
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.95;
    state.animal.adult.reserve_g = 0.05;
    state.animal.molt_stress_index = 0.65;
    state.animal.inter_molt_timer_days = 0.0;
    state.animal.adult.molt_timer_days =
        state.shrimp_params.base_molt_interval_days / state.shrimp_params.temp_condition_min_factor;
    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.2;
    state.algae.set_periphyton_total(0.1);
    state.reseed_stability_tracker();

    let mut engine = Engine::from_parts(state, vec![]);

    engine.step_hours(24)?;

    let cold_warnings: Vec<_> = engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::MoltStressWarning)
        .collect();

    assert!(
        !cold_warnings.is_empty(),
        "Should emit MoltStressWarning under sustained cold-water stress"
    );
    assert!(cold_warnings
        .iter()
        .any(|event| event.cause_codes.contains(&EventCause::LowTemperature)));
    assert!(cold_warnings
        .iter()
        .any(|event| event.summary.contains("below 22.0-26.0 C optimal")));

    Ok(())
}

#[test]
fn molt_stress_warning_threshold_is_named_parameter() -> Result<(), tank_core::SimError> {
    let mut state = TankState::new(SimSeed(5502));
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;
    let vol = state.water_volume_l();
    state.water.calcium_mg_total = 40.0 * vol;
    state.water.magnesium_mg_total = 10.0 * vol;
    state.water.alkalinity_meq_total = 8.0 * vol;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * vol;
    state.water.dissolved_oxygen_mg_total = 8.0 * vol;

    state.animal.adult.count = 10;
    state.animal.adult.condition_index = 0.3;
    state.animal.molt_stress_index = 0.55;
    state.reseed_stability_tracker();

    let mut default_engine = Engine::from_parts(state.clone(), vec![]);
    default_engine.step_hours(24)?;
    let default_warnings = default_engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::MoltStressWarning)
        .count();

    state.shrimp_params.molt_stress_warning_threshold = 0.5;
    let mut permissive_engine = Engine::from_parts(state, vec![]);
    permissive_engine.step_hours(24)?;
    let permissive_warnings: Vec<_> = permissive_engine
        .full_state()
        .event_log
        .iter()
        .filter(|e| e.kind == EventKind::MoltStressWarning)
        .collect();

    assert_eq!(
        default_warnings, 0,
        "default threshold should not emit when stress decays just below 0.6"
    );
    assert!(
        !permissive_warnings.is_empty(),
        "lowering the named warning threshold should emit a warning for the same stress state"
    );
    assert!(permissive_warnings
        .iter()
        .any(|event| event.cause_codes.contains(&EventCause::PoorCondition)));

    Ok(())
}
