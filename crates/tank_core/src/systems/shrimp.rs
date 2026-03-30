use crate::systems::chemistry::{compute_nh3_mg_n_per_l, resolve_carbonate_state};
use crate::types::{
    algae_carbon_mg, algae_detrital_mass_g, algae_nitrogen_mg, detritus_carbon_mg,
    detritus_nitrogen_mg, shrimp_body_detrital_mass_g, EggCohort, EventCause, EventKind,
    EventSeverity, HabitatKind, ShrimpRuntimeParams, TankState, ADULT_SHRIMP_BIOMASS_G,
    JUVENILE_SHRIMP_BIOMASS_G, LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G, MG_N_PER_MEQ_AMMONIA,
    SUB_ADULT_SHRIMP_BIOMASS_G,
};
const ROUTING_MASS_ASSERT_TOLERANCE_G: f64 = 1e-12;
const DEATH_DETRITUS_FRACTION_TOLERANCE: f64 = 1e-9;
const DETERMINISTIC_CARRY_LIMIT: f64 = 1.0;

// ── Hourly ──────────────────────────────────────────────────────────────────

/// Accumulates NH3, nitrite, low-DO, heat, and instability stress each hour.
/// Runs after chemistry/DO updates, before hourly event emission.
pub fn step_hourly_shrimp_stress(state: &mut TankState) {
    if state.animal.total_count() == 0 {
        return;
    }

    let chemistry = state.concentrations();
    let volume_l = chemistry.volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let tan_mg_l = chemistry.tan_mg_n_per_l();
    let nh3_mg_l = compute_nh3_mg_n_per_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = chemistry.nitrite_mg_n_per_l();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;

    // NH3 stress (threshold 0.02 mg/L, stronger than TAN alone)
    if nh3_mg_l > 0.02 {
        state.animal.hourly_nh3_stress_accum += (nh3_mg_l - 0.02) * 2.0 / 24.0;
    }

    // Nitrite stress (threshold 0.5 mg/L)
    if nitrite_mg_l > 0.5 {
        state.animal.hourly_nitrite_stress_accum += (nitrite_mg_l - 0.5) * 0.5 / 24.0;
    }

    // Low DO stress (threshold 5.0 mg/L)
    if do_mg_l < 5.0 {
        state.animal.hourly_low_do_stress_accum += (5.0 - do_mg_l) * 0.3 / 24.0;
    }

    // Heat stress (above species optimal max)
    let opt_max = state.shrimp_params.optimal_temp_max_c;
    if temp > opt_max {
        state.animal.hourly_heat_stress_accum += (temp - opt_max) * 0.1 / 24.0;
    }

    // Instability stress from stability tracker
    state.animal.hourly_instability_stress_accum +=
        state.stability_tracker.instability_index * 0.05 / 24.0;
}

// ── Daily ───────────────────────────────────────────────────────────────────

/// Full daily shrimp lifecycle: feeding -> condition -> molt stress ->
/// molt cycle -> reproductive readiness -> spawning -> egg development/hatching ->
/// juvenile recruitment (two-stage) -> mortality -> events.
pub fn step_daily_shrimp(state: &mut TankState) {
    if state.animal.total_count() == 0 && state.animal.berried_females_count == 0 {
        reset_hourly_accumulators(state);
        return;
    }

    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        reset_hourly_accumulators(state);
        return;
    }

    shrimp_feeding(state);
    refresh_carbonate_state(state);
    update_condition(state);
    update_molt_stress(state);
    molt_cycle(state);
    update_reproductive_readiness(state);
    spawning(state);
    egg_development(state);
    juvenile_to_subadult(state);
    subadult_to_adult(state);
    mortality(state);
    refresh_carbonate_state(state);
    emit_molt_stress_warning(state);

    reset_hourly_accumulators(state);
}

/// Updates the stability tracker at the end of each daily cycle.
pub fn update_stability_tracker(state: &mut TankState) {
    let chemistry = state.concentrations();
    let gh_d = chemistry.gh_d();
    let do_mg_l = chemistry.do_mg_per_l();

    let tracker = &mut state.stability_tracker;

    let temp_swing = (state.water.temperature_c - tracker.prev_temp_c).abs();
    let ph_swing = (state.water.ph - tracker.prev_ph).abs();
    let gh_swing = (gh_d - tracker.prev_gh_d).abs();
    let do_swing = (do_mg_l - tracker.prev_do_mg_l).abs();

    // Weighted instability normalised to 0..1
    let raw_instability =
        (temp_swing / 3.0 + ph_swing / 0.5 + gh_swing / 3.0 + do_swing / 3.0).clamp(0.0, 1.0);

    // Rises quickly, decays slowly
    if raw_instability > tracker.instability_index {
        tracker.instability_index += 0.3 * (raw_instability - tracker.instability_index);
    } else {
        tracker.instability_index += 0.1 * (raw_instability - tracker.instability_index);
    }
    tracker.instability_index = tracker.instability_index.clamp(0.0, 1.0);

    tracker.prev_temp_c = state.water.temperature_c;
    tracker.prev_ph = state.water.ph;
    tracker.prev_gh_d = gh_d;
    tracker.prev_do_mg_l = do_mg_l;
}

// ── Private helpers ─────────────────────────────────────────────────────────

fn shrimp_grazing_access_factor(state: &TankState) -> f64 {
    0.5 + (0.5 * state.avg_substrate_index(|layer| layer.grazing_surface_index))
}

fn shrimp_target_food_route_g(state: &TankState) -> f64 {
    let total_feeding_units = state.animal.feeding_units();
    if total_feeding_units <= f64::EPSILON {
        return 0.0;
    }

    let food_demand_biomass_g = total_feeding_units
        * state
            .process_params
            .shrimp_periphyton_grazing_g_per_shrimp_per_day
        * shrimp_grazing_access_factor(state);
    algae_detrital_mass_g(
        food_demand_biomass_g,
        state.process_params.feed_n_to_c_ratio,
    )
}

fn shrimp_periphyton_accessibility(substrate_surface_access: f64, kind: HabitatKind) -> f64 {
    match kind {
        HabitatKind::GlassHardscape => 1.0,
        HabitatKind::SubstrateSurface => substrate_surface_access.clamp(0.0, 1.0),
        HabitatKind::PlantSurfaces => 0.45,
        HabitatKind::FilterMedia => 0.05,
        HabitatKind::SubstrateDeep => 0.0,
    }
}

