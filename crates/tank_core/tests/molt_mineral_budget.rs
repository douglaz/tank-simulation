use tank_core::systems::shrimp::{molt_mineral_modifier, step_daily_shrimp};
use tank_core::{
    Engine, EventCause, EventKind, PlayerAction, ShrimpRuntimeParams, SimError, SimSeed,
    SimulationEngine, SourceWaterProfile, TankGeometry, TankState, WaterState,
    ADULT_SHRIMP_BIOMASS_G, JUVENILE_SHRIMP_BIOMASS_G, LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
    SUB_ADULT_SHRIMP_BIOMASS_G,
};

fn molt_test_state(seed: SimSeed) -> TankState {
    let geometry = TankGeometry {
        length_cm: 40.0,
        width_cm: 30.0,
        height_cm: 35.0,
        fill_height_cm: 30.0,
        glass_thickness_mm: 5.0,
        open_top: true,
        lid_exchange_factor: 0.25,
        hardscape_area_cm2: 0.0,
    };
    let mut state = TankState::new(seed);
    state.geometry = geometry;
    state.water = WaterState::default_for_volume_l(state.water_volume_l());
    state.water.temperature_c = 24.0;
    state.environment.ambient_temp_c = 24.0;

    let volume_l = state.water_volume_l();
    state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
    state.water.alkalinity_meq_total = 8.0 * volume_l;
    state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * volume_l;
    state.water.bicarbonate_mg_total = 300.0 * volume_l;
    state.water.calcium_mg_total = 40.0 * volume_l;
    state.water.magnesium_mg_total = 10.0 * volume_l;
    state.algae.set_periphyton_total(5.0);

    state.microbe.set_decomposer_total(0.1);
    state.microbe.ammonia_oxidizer_biomass_g = 0.8;
    state.microbe.nitrite_oxidizer_biomass_g = 1.2;
    state.microbe.comammox_biomass_g = 0.4;
    state.filter_state.biofilter_maturity_index = 1.0;

    state.hardware.aeration.enabled = true;
    state.hardware.aeration.intensity = 0.3;
    state.hardware.light.enabled = true;
    state.hardware.light.intensity_index = 0.6;
    state.hardware.light.photoperiod_hours = 8.0;

    state.reseed_stability_tracker();
    state
}

fn configure_stage_locked_population(
    state: &mut TankState,
    adults: u32,
    sub_adults: u32,
    juveniles: u32,
) {
    state.animal.adult.count = adults;
    state.animal.sub_adult.count = sub_adults;
    state.animal.juvenile.count = juveniles;
    state.animal.inter_molt_timer_days = 0.0;
    state.animal.adult.molt_timer_days = 0.0;
    state.animal.sub_adult.molt_timer_days = 0.0;
    state.animal.juvenile.molt_timer_days = 0.0;
    state.shrimp_params.base_spawn_rate = 0.0;
    state.shrimp_params.juvenile_to_subadult_days = 10_000.0;
    state.shrimp_params.subadult_to_adult_days = 10_000.0;
    state.process_params.shrimp_base_mortality_per_day = 0.0;
    state.process_params.shrimp_stress_mortality_scale = 0.0;
}

fn set_minerals(state: &mut TankState, ca_mg_per_l: f64, mg_mg_per_l: f64) {
    let volume_l = state.water_volume_l();
    state.water.calcium_mg_total = ca_mg_per_l * volume_l;
    state.water.magnesium_mg_total = mg_mg_per_l * volume_l;
    state.reseed_stability_tracker();
}

fn seed_molt_reserves(state: &mut TankState) {
    state.animal.adult.reserve_g = f64::from(state.animal.adult.count)
        * ADULT_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
        * state.shrimp_params.molt_reserve_fraction;
    state.animal.sub_adult.reserve_g = f64::from(state.animal.sub_adult.count)
        * SUB_ADULT_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
        * state.shrimp_params.molt_reserve_fraction;
    state.animal.juvenile.reserve_g = f64::from(state.animal.juvenile.count)
        * JUVENILE_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
        * state.shrimp_params.molt_reserve_fraction;
}

