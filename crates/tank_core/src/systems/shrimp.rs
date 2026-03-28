use crate::systems::chemistry::{compute_nh3_mg_l, resolve_carbonate_state};
use crate::types::{
    algae_carbon_mg, algae_detrital_mass_g, algae_nitrogen_mg, detritus_carbon_mg,
    detritus_nitrogen_mg, live_biomass_carbon_mg, live_biomass_detrital_mass_g,
    live_biomass_nitrogen_mg, EggCohort, EventCause, EventKind, EventSeverity, ShrimpRuntimeParams,
    TankState, ADULT_SHRIMP_BIOMASS_G, JUVENILE_SHRIMP_BIOMASS_G,
    LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
};

const JUVENILES_PER_CLUTCH: u32 = 25;

// ── Hourly ──────────────────────────────────────────────────────────────────

/// Accumulates NH3, nitrite, low-DO, heat, and instability stress each hour.
/// Runs after chemistry/DO updates, before hourly event emission.
pub fn step_hourly_shrimp_stress(state: &mut TankState) {
    let total = state.animal.adults_count + state.animal.juveniles_count;
    if total == 0 {
        return;
    }

    let chemistry = state.concentrations();
    let volume_l = chemistry.volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let tan_mg_l = chemistry.tan_mg_n_per_l();
    let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, state.water.ph, state.water.temperature_c);
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

/// Full daily shrimp lifecycle: feeding → condition → molt stress →
/// reproductive readiness → spawning → egg development/hatching →
/// juvenile recruitment → mortality → events.
pub fn step_daily_shrimp(state: &mut TankState) {
    let total = state.animal.adults_count + state.animal.juveniles_count;
    if total == 0 && state.animal.berried_females_count == 0 {
        reset_hourly_accumulators(state);
        return;
    }

    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        reset_hourly_accumulators(state);
        return;
    }

    shrimp_feeding(state);
    update_condition(state);
    update_molt_stress(state);
    update_reproductive_readiness(state);
    spawning(state);
    egg_development(state);
    juvenile_recruitment(state);
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