fn shrimp_feeding(state: &mut TankState) {
    let total_feeding_units = state.animal.feeding_units();

    if total_feeding_units <= f64::EPSILON {
        state.animal.daily_food_consumed_g = 0.0;
        return;
    }

    let rate = state
        .process_params
        .shrimp_periphyton_grazing_g_per_shrimp_per_day;
    let grazing_access_factor = shrimp_grazing_access_factor(state);
    let food_demand_biomass_g = total_feeding_units * rate * grazing_access_factor;
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let food_demand_route_g = algae_detrital_mass_g(food_demand_biomass_g, n_to_c_ratio);

    // Shrimp graze periphyton (at most 50% of available per day), preferring
    // exposed surfaces over filter-internal or buried biofilm.
    let max_periph = state.algae.periphyton_biomass_g * 0.5;
    let requested_periph_biomass_g = food_demand_biomass_g.min(max_periph).max(0.0);
    let substrate_surface_access =
        0.4 + 0.6 * state.avg_substrate_index(|layer| layer.grazing_surface_index);
    let periph_consumed_biomass_g = super::microfauna::remove_periphyton_by_accessibility(
        &mut state.algae,
        requested_periph_biomass_g,
        |kind| shrimp_periphyton_accessibility(substrate_surface_access, kind),
    );
    let periph_consumed_route_g = algae_detrital_mass_g(periph_consumed_biomass_g, n_to_c_ratio);

    // Fine detritus only fills the remaining appetite on the same routing
    // organic-matter basis used by daily_food_consumed_g and reserve routing.
    let detritus_demand_route_g = (food_demand_route_g - periph_consumed_route_g).max(0.0);
    let max_detritus_route_g = state.detritus.fine_detritus_g_total * 0.2;
    let detritus_consumed_route_g = detritus_demand_route_g.min(max_detritus_route_g).max(0.0);
    state.detritus.fine_detritus_g_total =
        (state.detritus.fine_detritus_g_total - detritus_consumed_route_g).max(0.0);

    // Total consumed food (population-wide)
    state.animal.daily_food_consumed_g = periph_consumed_route_g + detritus_consumed_route_g;
    let consumed_n_mg = algae_nitrogen_mg(periph_consumed_biomass_g)
        + detritus_nitrogen_mg(detritus_consumed_route_g, n_to_c_ratio);
    let consumed_c_mg = algae_carbon_mg(periph_consumed_biomass_g, n_to_c_ratio)
        + detritus_carbon_mg(detritus_consumed_route_g, n_to_c_ratio);
    let elemental_food_mass_g = (consumed_n_mg + consumed_c_mg) / 1000.0;
    debug_assert!(
        (state.animal.daily_food_consumed_g - elemental_food_mass_g).abs()
            <= ROUTING_MASS_ASSERT_TOLERANCE_G,
        "shrimp feeding routed {} g but elemental bookkeeping implies {} g",
        state.animal.daily_food_consumed_g,
        elemental_food_mass_g,
    );

    route_consumed_food(state, consumed_n_mg, consumed_c_mg);
}

/// Routes consumed food through the consumer routing contract:
///
/// ```text
/// consumed = feces + assimilated
/// assimilated = respired + excreted + retained
/// ```
///
/// Destinations:
/// - feces -> fine_detritus_g_total (as organic matter grams)
/// - excreted N -> ammonia_total_mg_n_total (TAN)
/// - excreted C -> dissolved_organic_carbon_mg_c_total (DOC)
/// - respired C -> dissolved_inorganic_carbon_mg_c_total (DIC)
/// - respired -> O2 demand (dissolved_oxygen_mg_total)
/// - retained -> per-stage reserve_g (organic matter grams, proportional to feeding weight)
fn route_consumed_food(state: &mut TankState, consumed_n_mg: f64, consumed_c_mg: f64) {
    if consumed_n_mg <= f64::EPSILON && consumed_c_mg <= f64::EPSILON {
        return;
    }

    let params = &state.process_params;
    let ae = params.shrimp_assimilation_efficiency;
    let fecal_fraction = 1.0 - ae;
    let resp_frac = params.shrimp_respiration_fraction_of_assimilated;
    let excr_frac = params.shrimp_excretion_fraction_of_assimilated;
    let growth_frac = params.shrimp_growth_fraction_of_assimilated;
    let o2_per_c = params.shrimp_o2_per_mg_c_respired;

    // ── Feces: unassimilated share -> fine detritus ──
    let fecal_n_mg = consumed_n_mg * fecal_fraction;
    let fecal_c_mg = consumed_c_mg * fecal_fraction;
    let fecal_mass_g = (fecal_n_mg + fecal_c_mg) / 1000.0;
    state.detritus.fine_detritus_g_total += fecal_mass_g;

    // ── Assimilated share ──
    let assimilated_n_mg = consumed_n_mg * ae;
    let assimilated_c_mg = consumed_c_mg * ae;

    // ── Excretion: TAN + DOC ──
    let excreted_n_mg = assimilated_n_mg * excr_frac;
    state.water.ammonia_total_mg_n_total += excreted_n_mg;
    state.water.alkalinity_meq_total += excreted_n_mg / MG_N_PER_MEQ_AMMONIA;

    let excreted_c_mg = assimilated_c_mg * excr_frac;
    state.water.dissolved_organic_carbon_mg_c_total += excreted_c_mg;

    // ── Respiration: O2 demand + DIC ──
    let target_respired_c_mg = assimilated_c_mg * resp_frac;
    let o2_demand_mg = target_respired_c_mg * o2_per_c;
    let actual_o2_consumed_mg = state
        .water
        .dissolved_oxygen_mg_total
        .max(0.0)
        .min(o2_demand_mg);
    let respiration_scale = if o2_demand_mg > f64::EPSILON {
        actual_o2_consumed_mg / o2_demand_mg
    } else {
        1.0
    };
    let respired_c_mg = target_respired_c_mg * respiration_scale;
    state.water.dissolved_inorganic_carbon_mg_c_total += respired_c_mg;
    state.water.dissolved_oxygen_mg_total =
        (state.water.dissolved_oxygen_mg_total - actual_o2_consumed_mg).max(0.0);

    let target_respired_n_mg = assimilated_n_mg * resp_frac;
    let respired_n_mg = target_respired_n_mg * respiration_scale;
    state.water.ammonia_total_mg_n_total += respired_n_mg;
    state.water.alkalinity_meq_total += respired_n_mg / MG_N_PER_MEQ_AMMONIA;

    // ── Retained: per-stage reserve ──
    let retained_n_mg = assimilated_n_mg * growth_frac + (target_respired_n_mg - respired_n_mg);
    let retained_c_mg = assimilated_c_mg * growth_frac + (target_respired_c_mg - respired_c_mg);
    let retained_mass_g = (retained_n_mg + retained_c_mg) / 1000.0;

    state.animal.add_reserve_by_feeding_units(retained_mass_g);
}