fn load_source_profile(id: &str) -> SourceWaterProfile {
    let preset = tank_data::load_source_water(id)
        .unwrap_or_else(|err| panic!("failed to load source water preset '{id}': {err}"));
    SourceWaterProfile {
        temperature_c: preset.temperature_c,
        ammonia_mg_n_per_l: preset.ammonia_mg_n_per_l,
        nitrite_mg_n_per_l: preset.nitrite_mg_n_per_l,
        nitrate_mg_n_per_l: preset.nitrate_mg_n_per_l,
        phosphate_mg_p_per_l: preset.phosphate_mg_p_per_l,
        dic_mg_c_per_l: preset.dic_mg_c_per_l,
        doc_mg_c_per_l: preset.doc_mg_c_per_l,
        don_mg_n_per_l: preset.don_mg_n_per_l,
        alkalinity_meq_per_l: preset.alkalinity_meq_per_l,
        calcium_mg_per_l: preset.calcium_mg_per_l,
        magnesium_mg_per_l: preset.magnesium_mg_per_l,
        sodium_mg_per_l: preset.sodium_mg_per_l,
        potassium_mg_per_l: preset.potassium_mg_per_l,
        bicarbonate_mg_per_l: preset.bicarbonate_mg_per_l,
        chloride_mg_per_l: preset.chloride_mg_per_l,
        sulfate_mg_per_l: preset.sulfate_mg_per_l,
    }
}

fn apply_source_profile(state: &mut TankState, profile: &SourceWaterProfile) {
    let volume_l = state.water_volume_l();
    state.water = WaterState::from_source_profile_for_volume_l(profile, volume_l);
    state.environment.ambient_temp_c = profile.temperature_c;
    state.reseed_stability_tracker();
}

fn run_hours_with_daily_feed(
    mut state: TankState,
    hours: u32,
    feed_grams: f64,
) -> Result<TankState, SimError> {
    let mut engine = Engine::from_parts(state, vec![]);
    for hour in 0..hours {
        if hour % 24 == 0 {
            engine.apply_action(PlayerAction::Feed { grams: feed_grams })?;
        }
        engine.step_hours(1)?;
    }
    state = engine.full_state().clone();
    Ok(state)
}

#[test]
fn test_molt_success_increases_with_gh() {
    let params = ShrimpRuntimeParams::default();

    let low = molt_mineral_modifier(2.0, params.ca_min_mg_per_l, params.mg_min_mg_per_l, &params);
    let normal =
        molt_mineral_modifier(6.0, params.ca_min_mg_per_l, params.mg_min_mg_per_l, &params);
    let high = molt_mineral_modifier(
        10.0,
        params.ca_min_mg_per_l,
        params.mg_min_mg_per_l,
        &params,
    );

    assert!(
        low < normal,
        "GH 2 should trail GH 6 ({low:.3} vs {normal:.3})"
    );
    assert!(
        normal <= high,
        "GH 6 should not exceed GH 10 ({normal:.3} vs {high:.3})"
    );
    assert!(
        low < 0.5,
        "GH 2 should carry a strong penalty, got {low:.3}"
    );
    assert!(
        normal >= 0.95,
        "GH 6 should sit in the healthy band, got {normal:.3}"
    );
    assert!(
        (high - 1.0).abs() < 1e-9,
        "GH 10 should plateau at 1.0, got {high:.3}"
    );
}

#[test]
fn test_molt_success_formula_uses_ca_and_mg() {
    let params = ShrimpRuntimeParams::default();

    let full = molt_mineral_modifier(7.0, 25.0, 8.0, &params);
    let low_ca = molt_mineral_modifier(7.0, 10.0, 8.0, &params);
    let low_mg = molt_mineral_modifier(7.0, 25.0, 2.0, &params);
    let both_low = molt_mineral_modifier(7.0, 10.0, 2.0, &params);

    assert!(
        low_ca < full,
        "low Ca should reduce molt support ({low_ca:.3} < {full:.3})"
    );
    assert!(
        low_mg < full,
        "low Mg should reduce molt support ({low_mg:.3} < {full:.3})"
    );
    assert!(
        both_low < low_ca && both_low < low_mg,
        "combined Ca/Mg deficiency should be worst ({both_low:.3}, {low_ca:.3}, {low_mg:.3})"
    );
}

