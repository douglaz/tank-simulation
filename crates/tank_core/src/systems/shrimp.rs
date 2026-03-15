use crate::systems::chemistry::compute_nh3_mg_l;
use crate::types::{
    EggCohort, EventCause, EventKind, EventSeverity, ShrimpRuntimeParams, TankState,
};

// ── Hourly ──────────────────────────────────────────────────────────────────

/// Accumulates NH3, nitrite, low-DO, heat, and instability stress each hour.
/// Runs after chemistry/DO updates, before hourly event emission.
pub fn step_hourly_shrimp_stress(state: &mut TankState) {
    let total = state.animal.adults_count + state.animal.juveniles_count;
    if total == 0 {
        return;
    }

    let volume_l = state.geometry.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let tan_mg_l = state.water.ammonia_total_mg_n_total / volume_l;
    let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = state.water.nitrite_mg_n_total / volume_l;
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;
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

/// Full daily shrimp lifecycle: feeding → condition → molt stress →
/// reproductive readiness → spawning → egg development/hatching →
/// juvenile recruitment → mortality → events.
pub fn step_daily_shrimp(state: &mut TankState) {
    let total = state.animal.adults_count + state.animal.juveniles_count;
    if total == 0 && state.animal.berried_females_count == 0 {
        reset_hourly_accumulators(state);
        return;
    }

    let volume_l = state.geometry.water_volume_l();
    if volume_l <= f64::EPSILON {
        reset_hourly_accumulators(state);
        return;
    }

    shrimp_feeding(state, volume_l);
    update_condition(state, volume_l);
    update_molt_stress(state, volume_l);
    update_reproductive_readiness(state);
    spawning(state);
    egg_development(state);
    juvenile_recruitment(state);
    mortality(state);
    emit_molt_stress_warning(state, volume_l);

    reset_hourly_accumulators(state);
}

/// Updates the stability tracker at the end of each daily cycle.
pub fn update_stability_tracker(state: &mut TankState) {
    let volume_l = state.geometry.water_volume_l().max(f64::EPSILON);
    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;

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

fn shrimp_feeding(state: &mut TankState, _volume_l: f64) {
    let adults = state.animal.adults_count as f64;
    let juveniles = state.animal.juveniles_count as f64;
    let total_feeding_units = adults + juveniles * 0.3;
    let rate = state
        .process_params
        .shrimp_periphyton_grazing_g_per_shrimp_per_day;
    let food_demand = total_feeding_units * rate;

    // Shrimp graze periphyton (at most 50% of available per day)
    let max_periph = state.algae.periphyton_biomass_g * 0.5;
    let periph_consumed = food_demand.min(max_periph).max(0.0);
    state.algae.periphyton_biomass_g =
        (state.algae.periphyton_biomass_g - periph_consumed).max(0.0);

    // Shrimp also eat fine detritus (biofilm, decomposing organic matter)
    let detritus_demand = total_feeding_units * rate;
    let max_detritus = state.detritus.fine_detritus_g_total * 0.2;
    let detritus_consumed = detritus_demand.min(max_detritus).max(0.0);
    state.detritus.fine_detritus_g_total =
        (state.detritus.fine_detritus_g_total - detritus_consumed).max(0.0);
}

fn update_condition(state: &mut TankState, volume_l: f64) {
    let tan_mg_l = state.water.ammonia_total_mg_n_total / volume_l;
    let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = state.water.nitrite_mg_n_total / volume_l;
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;
    let temp = state.water.temperature_c;

    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;

    let total_shrimp = state.animal.adults_count as f64 + state.animal.juveniles_count as f64;
    let available_food =
        state.algae.periphyton_biomass_g + state.detritus.fine_detritus_g_total * 0.3;
    let food_per_shrimp = if total_shrimp > 0.0 {
        available_food / total_shrimp
    } else {
        1.0
    };

    let food_factor = (food_per_shrimp / 0.05).clamp(0.0, 1.0);
    let do_factor = (do_mg_l / 6.0).clamp(0.0, 1.0);
    // NH3 risk: condition degrades steadily, reaching zero at ~0.33 mg/L NH3
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
    state.animal.condition_index += smoothing * (target - state.animal.condition_index);
    state.animal.condition_index = state.animal.condition_index.clamp(0.0, 1.0);
}

fn update_molt_stress(state: &mut TankState, volume_l: f64) {
    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
    let params = &state.shrimp_params;

    let gh_stress = if gh_d < params.gh_min_d {
        (params.gh_min_d - gh_d) / params.gh_min_d
    } else {
        0.0
    };

    let instability_stress = state.stability_tracker.instability_index;
    let condition_stress = (0.5 - state.animal.condition_index).max(0.0);
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

fn update_reproductive_readiness(state: &mut TankState) {
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_temp = temp_repro_factor(temp, params);
    let f_condition = state.animal.condition_index;
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);
    let f_molt = (1.0 - state.animal.molt_stress_index).clamp(0.0, 1.0);

    let target = (f_temp * f_condition * f_stability * f_molt).clamp(0.0, 1.0);

    state.animal.reproductive_readiness_index +=
        0.1 * (target - state.animal.reproductive_readiness_index);
    state.animal.reproductive_readiness_index =
        state.animal.reproductive_readiness_index.clamp(0.0, 1.0);
}

fn spawning(state: &mut TankState) {
    if state.animal.adults_count == 0 {
        return;
    }

    let eligible = ((0.5 * state.animal.adults_count as f64)
        - state.animal.berried_females_count as f64)
        .max(0.0) as u32;

    if eligible == 0 {
        return;
    }

    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_temp = temp_repro_factor(temp, params);
    let f_condition = state.animal.condition_index;
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    let volume_l = state.geometry.water_volume_l().max(f64::EPSILON);
    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
    let f_mineral = gh_mineral_factor(gh_d, params);

    let p_spawn = params.base_spawn_rate * f_temp * f_condition * f_stability * f_mineral;

    let mut new_berried = 0u32;
    for _ in 0..eligible {
        if state.rng.next_f64() < p_spawn {
            new_berried += 1;
        }
    }

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
            return;
        }
    }

    let volume_l = state.geometry.water_volume_l().max(f64::EPSILON);
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_condition = state.animal.condition_index;
    let f_oxygen = (do_mg_l / 6.0).clamp(0.0, 1.0);
    let f_temp = temp_repro_factor(temp, params);
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;
    let f_mineral = gh_mineral_factor(gh_d, params);

    let p_hatch =
        params.hatch_success_base * f_condition * f_oxygen * f_temp * f_stability * f_mineral;

    let egg_duration = params.egg_duration_days as f64;
    let mut total_successful = 0u32;
    let mut total_failed = 0u32;
    let mut resolved_berried = 0u32;

    // Advance each cohort; resolve only those that have reached duration
    let mut i = 0;
    while i < state.animal.egg_cohorts.len() {
        state.animal.egg_cohorts[i].progress_days += 1.0;

        if state.animal.egg_cohorts[i].progress_days >= egg_duration {
            let cohort = state.animal.egg_cohorts.remove(i);
            resolved_berried += cohort.count;

            for _ in 0..cohort.count {
                if state.rng.next_f64() < p_hatch {
                    total_successful += 1;
                } else {
                    total_failed += 1;
                }
            }
            // Don't increment i; the next cohort shifted into position
        } else {
            i += 1;
        }
    }

    // Successful hatches produce juveniles (~25 per clutch for Neocaridina)
    let juveniles_per_clutch = 25u32;
    if total_successful > 0 {
        state.animal.juveniles_count += total_successful * juveniles_per_clutch;
    }

    // Only resolved clutches leave berried_females_count
    state.animal.berried_females_count = state
        .animal
        .berried_females_count
        .saturating_sub(resolved_berried);

    if total_failed > 0 {
        // Failure-mode invariant: on hatch failure, juveniles_count does NOT increase
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
        if causes.is_empty() {
            causes.push(EventCause::PoorCondition);
        }

        crate::systems::events::emit_once_per_day_pub(
            state,
            EventSeverity::Warning,
            EventKind::EggFailure,
            causes,
            format!("{total_failed} clutch(es) failed to hatch"),
        );
    }

    // Update egg_progress_days from remaining cohorts for display/snapshot
    state.animal.sync_egg_progress_from_cohorts();
}