fn update_condition(state: &mut TankState) {
    let chemistry = state.concentrations();
    let tan_mg_l = chemistry.tan_mg_n_per_l();
    let nh3_mg_l = compute_nh3_mg_n_per_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = chemistry.nitrite_mg_n_per_l();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;
    let gh_d = chemistry.gh_d();

    // Compute population-level food factor using the same access-adjusted
    // routing target as shrimp_feeding().
    let target_food_g = shrimp_target_food_route_g(state);
    let food_factor = if target_food_g > f64::EPSILON {
        let satiation = (state.animal.daily_food_consumed_g / target_food_g).clamp(0.0, 1.0);
        0.4 + (0.6 * satiation)
    } else {
        1.0
    };
    let do_factor = (do_mg_l / 6.0).clamp(0.0, 1.0);
    let nh3_factor = (1.0 - nh3_mg_l * 3.0).clamp(0.0, 1.0);
    let nitrite_factor = (1.0 - nitrite_mg_l * 0.5).clamp(0.0, 1.0);

    let params = &state.shrimp_params;
    let temp_factor = temp_condition_factor(temp, params);
    let gh_factor = gh_mineral_factor(gh_d, params);
    let instability_factor = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    let stress_penalty = state.animal.hourly_nh3_stress_accum
        + state.animal.hourly_nitrite_stress_accum
        + state.animal.hourly_low_do_stress_accum
        + state.animal.hourly_heat_stress_accum
        + state.animal.hourly_instability_stress_accum;

    let target = (food_factor
        * do_factor
        * nh3_factor
        * nitrite_factor
        * temp_factor
        * gh_factor
        * instability_factor
        - stress_penalty * 0.5)
        .clamp(0.0, 1.0);

    let smoothing = state.process_params.shrimp_condition_smoothing;

    // Apply to each stage's condition independently.
    state.animal.adult.condition_index += smoothing * (target - state.animal.adult.condition_index);
    state.animal.adult.condition_index = state.animal.adult.condition_index.clamp(0.0, 1.0);

    state.animal.sub_adult.condition_index +=
        smoothing * (target - state.animal.sub_adult.condition_index);
    state.animal.sub_adult.condition_index = state.animal.sub_adult.condition_index.clamp(0.0, 1.0);

    state.animal.juvenile.condition_index +=
        smoothing * (target - state.animal.juvenile.condition_index);
    state.animal.juvenile.condition_index = state.animal.juvenile.condition_index.clamp(0.0, 1.0);
}

fn update_molt_stress(state: &mut TankState) {
    let gh_d = state.concentrations().gh_d();
    let params = &state.shrimp_params;

    let gh_stress = if gh_d < params.gh_min_d {
        (params.gh_min_d - gh_d) / params.gh_min_d
    } else {
        0.0
    };

    let instability_stress = state.stability_tracker.instability_index;
    let condition_stress = (0.5 - state.animal.population_condition_index()).max(0.0);
    let thermal_stress = if state.water.temperature_c > params.optimal_temp_max_c {
        ((state.water.temperature_c - params.optimal_temp_max_c) / 8.0).clamp(0.0, 0.5)
    } else {
        0.0
    };

    let hourly_pressure =
        state.animal.hourly_heat_stress_accum + state.animal.hourly_instability_stress_accum;

    let stress_pressure = (gh_stress * 0.3
        + instability_stress * 0.3
        + condition_stress * 0.2
        + thermal_stress * 0.2
        + hourly_pressure * 0.3)
        .clamp(0.0, 1.0);

    // Rises quickly, decays slowly
    if stress_pressure > state.animal.molt_stress_index {
        state.animal.molt_stress_index += 0.2 * (stress_pressure - state.animal.molt_stress_index);
    } else {
        state.animal.molt_stress_index += 0.05 * (stress_pressure - state.animal.molt_stress_index);
    }
    state.animal.molt_stress_index = state.animal.molt_stress_index.clamp(0.0, 1.0);
}

/// Explicit molt cycle: timer-based readiness, success/failure resolution,
/// and failed_molt_accum penalty tracking.
fn molt_cycle(state: &mut TankState) {
    if state.animal.total_count() == 0 {
        return;
    }

    let gh_d = state.concentrations().gh_d();
    let params = &state.shrimp_params;
    let base_interval = params.base_molt_interval_days.max(1.0);
    let temp_factor = temp_condition_factor(state.water.temperature_c, params).max(0.25);
    let effective_interval = (base_interval / temp_factor).max(1.0);

    let timer_factor = (state.animal.inter_molt_timer_days / effective_interval).clamp(0.0, 1.0);
    let mineral_factor = gh_mineral_factor(gh_d, params);
    let condition_factor = state.animal.population_condition_index();
    let instability_factor = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);
    let thermal_factor = temp_condition_factor(state.water.temperature_c, params);

    state.animal.molt_readiness = timer_factor;

    if timer_factor >= 1.0 {
        let success_score = (0.45 * mineral_factor
            + 0.35 * condition_factor
            + 0.10 * instability_factor
            + 0.10 * thermal_factor)
            .clamp(0.0, 1.0);
        state.animal.inter_molt_timer_days = 0.0;

        if success_score >= 0.55 {
            state.animal.last_molt_success = true;
            state.animal.failed_molt_accum = (state.animal.failed_molt_accum - 0.35).max(0.0);
        } else {
            state.animal.last_molt_success = false;
            state.animal.failed_molt_accum = (state.animal.failed_molt_accum + 0.3).min(1.0);
        }
    }

    // Advance timer
    state.animal.inter_molt_timer_days += 1.0;

    // Derive molt_stress_index from failed_molt_accum and current factors.
    // This grounds the existing index in explicit state rather than independent
    // integration. The update_molt_stress call has already set a baseline; we
    // blend in the failed_molt_accum contribution.
    let accum_stress = state.animal.failed_molt_accum * 0.5;
    state.animal.molt_stress_index =
        (state.animal.molt_stress_index + accum_stress).clamp(0.0, 1.0);
}