#[test]
fn test_molt_success_affected_by_condition() {
    let run_molt_with_condition = |condition: f64| {
        let mut state = molt_test_state(SimSeed(8_100));
        configure_stage_locked_population(&mut state, 10, 0, 0);
        set_minerals(&mut state, 40.0, 10.0);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state.animal.adult.condition_index = condition;
        state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

        step_daily_shrimp(&mut state);
        state.animal.last_molt_success
    };

    assert!(run_molt_with_condition(0.9));
    assert!(!run_molt_with_condition(0.05));
}

#[test]
fn test_failed_molt_increases_mortality() -> Result<(), SimError> {
    let run_case = |ca: f64, mg: f64, seed: u64| -> Result<(u32, f64), SimError> {
        let mut state = molt_test_state(SimSeed(seed));
        configure_stage_locked_population(&mut state, 12, 12, 12);
        set_minerals(&mut state, ca, mg);
        state.animal.set_population_condition_index(0.75);
        seed_molt_reserves(&mut state);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state
            .process_params
            .shrimp_periphyton_grazing_g_per_shrimp_per_day = 0.0;
        state.shrimp_params.base_molt_interval_days = 7.0;
        state.shrimp_params.sub_adult_molt_interval_days = 5.0;
        state.shrimp_params.juvenile_molt_interval_days = 3.0;

        let final_state = run_hours_with_daily_feed(state, 30 * 24, 0.0)?;
        Ok((
            final_state.animal.total_count(),
            final_state.animal.failed_molt_accum,
        ))
    };

    let (hard_population, hard_failed_molt_accum) = run_case(40.0, 10.0, 8_200)?;
    let (soft_population, soft_failed_molt_accum) = run_case(1.0, 0.1, 8_201)?;

    assert!(
        soft_population < hard_population,
        "repeated failed molts should shrink the soft-water population ({soft_population} < {hard_population})"
    );
    assert!(
        soft_failed_molt_accum > hard_failed_molt_accum,
        "failed_molt_accum should stay elevated in the bad-mineral case ({soft_failed_molt_accum:.3} > {hard_failed_molt_accum:.3})"
    );

    Ok(())
}

#[test]
fn test_very_low_gh_causes_molt_deaths() -> Result<(), SimError> {
    let mut state = molt_test_state(SimSeed(8_300));
    configure_stage_locked_population(&mut state, 10, 10, 10);
    set_minerals(&mut state, 1.5, 0.3);
    state.animal.set_population_condition_index(0.7);
    seed_molt_reserves(&mut state);
    state.shrimp_params.base_molt_interval_days = 8.0;
    state.shrimp_params.sub_adult_molt_interval_days = 6.0;
    state.shrimp_params.juvenile_molt_interval_days = 4.0;

    let final_state = run_hours_with_daily_feed(state, 500, 0.05)?;
    let molt_failures: Vec<_> = final_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::MoltFailure)
        .collect();

    assert!(
        !molt_failures.is_empty(),
        "very soft water should emit MoltFailure events"
    );
    assert!(molt_failures
        .iter()
        .any(|event| { event.cause_codes.contains(&EventCause::LowMinerals) }));
    assert!(molt_failures.iter().any(|event| {
        event.summary.contains("GH") || event.summary.contains("Ca") || event.summary.contains("Mg")
    }));
    assert!(
        final_state.animal.total_count() < 30,
        "very low GH should drive detectable deaths, got {} survivors",
        final_state.animal.total_count()
    );

    Ok(())
}

