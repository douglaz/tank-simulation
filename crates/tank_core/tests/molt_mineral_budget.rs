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
        * 0.1;
    state.animal.sub_adult.reserve_g = f64::from(state.animal.sub_adult.count)
        * SUB_ADULT_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
        * 0.1;
    state.animal.juvenile.reserve_g = f64::from(state.animal.juvenile.count)
        * JUVENILE_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G
        * 0.1;
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
        state.process_params.shrimp_periphyton_grazing_g_per_shrimp_per_day = 0.0;
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

    assert_eq!(params.molt_success_threshold, 0.55);
    assert!(params.juvenile_molt_interval_days < params.sub_adult_molt_interval_days);
    assert!(params.sub_adult_molt_interval_days < params.base_molt_interval_days);
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
fn test_soft_vs_hard_water_shrimp_survival() -> Result<(), SimError> {
    let run_profile = |profile_id: &str, seed: u64| -> Result<TankState, SimError> {
        let mut state = molt_test_state(SimSeed(seed));
        apply_source_profile(&mut state, &load_source_profile(profile_id));
        configure_stage_locked_population(&mut state, 12, 12, 12);
        state.animal.set_population_condition_index(0.8);
        seed_molt_reserves(&mut state);
        state.process_params.shrimp_condition_smoothing = 0.0;
        state.process_params.shrimp_periphyton_grazing_g_per_shrimp_per_day = 0.0;
        run_hours_with_daily_feed(state, 1000, 0.0)
    };

    let hard_state = run_profile("hard_shrimp", 9_000)?;
    let soft_state = run_profile("ro_like", 9_001)?;

    let hard_population = hard_state.animal.total_count();
    let soft_population = soft_state.animal.total_count();
    let soft_molt_failures: Vec<_> = soft_state
        .event_log
        .iter()
        .filter(|event| event.kind == EventKind::MoltFailure)
        .collect();

    assert!(
        hard_population > soft_population,
        "hard_shrimp should outperform ro_like over 1000 hours ({hard_population} > {soft_population})"
    );
    assert!(
        hard_population.saturating_sub(soft_population) >= 5,
        "population gap should be noticeable after 1000 hours ({hard_population} vs {soft_population})"
    );
    assert!(
        !soft_molt_failures.is_empty(),
        "the ro_like case should log molt failures tied to the source-water minerals"
    );
    assert!(soft_molt_failures
        .iter()
        .any(|event| { event.cause_codes.contains(&EventCause::LowMinerals) }));
    assert!(soft_molt_failures
        .iter()
        .any(|event| event.summary.contains("GH")));

    Ok(())
}