fn shrimp_feeding(state: &mut TankState) {
    let adults = state.animal.adults_count as f64;
    let juveniles = state.animal.juveniles_count as f64;
    let total_feeding_units = adults + juveniles * 0.3;
    if total_feeding_units <= f64::EPSILON {
        state.animal.daily_food_consumed_g = 0.0;
        return;
    }

    let rate = state
        .process_params
        .shrimp_periphyton_grazing_g_per_shrimp_per_day;
    let grazing_access_factor =
        0.5 + (0.5 * state.avg_substrate_index(|layer| layer.grazing_surface_index));
    let food_demand = total_feeding_units * rate * grazing_access_factor;

    // Shrimp graze periphyton (at most 50% of available per day)
    let max_periph = state.algae.periphyton_biomass_g * 0.5;
    let periph_consumed = food_demand.min(max_periph).max(0.0);
    state.algae.periphyton_biomass_g =
        (state.algae.periphyton_biomass_g - periph_consumed).max(0.0);

    // Shrimp also eat fine detritus (biofilm, decomposing organic matter)
    let detritus_demand = total_feeding_units * rate * grazing_access_factor;
    let max_detritus = state.detritus.fine_detritus_g_total * 0.2;
    let detritus_consumed = detritus_demand.min(max_detritus).max(0.0);
    state.detritus.fine_detritus_g_total =
        (state.detritus.fine_detritus_g_total - detritus_consumed).max(0.0);

    // Convert consumed food to elemental N and C for routing.
    // Phase-1 simplification: periphyton and fine detritus share the same
    // food-quality assumption (feed_n_to_c_ratio). This is documented in
    // ProcessParams and acceptable because both food sources are mixed
    // organic matter at similar N:C in a shrimp tank.
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let periph_consumed_route_g = algae_detrital_mass_g(periph_consumed, n_to_c_ratio);
    state.animal.daily_food_consumed_g = periph_consumed_route_g + detritus_consumed;
    let consumed_n_mg =
        algae_nitrogen_mg(periph_consumed) + detritus_nitrogen_mg(detritus_consumed, n_to_c_ratio);
    let consumed_c_mg = algae_carbon_mg(periph_consumed, n_to_c_ratio)
        + detritus_carbon_mg(detritus_consumed, n_to_c_ratio);

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
/// - feces → fine_detritus_g_total (as organic matter grams)
/// - excreted N → ammonia_total_mg_n_total (TAN)
/// - excreted C → dissolved_organic_carbon_mg_c_total (DOC)
/// - respired C → dissolved_inorganic_carbon_mg_c_total (DIC)
/// - respired → O2 demand (dissolved_oxygen_mg_total)
/// - retained → animal.reserve_g (organic matter grams)
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
    // ── Feces: unassimilated share → fine detritus ──
    let fecal_n_mg = consumed_n_mg * fecal_fraction;
    let fecal_c_mg = consumed_c_mg * fecal_fraction;
    // Convert elemental mg back to organic-matter grams for the detritus pool.
    // organic_matter_g = (N_mg + C_mg) / 1000.0
    let fecal_mass_g = (fecal_n_mg + fecal_c_mg) / 1000.0;
    state.detritus.fine_detritus_g_total += fecal_mass_g;

    // ── Assimilated share ──
    let assimilated_n_mg = consumed_n_mg * ae;
    let assimilated_c_mg = consumed_c_mg * ae;

    // ── Excretion: TAN + DOC ──
    // Phase-1 lumping: dissolved N excretion goes to TAN and dissolved C
    // excretion goes to DOC. We still do not create separate urea or DON pools.
    let excreted_n_mg = assimilated_n_mg * excr_frac;
    state.water.ammonia_total_mg_n_total += excreted_n_mg;

    // Excreted C goes to the canonical DOC pool.
    let excreted_c_mg = assimilated_c_mg * excr_frac;
    state.water.dissolved_organic_carbon_mg_c_total += excreted_c_mg;

    // ── Respiration: O2 demand + DIC ──
    let respired_c_mg = assimilated_c_mg * resp_frac;
    state.water.dissolved_inorganic_carbon_mg_c_total += respired_c_mg;

    // O2 consumption: mg O2 = mg C respired × respiratory quotient
    let o2_demand_mg = respired_c_mg * o2_per_c;
    state.water.dissolved_oxygen_mg_total =
        (state.water.dissolved_oxygen_mg_total - o2_demand_mg).max(0.0);

    // Respired N is released as TAN (nitrogen from oxidized amino acids).
    let respired_n_mg = assimilated_n_mg * resp_frac;
    state.water.ammonia_total_mg_n_total += respired_n_mg;

    // ── Retained: body reserve ──
    // Phase 1 only accumulates this retained share. Later reserve/condition
    // work will add maintenance, molt, and reproduction drains so reserve_g
    // does not grow without bound over multi-year runs.
    let retained_n_mg = assimilated_n_mg * growth_frac;
    let retained_c_mg = assimilated_c_mg * growth_frac;
    // Convert back to organic-matter grams for the reserve pool.
    let retained_mass_g = (retained_n_mg + retained_c_mg) / 1000.0;
    state.animal.reserve_g += retained_mass_g;
}