#[test]
fn test_mineral_modifier_named_parameters() {
    let mut params = ShrimpRuntimeParams::default();
    let baseline = molt_mineral_modifier(4.0, 15.0, 3.0, &params);

    params.gh_min_d = 4.0;
    let permissive_gh = molt_mineral_modifier(4.0, 15.0, 3.0, &params);
    assert!(permissive_gh > baseline);

    params = ShrimpRuntimeParams::default();
    params.ca_min_mg_per_l = 10.0;
    let permissive_ca = molt_mineral_modifier(4.0, 15.0, 3.0, &params);
    assert!(permissive_ca > baseline);

    params = ShrimpRuntimeParams::default();
    params.mg_min_mg_per_l = 2.0;
    let permissive_mg = molt_mineral_modifier(4.0, 15.0, 3.0, &params);
    assert!(permissive_mg > baseline);

    params = ShrimpRuntimeParams::default();
    let strict_high_gh = molt_mineral_modifier(14.0, 20.0, 5.0, &params);
    params.molt_gh_excess_penalty_divisor = 20.0;
    let permissive_high_gh = molt_mineral_modifier(14.0, 20.0, 5.0, &params);
    assert!(permissive_high_gh > strict_high_gh);

    params = ShrimpRuntimeParams::default();
    let default_floor = molt_mineral_modifier(8.0, 2.0, 0.5, &params);
    params.molt_mineral_factor_floor = 0.1;
    let lower_floor = molt_mineral_modifier(8.0, 2.0, 0.5, &params);
    assert!(lower_floor < default_floor);

    params = ShrimpRuntimeParams::default();
    assert_eq!(params.molt_success_threshold, 0.55);
    assert_eq!(params.critical_molt_gh_ratio, 0.3);
    assert_eq!(params.failed_molt_accum_increase_per_failed_stage, 0.3);
    assert_eq!(params.failed_molt_accum_recovery_per_successful_stage, 0.35);
    assert_eq!(params.failed_molt_stress_blend, 0.5);
    assert_eq!(params.molt_reserve_fraction, 0.1);
    assert_eq!(params.molt_reserve_factor_floor, 0.4);
    assert_eq!(params.molt_condition_weight, 0.75);
    assert_eq!(params.molt_reserve_weight, 0.25);
    assert_eq!(params.molt_failure_poor_condition_threshold, 0.65);
    assert_eq!(params.molt_failure_instability_threshold, 0.3);
    assert_eq!(params.molt_stress_warning_threshold, 0.6);
    assert_eq!(params.molt_stress_mortality_threshold, 0.5);
    assert_eq!(params.molt_stress_mineral_gh_weight, 0.5);
    assert_eq!(params.molt_stress_mineral_ca_weight, 0.3);
    assert_eq!(params.molt_stress_mineral_mg_weight, 0.2);
    assert_eq!(params.molt_stress_pressure_mineral_weight, 0.3);
    assert_eq!(params.molt_stress_pressure_instability_weight, 0.3);
    assert_eq!(params.molt_stress_pressure_condition_weight, 0.2);
    assert_eq!(params.molt_stress_pressure_thermal_weight, 0.2);
    assert_eq!(params.molt_stress_pressure_hourly_weight, 0.3);
    assert_eq!(params.molt_stress_rise_smoothing, 0.2);
    assert_eq!(params.molt_stress_decay_smoothing, 0.05);
    assert_eq!(params.molt_gh_excess_penalty_divisor, 10.0);
    assert_eq!(params.molt_mineral_factor_floor, 0.3);
    assert!(params.juvenile_molt_interval_days < params.sub_adult_molt_interval_days);
    assert!(params.sub_adult_molt_interval_days < params.base_molt_interval_days);
}

#[test]
fn test_zero_gh_min_parameter_uses_defensive_guard() {
    let mut params = ShrimpRuntimeParams::default();
    params.gh_min_d = 0.0;

    let modifier =
        molt_mineral_modifier(0.0, params.ca_min_mg_per_l, params.mg_min_mg_per_l, &params);
    assert!(modifier.is_finite(), "zero gh_min_d should not produce NaN");
    assert!(
        (modifier - 1.0).abs() < 1e-9,
        "zero gh_min_d should behave like an unbounded healthy lower GH floor, got {modifier}"
    );

    let mut state = molt_test_state(SimSeed(8_346));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 40.0, 10.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.shrimp_params.gh_min_d = 0.0;

    step_daily_shrimp(&mut state);

    assert!(
        state.animal.molt_stress_index.is_finite(),
        "zero gh_min_d should not destabilize molt stress calculations"
    );
}