fn juvenile_recruitment(state: &mut TankState) {
    if state.animal.juveniles_count == 0 {
        return;
    }

    let maturation_rate = 1.0
        / state
            .process_params
            .shrimp_juvenile_maturation_days
            .max(1.0);
    let maturing = (state.animal.juveniles_count as f64 * maturation_rate).floor() as u32;
    let maturing = maturing.min(state.animal.juveniles_count);

    state.animal.juveniles_count -= maturing;
    state.animal.adults_count += maturing;
}

fn mortality(state: &mut TankState) {
    let base_rate = state.process_params.shrimp_base_mortality_per_day;
    let stress_scale = state.process_params.shrimp_stress_mortality_scale;

    let stress_total = state.animal.hourly_nh3_stress_accum
        + state.animal.hourly_nitrite_stress_accum
        + state.animal.hourly_low_do_stress_accum
        + state.animal.hourly_heat_stress_accum
        + (state.animal.molt_stress_index - 0.5).max(0.0)
        + (0.5 - state.animal.condition_index).max(0.0);

    let p_adult_death = (base_rate + stress_total * stress_scale).clamp(0.0, 0.5);

    let juv_sensitivity = state.shrimp_params.juvenile_sensitivity;
    let p_juv_death = (base_rate + stress_total * stress_scale * juv_sensitivity).clamp(0.0, 0.5);

    let mut adult_deaths = 0u32;
    for _ in 0..state.animal.adults_count {
        if state.rng.next_f64() < p_adult_death {
            adult_deaths += 1;
        }
    }

    let mut juv_deaths = 0u32;
    for _ in 0..state.animal.juveniles_count {
        if state.rng.next_f64() < p_juv_death {
            juv_deaths += 1;
        }
    }

    state.animal.adults_count = state.animal.adults_count.saturating_sub(adult_deaths);
    state.animal.juveniles_count = state.animal.juveniles_count.saturating_sub(juv_deaths);

    // Preserve berried_females <= adults invariant (also trims egg cohorts)
    state.animal.clamp_berried_to_adults();
}

fn emit_molt_stress_warning(state: &mut TankState, volume_l: f64) {
    if state.animal.molt_stress_index <= 0.6 {
        return;
    }

    let mut causes = Vec::new();
    let ca_mg_l = state.water.calcium_mg_total / volume_l;
    let mg_mg_l = state.water.magnesium_mg_total / volume_l;
    let gh_d = ((2.497 * ca_mg_l) + (4.118 * mg_mg_l)) / 17.848;

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
}

// ── Factor functions ────────────────────────────────────────────────────────

/// Temperature factor for reproduction.
/// Best 22–26 °C, clearly worse by 30 °C, near-zero by 33 °C.
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