fn update_reproductive_readiness(state: &mut TankState) {
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_temp = temp_repro_factor(temp, params);
    let f_condition = state.animal.adult.condition_index;
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);
    let f_molt = (1.0 - state.animal.molt_stress_index).clamp(0.0, 1.0);

    let target = (f_temp * f_condition * f_stability * f_molt).clamp(0.0, 1.0);

    state.animal.reproductive_readiness_index +=
        0.1 * (target - state.animal.reproductive_readiness_index);
    state.animal.reproductive_readiness_index =
        state.animal.reproductive_readiness_index.clamp(0.0, 1.0);
}

fn spawning(state: &mut TankState) {
    if state.animal.adult.count == 0 {
        state.animal.spawn_progress_accum = 0.0;
        return;
    }

    let eligible = ((0.5 * state.animal.adult.count as f64)
        - state.animal.berried_females_count as f64)
        .max(0.0) as u32;

    if eligible == 0 {
        state.animal.spawn_progress_accum = 0.0;
        return;
    }

    let params = &state.shrimp_params;
    let gh_d = state.concentrations().gh_d();
    let f_mineral = gh_mineral_factor(gh_d, params);
    let spawn_rate =
        (params.base_spawn_rate * state.animal.reproductive_readiness_index * f_mineral)
            .clamp(0.0, 1.0);
    let new_berried =
        deterministic_transfer_count(eligible, spawn_rate, &mut state.animal.spawn_progress_accum);

    if new_berried > 0 {
        state.animal.berried_females_count += new_berried;
        // Create a new cohort for this batch of berried females
        state.animal.egg_cohorts.push(EggCohort {
            count: new_berried,
            progress_days: 0.0,
        });

        crate::systems::events::emit_once_per_day_pub(
            state,
            EventSeverity::Info,
            EventKind::ShrimpBerried,
            vec![EventCause::RoutineAction],
            format!("{new_berried} female(s) became berried"),
        );
    }
}

fn egg_development(state: &mut TankState) {
    if state.animal.egg_cohorts.is_empty() {
        // Legacy path: if berried females exist without cohorts (e.g. from old save),
        // migrate them into a single cohort using the current egg_progress_days.
        if state.animal.berried_females_count > 0 {
            state.animal.egg_cohorts.push(EggCohort {
                count: state.animal.berried_females_count,
                progress_days: state.animal.egg_progress_days,
            });
        } else {
            state.animal.hatch_success_carry = 0.0;
            return;
        }
    }

    let chemistry = state.concentrations();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_condition = state.animal.adult.condition_index;
    let f_oxygen = (do_mg_l / 6.0).clamp(0.0, 1.0);
    let f_temp = temp_repro_factor(temp, params);
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    let gh_d = chemistry.gh_d();
    let f_mineral = gh_mineral_factor(gh_d, params);

    let hatch_rate =
        (params.hatch_success_base * f_condition * f_oxygen * f_temp * f_stability * f_mineral)
            .clamp(0.0, 1.0);

    // Condition-dependent clutch size
    let adult_condition = state.animal.adult.condition_index;
    let effective_clutch_size = clutch_condition_modifier(adult_condition, params);

    let egg_duration = params.egg_duration_days as f64;
    let mut total_successful = 0u32;
    let mut total_failed = 0u32;
    let mut total_resource_limited = 0u32;
    let mut resolved_berried = 0u32;

    // Advance each cohort; resolve only those that have reached duration
    let mut i = 0;
    while i < state.animal.egg_cohorts.len() {
        state.animal.egg_cohorts[i].progress_days += 1.0;

        if state.animal.egg_cohorts[i].progress_days >= egg_duration {
            let cohort = state.animal.egg_cohorts.remove(i);
            resolved_berried += cohort.count;

            let successful_clutches = if effective_clutch_size == 0 {
                0
            } else {
                deterministic_transfer_count(
                    cohort.count,
                    hatch_rate,
                    &mut state.animal.hatch_success_carry,
                )
            };
            let failed_clutches = cohort.count.saturating_sub(successful_clutches);
            let funded_successful =
                fund_hatched_clutches(state, successful_clutches, effective_clutch_size);

            total_successful += funded_successful;
            total_failed += failed_clutches;
            total_resource_limited += successful_clutches.saturating_sub(funded_successful);
            // Don't increment i; the next cohort shifted into position
        } else {
            i += 1;
        }
    }

    // Successful hatches produce juveniles
    if total_successful > 0 {
        state.animal.juvenile.receive_entrants(
            total_successful * effective_clutch_size,
            0.0,
            state.animal.adult.condition_index,
        );
    }

    // Only resolved clutches leave berried_females_count
    state.animal.berried_females_count = state
        .animal
        .berried_females_count
        .saturating_sub(resolved_berried);

    if total_failed > 0 || total_resource_limited > 0 {
        let mut causes = Vec::new();
        if f_oxygen < 0.6 {
            causes.push(EventCause::LowOxygen);
        }
        if f_temp < 0.5 {
            causes.push(EventCause::HighTemperature);
        }
        if f_mineral < 0.6 {
            causes.push(EventCause::LowMinerals);
        }
        if f_stability < 0.6 {
            causes.push(EventCause::ChemistryInstability);
        }
        if total_resource_limited > 0 {
            causes.push(EventCause::Starvation);
        }
        if causes.is_empty() {
            causes.push(EventCause::PoorCondition);
        }

        crate::systems::events::emit_once_per_day_pub(
            state,
            EventSeverity::Warning,
            EventKind::EggFailure,
            causes,
            format!(
                "{} clutch(es) failed to hatch",
                total_failed + total_resource_limited
            ),
        );
    }

    // Update egg_progress_days from remaining cohorts for display/snapshot
    state.animal.sync_egg_progress_from_cohorts();
}

/// Condition-dependent effective clutch size. Returns the number of juveniles
/// per clutch adjusted by the mother's condition.
fn clutch_condition_modifier(adult_condition: f64, params: &ShrimpRuntimeParams) -> u32 {
    let base = params.base_clutch_size;
    let min_cond = params.min_clutch_condition;

    let modifier = if adult_condition >= 0.7 {
        1.0
    } else if adult_condition >= min_cond {
        // Linear taper from 1.0 at 0.7 to 0.5 at min_clutch_condition
        0.5 + 0.5 * (adult_condition - min_cond) / (0.7 - min_cond).max(f64::EPSILON)
    } else {
        0.0
    };

    (f64::from(base) * modifier).round().max(0.0) as u32
}