#[test]
fn test_zero_ca_and_mg_min_parameters_use_defensive_guards() {
    let mut params = ShrimpRuntimeParams::default();
    params.gh_min_d = 0.0;
    params.ca_min_mg_per_l = 0.0;
    params.mg_min_mg_per_l = 0.0;

    let modifier = molt_mineral_modifier(0.0, 0.0, 0.0, &params);
    assert!(
        modifier.is_finite(),
        "zero Ca/Mg minimums should not produce NaN"
    );
    assert!(
        (modifier - 1.0).abs() < 1e-9,
        "zero Ca/Mg minimums should behave like unbounded healthy lower floors, got {modifier}"
    );

    let mut state = molt_test_state(SimSeed(8_347));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 0.0, 0.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.shrimp_params.gh_min_d = 0.0;
    state.shrimp_params.ca_min_mg_per_l = 0.0;
    state.shrimp_params.mg_min_mg_per_l = 0.0;

    step_daily_shrimp(&mut state);

    assert!(
        state.animal.molt_stress_index.is_finite(),
        "zero Ca/Mg minimums should not destabilize molt stress calculations"
    );
}

#[test]
fn test_molt_condition_modifier_named_parameters() {
    let run_case = |configure: fn(&mut TankState)| {
        let mut state = molt_test_state(SimSeed(8_345));
        configure_stage_locked_population(&mut state, 10, 0, 0);
        set_minerals(&mut state, 40.0, 10.0);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state.process_params.shrimp_base_mortality_per_day = 0.0;
        state.process_params.shrimp_stress_mortality_scale = 0.0;
        state.shrimp_params.base_spawn_rate = 0.0;
        state.shrimp_params.molt_success_threshold = 0.6;
        state.animal.adult.condition_index = 0.6;
        state.animal.adult.reserve_g = 0.0;
        state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;
        configure(&mut state);
        step_daily_shrimp(&mut state);
        state.animal.last_molt_success
    };

    assert!(
        !run_case(|_| {}),
        "default reserve/condition tuning should fail a reserve-empty shrimp at a 0.6 threshold"
    );
    assert!(
        run_case(|state| {
            state.shrimp_params.molt_reserve_factor_floor = 0.9;
        }),
        "raising the named reserve floor should soften reserve shortfall"
    );
    assert!(
        run_case(|state| {
            state.shrimp_params.molt_condition_weight = 1.0;
            state.shrimp_params.molt_reserve_weight = 0.0;
        }),
        "shifting the named blend fully onto condition should change the molt outcome"
    );
    assert!(
        run_case(|state| {
            state.shrimp_params.molt_reserve_fraction = 0.05;
            state.animal.adult.reserve_g = f64::from(state.animal.adult.count)
                * ADULT_SHRIMP_BIOMASS_G
                * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
                * 0.05;
        }),
        "lowering the named reserve target should let the same reserve pool clear the molt gate"
    );
}

#[test]
fn test_molt_stress_parameters_are_named() {
    let build_state = |seed: SimSeed| {
        let mut state = molt_test_state(seed);
        configure_stage_locked_population(&mut state, 10, 0, 0);
        set_minerals(&mut state, 10.0, 20.0);
        state.animal.set_population_condition_index(1.0);
        state.animal.molt_stress_index = 0.0;
        state.process_params.shrimp_condition_smoothing = 0.0;
        state
    };

    let mut baseline = build_state(SimSeed(8_348));
    step_daily_shrimp(&mut baseline);

    let mut ca_weighted = build_state(SimSeed(8_349));
    ca_weighted.shrimp_params.molt_stress_mineral_gh_weight = 0.0;
    ca_weighted.shrimp_params.molt_stress_mineral_ca_weight = 1.0;
    ca_weighted.shrimp_params.molt_stress_mineral_mg_weight = 0.0;
    step_daily_shrimp(&mut ca_weighted);

    let mut no_mineral_pressure = build_state(SimSeed(8_350));
    no_mineral_pressure
        .shrimp_params
        .molt_stress_pressure_mineral_weight = 0.0;
    step_daily_shrimp(&mut no_mineral_pressure);

    let mut fast_rise = build_state(SimSeed(8_351));
    fast_rise.shrimp_params.molt_stress_rise_smoothing = 1.0;
    step_daily_shrimp(&mut fast_rise);

    assert!(
        ca_weighted.animal.molt_stress_index > baseline.animal.molt_stress_index,
        "raising the named calcium weight should increase stress under isolated Ca deficiency ({:.4} > {:.4})",
        ca_weighted.animal.molt_stress_index,
        baseline.animal.molt_stress_index
    );
    assert!(
        no_mineral_pressure.animal.molt_stress_index < baseline.animal.molt_stress_index,
        "zeroing the named mineral-pressure weight should soften molt stress ({:.4} < {:.4})",
        no_mineral_pressure.animal.molt_stress_index,
        baseline.animal.molt_stress_index
    );
    assert!(
        fast_rise.animal.molt_stress_index > baseline.animal.molt_stress_index,
        "raising the named rise smoothing should make stress respond faster ({:.4} > {:.4})",
        fast_rise.animal.molt_stress_index,
        baseline.animal.molt_stress_index
    );
}