fn update_condition(state: &mut TankState) {
    let chemistry = state.concentrations();
    let tan_mg_l = chemistry.tan_mg_n_per_l();
    let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = chemistry.nitrite_mg_n_per_l();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;

    let gh_d = chemistry.gh_d();

    let total_feeding_units =
        state.animal.adults_count as f64 + state.animal.juveniles_count as f64 * 0.3;
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let target_food_g = total_feeding_units
        * state
            .process_params
            .shrimp_periphyton_grazing_g_per_shrimp_per_day
        * (1.0 + algae_detrital_mass_g(1.0, n_to_c_ratio));
    let food_factor = if target_food_g > f64::EPSILON {
        let satiation = (state.animal.daily_food_consumed_g / target_food_g).clamp(0.0, 1.0);
        0.4 + (0.6 * satiation)
    } else {
        1.0
    };
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

fn update_molt_stress(state: &mut TankState) {
    let gh_d = state.concentrations().gh_d();
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

    let gh_d = state.concentrations().gh_d();
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

    let chemistry = state.concentrations();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_condition = state.animal.condition_index;
    let f_oxygen = (do_mg_l / 6.0).clamp(0.0, 1.0);
    let f_temp = temp_repro_factor(temp, params);
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    let gh_d = chemistry.gh_d();
    let f_mineral = gh_mineral_factor(gh_d, params);

    let p_hatch =
        params.hatch_success_base * f_condition * f_oxygen * f_temp * f_stability * f_mineral;

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

            for _ in 0..cohort.count {
                if state.rng.next_f64() < p_hatch {
                    if fund_hatched_clutch_biomass(state) {
                        total_successful += 1;
                    } else {
                        total_resource_limited += 1;
                    }
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
    if total_successful > 0 {
        state.animal.juveniles_count += total_successful * JUVENILES_PER_CLUTCH;
    }

    // Only resolved clutches leave berried_females_count
    state.animal.berried_females_count = state
        .animal
        .berried_females_count
        .saturating_sub(resolved_berried);

    if total_failed > 0 || total_resource_limited > 0 {
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

fn juvenile_recruitment(state: &mut TankState) {
    if state.animal.juveniles_count == 0 {
        // Reset accumulator so stale progress doesn't leak to the next cohort.
        state.animal.maturation_accum = 0.0;
        return;
    }

    let maturation_rate = 1.0
        / state
            .process_params
            .shrimp_juvenile_maturation_days
            .max(1.0);
    // Use a fractional accumulator so small cohorts don't freeze (floor=0)
    // or mature too fast (max(1)). The fractional remainder carries over to
    // the next day.
    state.animal.maturation_accum += state.animal.juveniles_count as f64 * maturation_rate;
    let candidate_maturing =
        (state.animal.maturation_accum.floor() as u32).min(state.animal.juveniles_count);
    state.animal.maturation_accum -= candidate_maturing as f64;

    let growth_biomass_g = (ADULT_SHRIMP_BIOMASS_G - JUVENILE_SHRIMP_BIOMASS_G).max(0.0);
    let mut funded_maturing = 0u32;
    for _ in 0..candidate_maturing {
        if fund_live_shrimp_biomass(state, growth_biomass_g) {
            funded_maturing += 1;
        } else {
            break;
        }
    }

    let unfunded_maturing = candidate_maturing.saturating_sub(funded_maturing);
    state.animal.maturation_accum += f64::from(unfunded_maturing);
    state.animal.juveniles_count -= funded_maturing;
    state.animal.adults_count += funded_maturing;
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

    route_dead_shrimp_to_detritus(state, adult_deaths, juv_deaths);

    state.animal.adults_count = state.animal.adults_count.saturating_sub(adult_deaths);

    // Scale maturation_accum so dead juveniles' progress doesn't leak to survivors.
    let pre_juv = state.animal.juveniles_count;
    state.animal.juveniles_count = pre_juv.saturating_sub(juv_deaths);
    if juv_deaths > 0 && pre_juv > 0 {
        state.animal.maturation_accum *= state.animal.juveniles_count as f64 / pre_juv as f64;
    }

    // Preserve berried_females <= adults invariant (also trims egg cohorts)
    state.animal.clamp_berried_to_adults();
}

fn route_dead_shrimp_to_detritus(state: &mut TankState, adult_deaths: u32, juv_deaths: u32) {
    if adult_deaths == 0 && juv_deaths == 0 {
        return;
    }

    let total_shrimp = state.animal.adults_count + state.animal.juveniles_count;
    let total_dead = adult_deaths + juv_deaths;

    let dead_biomass_g = (f64::from(adult_deaths) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(juv_deaths) * JUVENILE_SHRIMP_BIOMASS_G);
    state.detritus.fine_detritus_g_total +=
        live_biomass_detrital_mass_g(dead_biomass_g, state.process_params.feed_n_to_c_ratio);

    // Proportionally transfer dead shrimp's share of the reserve pool to detritus.
    if total_shrimp > 0 && state.animal.reserve_g > f64::EPSILON {
        let dead_fraction = f64::from(total_dead) / f64::from(total_shrimp);
        let reserve_transfer_g = state.animal.reserve_g * dead_fraction;
        state.animal.reserve_g -= reserve_transfer_g;
        state.detritus.fine_detritus_g_total += reserve_transfer_g;
    }
}

fn fund_hatched_clutch_biomass(state: &mut TankState) -> bool {
    fund_live_shrimp_biomass(
        state,
        f64::from(JUVENILES_PER_CLUTCH) * JUVENILE_SHRIMP_BIOMASS_G,
    )
}

fn withdraw_from_preferred_pools(preferred: &mut f64, fallback: &mut f64, amount_mg: f64) {
    let preferred_withdrawal = preferred.min(amount_mg);
    *preferred -= preferred_withdrawal;
    let remaining = amount_mg - preferred_withdrawal;
    if remaining > f64::EPSILON {
        *fallback = (*fallback - remaining).max(0.0);
    }
}

fn fund_live_shrimp_biomass(state: &mut TankState, biomass_g: f64) -> bool {
    if biomass_g <= f64::EPSILON {
        return true;
    }

    let required_reserve_g = biomass_g * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    let reserve_to_spend_g = state.animal.reserve_g.min(required_reserve_g);
    let reserve_shortfall_g = required_reserve_g - reserve_to_spend_g;
    if reserve_shortfall_g <= f64::EPSILON {
        state.animal.reserve_g -= reserve_to_spend_g;
        return true;
    }

    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let water_biomass_equivalent_g = reserve_shortfall_g / LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G;
    let required_nitrogen_mg = live_biomass_nitrogen_mg(water_biomass_equivalent_g, n_to_c_ratio);
    let required_carbon_mg = live_biomass_carbon_mg(water_biomass_equivalent_g, n_to_c_ratio);
    let available_nitrogen_mg =
        state.water.dissolved_organic_nitrogen_mg_n_total + state.water.ammonia_total_mg_n_total;
    let available_carbon_mg = state.water.dissolved_organic_carbon_mg_c_total
        + state.water.dissolved_inorganic_carbon_mg_c_total;

    if available_nitrogen_mg + f64::EPSILON < required_nitrogen_mg
        || available_carbon_mg + f64::EPSILON < required_carbon_mg
    {
        return false;
    }

    state.animal.reserve_g -= reserve_to_spend_g;

    withdraw_from_preferred_pools(
        &mut state.water.dissolved_organic_nitrogen_mg_n_total,
        &mut state.water.ammonia_total_mg_n_total,
        required_nitrogen_mg,
    );
    withdraw_from_preferred_pools(
        &mut state.water.dissolved_organic_carbon_mg_c_total,
        &mut state.water.dissolved_inorganic_carbon_mg_c_total,
        required_carbon_mg,
    );

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

#[cfg(test)]
mod tests {
    use super::shrimp_feeding;
    use crate::{algae_detrital_mass_g, SimSeed, TankState};

    #[test]
    fn shrimp_feeding_tracks_daily_food_on_routing_mass_basis() {
        let mut state = TankState::new(SimSeed(9999));
        state.algae.periphyton_biomass_g = 5.0;
        state.detritus.fine_detritus_g_total = 0.0;
        state.animal.adults_count = 10;

        let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
        let grazing_access_factor =
            0.5 + (0.5 * state.avg_substrate_index(|layer| layer.grazing_surface_index));
        let expected_periphyton_biomass_removed = 10.0
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
}