/// Juvenile -> sub-adult stage transition.
fn juvenile_to_subadult(state: &mut TankState) {
    if state.animal.juvenile.count == 0 {
        state.animal.juvenile.maturation_accum = 0.0;
        return;
    }

    let params = &state.shrimp_params;
    let base_rate = 1.0 / params.juvenile_to_subadult_days.max(1.0);
    let temp = state.water.temperature_c;
    let temp_scale = temp_condition_factor(temp, params);

    // Condition gate: only accumulate if juvenile condition is above threshold
    let condition_gate = if state.animal.juvenile.condition_index
        >= params.juvenile_maturation_condition_threshold
    {
        1.0
    } else {
        0.0
    };

    let daily_rate = base_rate * temp_scale * condition_gate;
    state.animal.juvenile.maturation_accum += state.animal.juvenile.count as f64 * daily_rate;

    let candidate_maturing =
        (state.animal.juvenile.maturation_accum.floor() as u32).min(state.animal.juvenile.count);
    state.animal.juvenile.maturation_accum -= candidate_maturing as f64;

    let growth_biomass_g = (SUB_ADULT_SHRIMP_BIOMASS_G - JUVENILE_SHRIMP_BIOMASS_G).max(0.0);
    let mut funded_maturing = 0u32;
    for _ in 0..candidate_maturing {
        if fund_live_shrimp_biomass(&mut state.animal.juvenile.reserve_g, growth_biomass_g) {
            funded_maturing += 1;
        } else {
            break;
        }
    }

    let unfunded_maturing = candidate_maturing.saturating_sub(funded_maturing);
    state.animal.juvenile.maturation_accum += f64::from(unfunded_maturing);
    let incoming_condition = state.animal.juvenile.condition_index;

    // Transfer proportional reserve from juvenile to sub_adult
    let mut reserve_transfer = 0.0;
    if funded_maturing > 0 {
        let pre_count = state.animal.juvenile.count;
        let transfer_fraction = if pre_count > 0 {
            f64::from(funded_maturing) / f64::from(pre_count)
        } else {
            0.0
        };
        reserve_transfer = state.animal.juvenile.reserve_g * transfer_fraction;
        state.animal.juvenile.reserve_g -= reserve_transfer;
    }

    state.animal.juvenile.count -= funded_maturing;
    state.animal.juvenile.clamp_maturation_accum_to_count();
    state
        .animal
        .sub_adult
        .receive_entrants(funded_maturing, reserve_transfer, incoming_condition);
}

/// Sub-adult -> adult stage transition.
fn subadult_to_adult(state: &mut TankState) {
    if state.animal.sub_adult.count == 0 {
        state.animal.sub_adult.maturation_accum = 0.0;
        return;
    }

    let params = &state.shrimp_params;
    let base_rate = 1.0 / params.subadult_to_adult_days.max(1.0);
    let temp = state.water.temperature_c;
    let temp_scale = temp_condition_factor(temp, params);

    // Condition gate: condition threshold AND last molt must have succeeded
    let condition_gate = if state.animal.sub_adult.condition_index
        >= params.subadult_maturation_condition_threshold
        && state.animal.last_molt_success
    {
        1.0
    } else {
        0.0
    };

    let daily_rate = base_rate * temp_scale * condition_gate;
    state.animal.sub_adult.maturation_accum += state.animal.sub_adult.count as f64 * daily_rate;

    let candidate_maturing =
        (state.animal.sub_adult.maturation_accum.floor() as u32).min(state.animal.sub_adult.count);
    state.animal.sub_adult.maturation_accum -= candidate_maturing as f64;

    let growth_biomass_g = (ADULT_SHRIMP_BIOMASS_G - SUB_ADULT_SHRIMP_BIOMASS_G).max(0.0);
    let mut funded_maturing = 0u32;
    for _ in 0..candidate_maturing {
        if fund_live_shrimp_biomass(&mut state.animal.sub_adult.reserve_g, growth_biomass_g) {
            funded_maturing += 1;
        } else {
            break;
        }
    }

    let unfunded_maturing = candidate_maturing.saturating_sub(funded_maturing);
    state.animal.sub_adult.maturation_accum += f64::from(unfunded_maturing);
    let incoming_condition = state.animal.sub_adult.condition_index;

    // Transfer proportional reserve from sub_adult to adult
    let mut reserve_transfer = 0.0;
    if funded_maturing > 0 {
        let pre_count = state.animal.sub_adult.count;
        let transfer_fraction = if pre_count > 0 {
            f64::from(funded_maturing) / f64::from(pre_count)
        } else {
            0.0
        };
        reserve_transfer = state.animal.sub_adult.reserve_g * transfer_fraction;
        state.animal.sub_adult.reserve_g -= reserve_transfer;
    }

    state.animal.sub_adult.count -= funded_maturing;
    state.animal.sub_adult.clamp_maturation_accum_to_count();
    state
        .animal
        .adult
        .receive_entrants(funded_maturing, reserve_transfer, incoming_condition);
}