#[test]
fn test_critical_molt_gh_ratio_is_named_parameter() {
    let run_case = |critical_ratio: f64| {
        let mut state = molt_test_state(SimSeed(8_350));
        configure_stage_locked_population(&mut state, 10, 0, 0);
        set_minerals(&mut state, 20.0, 5.0);
        state.animal.set_population_condition_index(0.95);
        seed_molt_reserves(&mut state);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state.shrimp_params.gh_min_d = 10.0;
        state.shrimp_params.molt_success_threshold = 0.1;
        state.shrimp_params.critical_molt_gh_ratio = critical_ratio;
        state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

        step_daily_shrimp(&mut state);
        state.animal.last_molt_success
    };

    assert!(
        run_case(0.3),
        "the named cutoff should allow a low-but-not-critical GH molt to succeed when the score clears the threshold"
    );
    assert!(
        !run_case(0.4),
        "raising the named cutoff should force the same GH case to fail"
    );
}

#[test]
fn test_molt_frequency_varies_by_stage() {
    let mut state = molt_test_state(SimSeed(8_400));
    configure_stage_locked_population(&mut state, 5, 5, 5);
    set_minerals(&mut state, 40.0, 10.0);
    state.animal.set_population_condition_index(0.9);
    state.process_params.shrimp_condition_smoothing = 0.0;

    let juvenile_interval = state.shrimp_params.juvenile_molt_interval_days as u32;
    for _ in 0..=juvenile_interval {
        step_daily_shrimp(&mut state);
    }

    assert!(
        state.animal.juvenile.molt_timer_days < state.animal.adult.molt_timer_days,
        "juveniles should reset their timer before adults ({:.1} < {:.1})",
        state.animal.juvenile.molt_timer_days,
        state.animal.adult.molt_timer_days
    );
    assert!(
        state.animal.sub_adult.molt_timer_days <= state.animal.adult.molt_timer_days,
        "sub-adults should not molt less frequently than adults ({:.1} <= {:.1})",
        state.animal.sub_adult.molt_timer_days,
        state.animal.adult.molt_timer_days
    );
}