fn mortality(state: &mut TankState) {
    let base_rate = state.process_params.shrimp_base_mortality_per_day;
    let stress_scale = state.process_params.shrimp_stress_mortality_scale;

    let pop_condition = state.animal.population_condition_index();
    let stress_total = state.animal.hourly_nh3_stress_accum
        + state.animal.hourly_nitrite_stress_accum
        + state.animal.hourly_low_do_stress_accum
        + state.animal.hourly_heat_stress_accum
        + (state.animal.molt_stress_index - 0.5).max(0.0)
        + (0.5 - pop_condition).max(0.0);

    // Failed molt accumulator contribution to mortality
    let molt_mortality =
        state.animal.failed_molt_accum * state.shrimp_params.failed_molt_mortality_scale;

    let p_adult_death = (base_rate + stress_total * stress_scale + molt_mortality).clamp(0.0, 0.5);

    let sub_adult_sensitivity = state.shrimp_params.sub_adult_sensitivity;
    let p_sub_adult_death =
        (base_rate + stress_total * stress_scale * sub_adult_sensitivity + molt_mortality)
            .clamp(0.0, 0.5);

    let juv_sensitivity = state.shrimp_params.juvenile_sensitivity;
    let p_juv_death = (base_rate + stress_total * stress_scale * juv_sensitivity + molt_mortality)
        .clamp(0.0, 0.5);

    let mut adult_deaths = 0u32;
    for _ in 0..state.animal.adult.count {
        if state.rng.next_f64() < p_adult_death {
            adult_deaths += 1;
        }
    }

    let mut sub_adult_deaths = 0u32;
    for _ in 0..state.animal.sub_adult.count {
        if state.rng.next_f64() < p_sub_adult_death {
            sub_adult_deaths += 1;
        }
    }

    let mut juv_deaths = 0u32;
    for _ in 0..state.animal.juvenile.count {
        if state.rng.next_f64() < p_juv_death {
            juv_deaths += 1;
        }
    }

    route_dead_shrimp_to_detritus(state, adult_deaths, sub_adult_deaths, juv_deaths);

    state.animal.adult.count = state.animal.adult.count.saturating_sub(adult_deaths);

    // Scale sub-adult maturation_accum so dead sub-adults' progress doesn't leak.
    let pre_sub = state.animal.sub_adult.count;
    state.animal.sub_adult.count = pre_sub.saturating_sub(sub_adult_deaths);
    if sub_adult_deaths > 0 && pre_sub > 0 {
        state.animal.sub_adult.maturation_accum *=
            state.animal.sub_adult.count as f64 / pre_sub as f64;
    }

    // Scale juvenile maturation_accum so dead juveniles' progress doesn't leak.
    let pre_juv = state.animal.juvenile.count;
    state.animal.juvenile.count = pre_juv.saturating_sub(juv_deaths);
    if juv_deaths > 0 && pre_juv > 0 {
        state.animal.juvenile.maturation_accum *=
            state.animal.juvenile.count as f64 / pre_juv as f64;
    }

    // Preserve berried_females <= adults invariant (also trims egg cohorts)
    state.animal.clamp_berried_to_adults();
}

/// Routes dead shrimp body biomass and reserve into fine_detritus_g_total.
///
/// C7 death/senescence interface for shrimp. Dead body mass and the
/// proportional reserve share enter the same fine_detritus_g_total pool
/// as feed-derived waste, plant senescence, and algae loss. From there,
/// the hourly nitrogen cycle (dissolution -> mineralization) processes
/// it identically to any other fine detritus.
/// See docs/MASS_FLOW.md, section 9.
fn route_dead_shrimp_to_detritus(
    state: &mut TankState,
    adult_deaths: u32,
    sub_adult_deaths: u32,
    juv_deaths: u32,
) {
    if adult_deaths == 0 && sub_adult_deaths == 0 && juv_deaths == 0 {
        return;
    }

    let detritus_fraction = state.process_params.death_biomass_to_detritus_fraction;
    debug_assert!(
        (detritus_fraction - 1.0).abs() <= DEATH_DETRITUS_FRACTION_TOLERANCE,
        "death_biomass_to_detritus_fraction must remain 1.0 until explicit export accounting exists; got {}",
        detritus_fraction,
    );

    // Route dead body biomass per stage
    let dead_biomass_g = (f64::from(adult_deaths) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(sub_adult_deaths) * SUB_ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(juv_deaths) * JUVENILE_SHRIMP_BIOMASS_G);
    state.detritus.fine_detritus_g_total += shrimp_body_detrital_mass_g(
        dead_biomass_g * detritus_fraction,
        state.shrimp_params.body_nitrogen_mg_per_g_wet_mass,
        state.shrimp_params.body_carbon_mg_per_g_wet_mass,
    );

    let reserve_transfer_g =
        state
            .animal
            .transfer_dead_reserve_g(adult_deaths, sub_adult_deaths, juv_deaths);
    if reserve_transfer_g > f64::EPSILON {
        state.detritus.fine_detritus_g_total += reserve_transfer_g * detritus_fraction;
    }
}

fn deterministic_transfer_count(available: u32, rate: f64, carry: &mut f64) -> u32 {
    let transfer_budget = f64::from(available) * rate.clamp(0.0, 1.0) + *carry;
    let transferred = transfer_budget.round().clamp(0.0, f64::from(available)) as u32;
    *carry = (transfer_budget - f64::from(transferred))
        .clamp(-DETERMINISTIC_CARRY_LIMIT, DETERMINISTIC_CARRY_LIMIT);
    transferred
}

/// Fund multiple hatched clutches from the adult reserve in one deterministic transfer.
fn fund_hatched_clutches(
    state: &mut TankState,
    requested_clutches: u32,
    effective_clutch_size: u32,
) -> u32 {
    if requested_clutches == 0 || effective_clutch_size == 0 {
        return 0;
    }

    let reserve_per_clutch = f64::from(effective_clutch_size)
        * JUVENILE_SHRIMP_BIOMASS_G
        * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    if reserve_per_clutch <= f64::EPSILON {
        return requested_clutches;
    }

    let affordable =
        ((state.animal.adult.reserve_g + f64::EPSILON) / reserve_per_clutch).floor() as u32;
    let funded = requested_clutches.min(affordable);
    state.animal.adult.reserve_g =
        (state.animal.adult.reserve_g - f64::from(funded) * reserve_per_clutch).max(0.0);
    funded
}

/// Attempts to fund growth biomass from a given reserve pool.
/// Returns true if the reserve was sufficient.
fn fund_live_shrimp_biomass(reserve: &mut f64, biomass_g: f64) -> bool {
    if biomass_g <= f64::EPSILON {
        return true;
    }

    let required_reserve_g = biomass_g * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    if *reserve + f64::EPSILON < required_reserve_g {
        return false;
    }

    *reserve -= required_reserve_g;
    true
}

fn refresh_carbonate_state(state: &mut TankState) {
    let volume_l = state.water_volume_l();
    resolve_carbonate_state(&mut state.water, volume_l);
}

fn emit_molt_stress_warning(state: &mut TankState) {
    if state.animal.molt_stress_index <= 0.6 {
        return;
    }

    let mut causes = Vec::new();
    let gh_d = state.concentrations().gh_d();

    if gh_d < state.shrimp_params.gh_min_d {
        causes.push(EventCause::LowMinerals);
    }
    if state.stability_tracker.instability_index > 0.3 {
        causes.push(EventCause::ChemistryInstability);
    }
    if state.water.temperature_c > state.shrimp_params.optimal_temp_max_c {
        causes.push(EventCause::HighTemperature);
    }
    if causes.is_empty() {
        causes.push(EventCause::PoorCondition);
    }

    crate::systems::events::emit_once_per_day_pub(
        state,
        EventSeverity::Warning,
        EventKind::MoltStressWarning,
        causes,
        format!(
            "Molt stress elevated: {:.2}",
            state.animal.molt_stress_index
        ),
    );
}

fn reset_hourly_accumulators(state: &mut TankState) {
    state.animal.hourly_nh3_stress_accum = 0.0;
    state.animal.hourly_nitrite_stress_accum = 0.0;
    state.animal.hourly_low_do_stress_accum = 0.0;
    state.animal.hourly_heat_stress_accum = 0.0;
    state.animal.hourly_instability_stress_accum = 0.0;
    state.animal.daily_food_consumed_g = 0.0;
}

// ── Factor functions ────────────────────────────────────────────────────────

/// Temperature factor for reproduction.
/// Best 22-26 C, clearly worse by 30 C, near-zero by 33 C.
fn temp_repro_factor(temp: f64, params: &ShrimpRuntimeParams) -> f64 {
    let opt_min = params.optimal_temp_min_c;
    let opt_max = params.optimal_temp_max_c;
    let penalty_start = params.high_temp_repro_penalty_start_c;
    let penalty_full = params.high_temp_repro_penalty_full_c;

    if temp < opt_min - 4.0 {
        0.1
    } else if temp < opt_min {
        0.1 + 0.9 * (temp - (opt_min - 4.0)) / 4.0
    } else if temp <= opt_max {
        1.0
    } else if temp <= penalty_start {
        let range = (penalty_start - opt_max).max(0.01);
        1.0 - 0.5 * (temp - opt_max) / range
    } else if temp <= penalty_full {
        let range = (penalty_full - penalty_start).max(0.01);
        0.5 - 0.45 * (temp - penalty_start) / range
    } else {
        0.05
    }
}

/// Temperature factor for general condition (broader bell than reproduction).
fn temp_condition_factor(temp: f64, params: &ShrimpRuntimeParams) -> f64 {
    if temp >= params.optimal_temp_min_c && temp <= params.optimal_temp_max_c {
        1.0
    } else if temp < params.optimal_temp_min_c {
        (1.0 - (params.optimal_temp_min_c - temp) / 10.0).clamp(0.2, 1.0)
    } else {
        (1.0 - (temp - params.optimal_temp_max_c) / 8.0).clamp(0.2, 1.0)
    }
}