#[test]
fn test_mixed_stage_molt_days_do_not_reduce_failed_molt_accum() {
    let mut state = molt_test_state(SimSeed(8_450));
    configure_stage_locked_population(&mut state, 6, 0, 6);
    set_minerals(&mut state, 40.0, 10.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.animal.adult.condition_index = 0.05;
    state.animal.juvenile.condition_index = 0.9;
    state.animal.failed_molt_accum = 0.2;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;
    state.animal.juvenile.molt_timer_days = state.shrimp_params.juvenile_molt_interval_days;

    step_daily_shrimp(&mut state);

    assert!(!state.animal.last_molt_success);
    assert!(
        state.animal.failed_molt_accum > 0.2,
        "a mixed success/failure day must not lower failed_molt_accum, got {:.3}",
        state.animal.failed_molt_accum
    );
}

#[test]
fn test_failed_molt_accumulation_parameters_are_named() {
    let mut default_failure = molt_test_state(SimSeed(8_451));
    configure_stage_locked_population(&mut default_failure, 10, 0, 0);
    set_minerals(&mut default_failure, 40.0, 10.0);
    default_failure.process_params.shrimp_condition_smoothing = 0.0;
    default_failure.animal.adult.condition_index = 0.05;
    default_failure.animal.adult.reserve_g = 0.0;
    default_failure.animal.adult.molt_timer_days =
        default_failure.shrimp_params.base_molt_interval_days;

    let mut gentle_failure = default_failure.clone();
    gentle_failure
        .shrimp_params
        .failed_molt_accum_increase_per_failed_stage = 0.1;

    step_daily_shrimp(&mut default_failure);
    step_daily_shrimp(&mut gentle_failure);

    assert!(
        default_failure.animal.failed_molt_accum > gentle_failure.animal.failed_molt_accum,
        "lowering the named failed-molt increment should soften accumulation ({:.3} > {:.3})",
        default_failure.animal.failed_molt_accum,
        gentle_failure.animal.failed_molt_accum
    );

    let mut default_recovery = molt_test_state(SimSeed(8_452));
    configure_stage_locked_population(&mut default_recovery, 10, 0, 0);
    set_minerals(&mut default_recovery, 40.0, 10.0);
    default_recovery.process_params.shrimp_condition_smoothing = 0.0;
    default_recovery.animal.set_population_condition_index(0.95);
    seed_molt_reserves(&mut default_recovery);
    default_recovery.animal.failed_molt_accum = 0.6;
    default_recovery.animal.adult.molt_timer_days =
        default_recovery.shrimp_params.base_molt_interval_days;

    let mut slow_recovery = default_recovery.clone();
    slow_recovery
        .shrimp_params
        .failed_molt_accum_recovery_per_successful_stage = 0.1;

    step_daily_shrimp(&mut default_recovery);
    step_daily_shrimp(&mut slow_recovery);

    assert!(
        default_recovery.animal.failed_molt_accum < slow_recovery.animal.failed_molt_accum,
        "lowering the named recovery rate should keep failed-molt accumulation elevated ({:.3} < {:.3})",
        default_recovery.animal.failed_molt_accum,
        slow_recovery.animal.failed_molt_accum
    );
}

#[test]
fn test_molt_failure_reports_marginal_gap_when_no_specific_cause() {
    let mut state = molt_test_state(SimSeed(9_103));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 40.0, 10.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.animal.adult.condition_index = 0.6;
    seed_molt_reserves(&mut state);
    state.stability_tracker.instability_index = 0.24;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

    step_daily_shrimp(&mut state);

    let failure = state
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::MoltFailure)
        .expect("marginal molt failure should emit an event");
    assert!(failure.cause_codes.contains(&EventCause::MarginalFailure));
    assert!(!failure.cause_codes.contains(&EventCause::PoorCondition));
    assert!(!failure
        .cause_codes
        .contains(&EventCause::ChemistryInstability));
    assert!(failure.summary.contains("score"));
}

#[test]
fn test_molt_failure_diagnostic_thresholds_are_named() {
    let mut state = molt_test_state(SimSeed(9_104));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 40.0, 10.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.animal.adult.condition_index = 0.6;
    seed_molt_reserves(&mut state);
    state.stability_tracker.instability_index = 0.24;
    state.shrimp_params.molt_failure_poor_condition_threshold = 0.75;
    state.shrimp_params.molt_failure_instability_threshold = 0.2;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

    step_daily_shrimp(&mut state);

    let failure = state
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::MoltFailure)
        .expect("threshold-tuned molt failure should emit an event");
    assert!(failure.cause_codes.contains(&EventCause::PoorCondition));
    assert!(failure
        .cause_codes
        .contains(&EventCause::ChemistryInstability));
    assert!(failure.summary.contains("condition 0.60<0.75"));
    assert!(failure.summary.contains("instability 0.24>0.20"));
}

#[test]
fn test_soft_vs_hard_water_shrimp_survival() -> Result<(), SimError> {
    let run_profile = |profile: &SourceWaterProfile, seed: u64| -> Result<TankState, SimError> {
        let mut state = molt_test_state(SimSeed(seed));
        apply_source_profile(&mut state, profile);
        configure_stage_locked_population(&mut state, 12, 12, 12);
        state.animal.set_population_condition_index(0.8);
        seed_molt_reserves(&mut state);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state
            .process_params
            .shrimp_periphyton_grazing_g_per_shrimp_per_day = 0.0;
        run_hours_with_daily_feed(state, 1000, 0.0)
    };

    let hard_profile = load_source_profile("hard_shrimp");
    let mut soft_profile = load_source_profile("ro_like");
    soft_profile.calcium_mg_per_l = 10.0;
    soft_profile.magnesium_mg_per_l = 2.0;

    let hard_state = run_profile(&hard_profile, 9_000)?;
    let soft_state = run_profile(&soft_profile, 9_001)?;

    let hard_population = hard_state.animal.total_count();
    let soft_population = soft_state.animal.total_count();
    let soft_molt_failures: Vec<_> = soft_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::MoltFailure)
        .collect();

    assert!(
        hard_population > soft_population,
        "hard_shrimp should outperform the mineral-poor soft-water profile over 1000 hours ({hard_population} > {soft_population})"
    );
    assert!(
        hard_population.saturating_sub(soft_population) >= 5,
        "population gap should be noticeable after 1000 hours ({hard_population} vs {soft_population})"
    );
    assert!(
        !soft_molt_failures.is_empty(),
        "the mineral-poor soft-water case should log molt failures tied to the source-water minerals"
    );
    assert!(soft_molt_failures
        .iter()
        .any(|event| { event.cause_codes.contains(&EventCause::LowMinerals) }));
    assert!(soft_molt_failures
        .iter()
        .any(|event| event.summary.contains("GH")
            && event.summary.contains("Ca")
            && event.summary.contains("Mg")));

    Ok(())
}

#[test]
fn test_cold_molt_failure_reports_low_temperature() {
    let mut state = molt_test_state(SimSeed(9_100));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 40.0, 10.0);
    state.animal.set_population_condition_index(0.95);
    seed_molt_reserves(&mut state);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.water.temperature_c = 10.0;
    state.environment.ambient_temp_c = 10.0;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days / 0.25;

    step_daily_shrimp(&mut state);

    let failure = state
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::MoltFailure)
        .expect("cold molt failure should emit an event");
    assert!(failure.cause_codes.contains(&EventCause::LowTemperature));
    assert!(failure.summary.contains("temp 10.0<22.0 C"));
}

#[test]
fn test_molt_failure_reports_reserve_shortfall() {
    let mut state = molt_test_state(SimSeed(9_101));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 40.0, 10.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.animal.adult.condition_index = 0.58;
    state.animal.adult.reserve_g = 0.0;
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

    step_daily_shrimp(&mut state);

    let failure = state
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::MoltFailure)
        .expect("reserve-limited molt failure should emit an event");
    assert!(failure.cause_codes.contains(&EventCause::Starvation));
    assert!(failure.summary.contains("reserve"));
}

#[test]
fn test_high_gh_molt_failure_reports_high_minerals() {
    let mut state = molt_test_state(SimSeed(9_102));
    configure_stage_locked_population(&mut state, 10, 0, 0);
    set_minerals(&mut state, 120.0, 30.0);
    state.process_params.shrimp_condition_smoothing = 0.0;
    state.animal.adult.condition_index = 1.0;
    seed_molt_reserves(&mut state);
    state.animal.adult.molt_timer_days = state.shrimp_params.base_molt_interval_days;

    step_daily_shrimp(&mut state);

    let failure = state
        .event_log
        .iter()
        .find(|event| event.kind == EventKind::MoltFailure)
        .expect("high-GH molt failure should emit an event");
    assert!(failure.cause_codes.contains(&EventCause::HighMinerals));
    assert!(failure.summary.contains("GH"));
    assert!(failure.summary.contains('>'));
    assert!(!failure.cause_codes.contains(&EventCause::PoorCondition));
}