/// GH/mineral factor for spawning and condition.
fn gh_mineral_factor(gh_d: f64, params: &ShrimpRuntimeParams) -> f64 {
    if gh_d >= params.gh_min_d && gh_d <= params.gh_max_d {
        1.0
    } else if gh_d < params.gh_min_d {
        (gh_d / params.gh_min_d.max(0.01)).clamp(0.3, 1.0)
    } else {
        (1.0 - (gh_d - params.gh_max_d) / 10.0).clamp(0.3, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        refresh_carbonate_state, route_consumed_food, shrimp_feeding, shrimp_grazing_access_factor,
        shrimp_target_food_route_g, step_daily_shrimp, update_condition, MG_N_PER_MEQ_AMMONIA,
    };
    use crate::{algae_detrital_mass_g, SimSeed, TankState, WaterState};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual} (tolerance {tolerance})"
        );
    }

    fn carbonate_condition_test_state() -> TankState {
        let mut state = TankState::new(SimSeed(10_001));
        state.geometry.length_cm = 40.0;
        state.geometry.width_cm = 30.0;
        state.geometry.height_cm = 35.0;
        state.geometry.fill_height_cm = 30.0;
        state.water = WaterState::default_for_volume_l(state.water_volume_l());
        state.water.temperature_c = 24.0;
        state.environment.ambient_temp_c = 24.0;

        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        state.water.ammonia_total_mg_n_total = 2.0 * volume_l;
        state.water.dissolved_inorganic_carbon_mg_c_total = 5.0 * volume_l;
        state.water.alkalinity_meq_total = 0.3 * volume_l;
        state.water.calcium_mg_total = 40.0 * volume_l;
        state.water.magnesium_mg_total = 10.0 * volume_l;
        state.algae.set_periphyton_total(5.0);

        state.animal.adult.count = 10;
        state.animal.adult.condition_index = 0.0;
        state.animal.sub_adult.condition_index = 0.0;
        state.animal.juvenile.condition_index = 0.0;
        state.animal.molt_stress_index = 0.0;
        state.animal.reproductive_readiness_index = 0.0;

        state.process_params.shrimp_condition_smoothing = 1.0;
        state.process_params.shrimp_base_mortality_per_day = 0.0;
        state.process_params.shrimp_stress_mortality_scale = 0.0;
        state.shrimp_params.base_spawn_rate = 0.0;

        refresh_carbonate_state(&mut state);
        state.reseed_stability_tracker();
        state.water.ph = 8.5;
        state
    }

    #[test]
    fn shrimp_feeding_tracks_daily_food_on_routing_mass_basis() {
        let mut state = TankState::new(SimSeed(9999));
        state.algae.set_periphyton_total(5.0);
        state.detritus.fine_detritus_g_total = 0.0;
        state.animal.adult.count = 10;

        let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
        let grazing_access_factor = shrimp_grazing_access_factor(&state);
        let total_feeding_units = state.animal.feeding_units();
        let expected_periphyton_biomass_removed = total_feeding_units
            * state
                .process_params
                .shrimp_periphyton_grazing_g_per_shrimp_per_day
            * grazing_access_factor;
        let expected_consumed_g =
            algae_detrital_mass_g(expected_periphyton_biomass_removed, n_to_c_ratio);

        shrimp_feeding(&mut state);

        assert!(
            (state.animal.daily_food_consumed_g - expected_consumed_g).abs() <= 1e-12,
            "expected {}, got {}",
            expected_consumed_g,
            state.animal.daily_food_consumed_g
        );
    }

    #[test]
    fn shrimp_feeding_caps_total_intake_on_routing_mass_basis() {
        let mut state = TankState::new(SimSeed(10_003));
        state.algae.set_periphyton_total(0.08);
        state.detritus.fine_detritus_g_total = 5.0;
        state.animal.adult.count = 10;
        state.animal.juvenile.count = 0;
        state
            .process_params
            .shrimp_periphyton_grazing_g_per_shrimp_per_day = 0.01;
        state.process_params.shrimp_assimilation_efficiency = 1.0 - 1e-12;
        for layer in &mut state.substrate_layers {
            layer.grazing_surface_index = 1.0;
        }

        let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
        let max_daily_intake_route_g = shrimp_target_food_route_g(&state);

        shrimp_feeding(&mut state);

        let periphyton_consumed_biomass_g = 0.08 - state.algae.periphyton_biomass_g;
        let periphyton_consumed_route_g =
            algae_detrital_mass_g(periphyton_consumed_biomass_g, n_to_c_ratio);
        let detritus_consumed_route_g = 5.0 - state.detritus.fine_detritus_g_total;

        assert_close(periphyton_consumed_biomass_g, 0.04, 1e-12);
        assert_close(
            periphyton_consumed_route_g + detritus_consumed_route_g,
            max_daily_intake_route_g,
            1e-9,
        );
        assert_close(
            state.animal.daily_food_consumed_g,
            periphyton_consumed_route_g + detritus_consumed_route_g,
            1e-9,
        );
    }

    #[test]
    fn update_condition_treats_full_accessible_ration_as_satiated() {
        let mut low_access = TankState::new(SimSeed(10_004));
        low_access.geometry.length_cm = 40.0;
        low_access.geometry.width_cm = 30.0;
        low_access.geometry.height_cm = 35.0;
        low_access.geometry.fill_height_cm = 30.0;
        low_access.water = WaterState::default_for_volume_l(low_access.water_volume_l());
        low_access.water.temperature_c = 24.0;
        low_access.environment.ambient_temp_c = 24.0;
        low_access.algae.set_periphyton_total(10.0);
        low_access.animal.adult.count = 10;
        low_access.animal.adult.condition_index = 0.0;
        low_access.process_params.shrimp_condition_smoothing = 1.0;

        let volume_l = low_access.water_volume_l();
        low_access.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        low_access.water.calcium_mg_total = 40.0 * volume_l;
        low_access.water.magnesium_mg_total = 10.0 * volume_l;

        let mut full_access = low_access.clone();
        for layer in &mut low_access.substrate_layers {
            layer.grazing_surface_index = 0.0;
        }
        for layer in &mut full_access.substrate_layers {
            layer.grazing_surface_index = 1.0;
        }

        refresh_carbonate_state(&mut low_access);
        low_access.reseed_stability_tracker();
        refresh_carbonate_state(&mut full_access);
        full_access.reseed_stability_tracker();

        low_access.animal.daily_food_consumed_g = shrimp_target_food_route_g(&low_access);
        full_access.animal.daily_food_consumed_g = shrimp_target_food_route_g(&full_access);
        assert!(
            full_access.animal.daily_food_consumed_g > low_access.animal.daily_food_consumed_g,
            "test setup expects the accessible ration to differ by grazing access"
        );

        update_condition(&mut low_access);
        update_condition(&mut full_access);

        assert_close(
            low_access.animal.adult.condition_index,
            full_access.animal.adult.condition_index,
            1e-3,
        );
    }

    #[test]
    fn oxygen_limited_respiration_scales_dissolved_fluxes_and_retains_shortfall() {
        let mut state = TankState::new(SimSeed(10_002));
        state.water.dissolved_oxygen_mg_total = 10.0;
        state.water.ammonia_total_mg_n_total = 0.0;
        state.water.alkalinity_meq_total = 0.0;
        state.water.dissolved_organic_carbon_mg_c_total = 0.0;
        state.water.dissolved_inorganic_carbon_mg_c_total = 0.0;
        state.animal.adult.reserve_g = 0.0;

        let consumed_n_mg = 16.0;
        let consumed_c_mg = 100.0;
        let params = &state.process_params;
        let ae = params.shrimp_assimilation_efficiency;
        let excr_frac = params.shrimp_excretion_fraction_of_assimilated;
        let growth_frac = params.shrimp_growth_fraction_of_assimilated;
        let resp_frac = params.shrimp_respiration_fraction_of_assimilated;
        let assimilated_n_mg = consumed_n_mg * ae;
        let assimilated_c_mg = consumed_c_mg * ae;
        let target_respired_n_mg = assimilated_n_mg * resp_frac;
        let target_respired_c_mg = assimilated_c_mg * resp_frac;
        let o2_demand_mg = target_respired_c_mg * params.shrimp_o2_per_mg_c_respired;
        let respiration_scale = state.water.dissolved_oxygen_mg_total / o2_demand_mg;

        // Route with all feeding weight going to adults (no sub-adults or juveniles).
        state.animal.adult.count = 1;
        route_consumed_food(&mut state, consumed_n_mg, consumed_c_mg);

        assert_close(state.water.dissolved_oxygen_mg_total, 0.0, 1e-12);
        assert_close(
            state.water.dissolved_inorganic_carbon_mg_c_total,
            target_respired_c_mg * respiration_scale,
            1e-12,
        );
        assert_close(
            state.water.ammonia_total_mg_n_total,
            assimilated_n_mg * excr_frac + target_respired_n_mg * respiration_scale,
            1e-12,
        );
        assert_close(
            state.water.alkalinity_meq_total,
            (assimilated_n_mg * excr_frac + target_respired_n_mg * respiration_scale)
                / MG_N_PER_MEQ_AMMONIA,
            1e-12,
        );
        assert_close(
            state.animal.adult.reserve_g,
            (assimilated_n_mg * growth_frac
                + (target_respired_n_mg - target_respired_n_mg * respiration_scale)
                + assimilated_c_mg * growth_frac
                + (target_respired_c_mg - target_respired_c_mg * respiration_scale))
                / 1000.0,
            1e-12,
        );
    }

    #[test]
    fn step_daily_shrimp_refreshes_carbonate_before_condition() {
        let mut expected = carbonate_condition_test_state();
        shrimp_feeding(&mut expected);
        refresh_carbonate_state(&mut expected);
        update_condition(&mut expected);

        let mut actual = carbonate_condition_test_state();
        let stale_ph = actual.water.ph;
        step_daily_shrimp(&mut actual);

        assert!(
            (actual.water.ph - stale_ph).abs() > 1e-6,
            "test setup must start from a stale carbonate cache"
        );
        assert_close(actual.water.ph, expected.water.ph, 1e-9);
        assert_close(
            actual.animal.adult.condition_index,
            expected.animal.adult.condition_index,
            1e-9,
        );
    }
}
