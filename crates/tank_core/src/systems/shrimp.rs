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

#[derive(Debug, Clone, Copy)]
struct MoltConditionBreakdown {
    modifier: f64,
    reserve_factor: f64,
    reserve_per_shrimp_g: f64,
    reserve_target_g: f64,
}

#[derive(Debug, Clone, Copy)]
struct FailedMoltDiagnostics {
    min_condition_index: f64,
    poor_condition_involved: bool,
    reserve_limited: bool,
    min_reserve_per_shrimp_g: f64,
    reserve_target_g: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NitriteStressDiagnostics {
    pub nitrite_mg_l: f64,
    pub chloride_mg_l: f64,
    pub effective_hazard_mg_l: f64,
    pub hourly_stress_increment: f64,
}

#[derive(Debug, Clone, Copy)]
struct MortalityProbabilities {
    adult: f64,
    sub_adult: f64,
    juvenile: f64,
}

// ── Hourly ──────────────────────────────────────────────────────────────────

/// Computes the effective nitrite hazard after chloride protection.
///
/// Chloride competes with nitrite for uptake at crustacean gill sites.
/// High chloride relative to nitrite reduces the effective toxic dose.
///
/// Formula: `effective = [NO2] / (1 + chloride_protection_factor × [Cl] / [NO2])`
///
/// Properties:
/// - Returns 0.0 when nitrite is zero regardless of chloride.
/// - Returns raw nitrite when chloride is zero (no protection).
/// - Monotonically decreasing in chloride for fixed nitrite > 0.
///
/// Confidence: medium. Directionally well-supported by freshwater crustacean
/// literature; scalar calibrated so Cl:NO2 > 10:1 yields < 20% hazard.
pub fn compute_effective_nitrite_hazard(
    nitrite_mg_l: f64,
    chloride_mg_l: f64,
    chloride_protection_factor: f64,
) -> f64 {
    if nitrite_mg_l <= 0.0 {
        return 0.0;
    }
    if chloride_protection_factor <= 0.0 || chloride_mg_l <= 0.0 {
        return nitrite_mg_l;
    }
    let cl_no2_ratio = chloride_mg_l / nitrite_mg_l;
    nitrite_mg_l / (1.0 + chloride_protection_factor * cl_no2_ratio)
}

fn compute_hourly_nitrite_stress_increment(effective_nitrite_mg_l: f64) -> f64 {
    if effective_nitrite_mg_l > 0.5 {
        (effective_nitrite_mg_l - 0.5) * 0.5 / 24.0
    } else {
        0.0
    }
}

pub(crate) fn compute_nitrite_stress_diagnostics(
    nitrite_mg_l: f64,
    chloride_mg_l: f64,
    chloride_protection_factor: f64,
) -> NitriteStressDiagnostics {
    let effective_hazard_mg_l =
        compute_effective_nitrite_hazard(nitrite_mg_l, chloride_mg_l, chloride_protection_factor);
    NitriteStressDiagnostics {
        nitrite_mg_l: nitrite_mg_l.max(0.0),
        chloride_mg_l: chloride_mg_l.max(0.0),
        effective_hazard_mg_l,
        hourly_stress_increment: compute_hourly_nitrite_stress_increment(effective_hazard_mg_l),
    }
}

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
    let chloride_mg_l = chemistry.chloride_mg_per_l();
    let do_mg_l = chemistry.do_mg_per_l();
    let temp = state.water.temperature_c;

    // NH3 stress (threshold 0.02 mg/L, stronger than TAN alone)
    if nh3_mg_l > 0.02 {
        state.animal.hourly_nh3_stress_accum += (nh3_mg_l - 0.02) * 2.0 / 24.0;
    }

    // Nitrite stress with chloride protection (threshold 0.5 mg/L effective hazard)
    let nitrite_diagnostics = compute_nitrite_stress_diagnostics(
        nitrite_mg_l,
        chloride_mg_l,
        state.shrimp_params.chloride_protection_factor,
    );
    if nitrite_diagnostics.hourly_stress_increment > 0.0 {
        state.animal.hourly_nitrite_stress_accum += nitrite_diagnostics.hourly_stress_increment;
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
/// molt cycle -> reproductive readiness -> spawning -> egg dropping ->
/// egg development/hatching -> juvenile recruitment (two-stage) ->
/// mortality -> events.
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
    egg_dropping(state);
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
    let params = &state.shrimp_params;

    let tracker = &mut state.stability_tracker;

    let temp_swing = (state.water.temperature_c - tracker.prev_temp_c).abs();
    let ph_swing = (state.water.ph - tracker.prev_ph).abs();
    let gh_swing = (gh_d - tracker.prev_gh_d).abs();
    let do_swing = (do_mg_l - tracker.prev_do_mg_l).abs();
    tracker.last_temp_swing_c = temp_swing;

    // Weighted instability normalised to 0..1
    let raw_instability = (temp_swing / 3.0_f64.max(0.01)
        + ph_swing / params.instability_ph_swing.max(0.01)
        + gh_swing / params.instability_gh_swing_d.max(0.01)
        + do_swing / params.instability_do_swing_mg_l.max(0.01))
    .clamp(0.0, 1.0);

    let rise_smoothing = params.instability_rise_smoothing.clamp(0.0, 1.0);
    let decay_smoothing = params.instability_decay_smoothing.clamp(0.0, 1.0);

    // Rises quickly, decays slowly
    if raw_instability > tracker.instability_index {
        tracker.instability_index +=
            rise_smoothing * (raw_instability - tracker.instability_index);
    } else {
        tracker.instability_index +=
            decay_smoothing * (raw_instability - tracker.instability_index);
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
    let chloride_mg_l = chemistry.chloride_mg_per_l();
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
    let nitrite_diagnostics = compute_nitrite_stress_diagnostics(
        nitrite_mg_l,
        chloride_mg_l,
        state.shrimp_params.chloride_protection_factor,
    );
    let nitrite_factor = (1.0 - nitrite_diagnostics.effective_hazard_mg_l * 0.5).clamp(0.0, 1.0);

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
    let chemistry = state.concentrations();
    let gh_d = chemistry.gh_d();
    let ca_mg_per_l = chemistry.calcium_mg_per_l();
    let mg_mg_per_l = chemistry.magnesium_mg_per_l();
    let params = &state.shrimp_params;
    let gh_min_d = params.gh_min_d.max(0.01);
    let ca_min_mg_per_l = params.ca_min_mg_per_l.max(0.01);
    let mg_min_mg_per_l = params.mg_min_mg_per_l.max(0.01);

    let gh_stress = if gh_d < params.gh_min_d {
        (params.gh_min_d - gh_d) / gh_min_d
    } else {
        0.0
    };
    let ca_stress = if params.ca_min_mg_per_l > 0.0 && ca_mg_per_l < params.ca_min_mg_per_l {
        (params.ca_min_mg_per_l - ca_mg_per_l) / ca_min_mg_per_l
    } else {
        0.0
    };
    let mg_stress = if params.mg_min_mg_per_l > 0.0 && mg_mg_per_l < params.mg_min_mg_per_l {
        (params.mg_min_mg_per_l - mg_mg_per_l) / mg_min_mg_per_l
    } else {
        0.0
    };
    let mineral_stress = (0.5 * gh_stress + 0.3 * ca_stress + 0.2 * mg_stress).clamp(0.0, 1.0);

    let instability_stress = state.stability_tracker.instability_index;
    let condition_stress = (0.5 - state.animal.population_condition_index()).max(0.0);
    let thermal_stress = if state.water.temperature_c > params.optimal_temp_max_c {
        ((state.water.temperature_c - params.optimal_temp_max_c) / 8.0).clamp(0.0, 0.5)
    } else {
        0.0
    };

    let hourly_pressure =
        state.animal.hourly_heat_stress_accum + state.animal.hourly_instability_stress_accum;

    let stress_pressure = (mineral_stress * 0.3
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

/// Explicit molt cycle: per-stage readiness, chemistry-aware success/failure
/// resolution, and failed_molt_accum penalty tracking.
fn molt_cycle(state: &mut TankState) {
    if state.animal.total_count() == 0 {
        return;
    }

    if state.animal.adult.count == 0 {
        state.animal.adult.molt_timer_days = 0.0;
    }
    if state.animal.sub_adult.count == 0 {
        state.animal.sub_adult.molt_timer_days = 0.0;
    }
    if state.animal.juvenile.count == 0 {
        state.animal.juvenile.molt_timer_days = 0.0;
    }

    let params = state.shrimp_params.clone();
    let chemistry = state.concentrations();
    let gh_d = chemistry.gh_d();
    let ca_mg_per_l = chemistry.calcium_mg_per_l();
    let mg_mg_per_l = chemistry.magnesium_mg_per_l();
    let mineral_factor = molt_mineral_modifier(gh_d, ca_mg_per_l, mg_mg_per_l, &params);
    let critical_gh_deficit = (gh_d / params.gh_min_d.max(0.01)) < params.critical_molt_gh_ratio;
    let raw_temp_factor = temp_condition_factor(state.water.temperature_c, &params);
    let temp_factor = raw_temp_factor.max(0.25);
    let thermal_factor = raw_temp_factor;
    let instability_factor = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);

    if state.animal.juvenile.count > 0 {
        state.animal.juvenile.molt_timer_days += 1.0;
    }
    if state.animal.sub_adult.count > 0 {
        state.animal.sub_adult.molt_timer_days += 1.0;
    }
    if state.animal.adult.count > 0 {
        state.animal.adult.molt_timer_days += 1.0;
    }

    let stage_state = [
        (
            0usize,
            state.animal.juvenile.count,
            state.animal.juvenile.condition_index,
            state.animal.juvenile.reserve_g,
            state.animal.juvenile.molt_timer_days,
            params.juvenile_molt_interval_days,
            JUVENILE_SHRIMP_BIOMASS_G,
        ),
        (
            1usize,
            state.animal.sub_adult.count,
            state.animal.sub_adult.condition_index,
            state.animal.sub_adult.reserve_g,
            state.animal.sub_adult.molt_timer_days,
            params.sub_adult_molt_interval_days,
            SUB_ADULT_SHRIMP_BIOMASS_G,
        ),
        (
            2usize,
            state.animal.adult.count,
            state.animal.adult.condition_index,
            state.animal.adult.reserve_g,
            state.animal.adult.molt_timer_days,
            params.base_molt_interval_days,
            ADULT_SHRIMP_BIOMASS_G,
        ),
    ];

    let mut any_resolved = false;
    let mut failed_stage_count = 0u32;
    let mut successful_stage_count = 0u32;
    let mut max_readiness: f64 = 0.0;
    let mut failed_diagnostics = FailedMoltDiagnostics {
        min_condition_index: 1.0,
        poor_condition_involved: false,
        reserve_limited: false,
        min_reserve_per_shrimp_g: f64::INFINITY,
        reserve_target_g: 0.0,
    };

    for (
        stage_index,
        count,
        condition_index,
        reserve_g,
        timer_days,
        base_interval_days,
        biomass_g,
    ) in stage_state
    {
        if count == 0 {
            continue;
        }

        let effective_interval = (base_interval_days / temp_factor).max(1.0);
        let readiness = (timer_days / effective_interval).clamp(0.0, 1.0);
        max_readiness = max_readiness.max(readiness);

        if readiness >= 1.0 {
            any_resolved = true;

            let condition_breakdown =
                molt_condition_breakdown(condition_index, reserve_g, count, biomass_g, &params);
            let condition_factor = condition_breakdown.modifier;
            let success_score =
                (mineral_factor * condition_factor * instability_factor * thermal_factor)
                    .clamp(0.0, 1.0);

            if !critical_gh_deficit && success_score >= params.molt_success_threshold {
                successful_stage_count += 1;
            } else {
                failed_stage_count += 1;
                failed_diagnostics.poor_condition_involved |= condition_factor < 0.65;
                failed_diagnostics.min_condition_index =
                    failed_diagnostics.min_condition_index.min(condition_index);
                if condition_breakdown.reserve_factor < 1.0 - f64::EPSILON
                    && condition_breakdown.reserve_per_shrimp_g
                        < failed_diagnostics.min_reserve_per_shrimp_g
                {
                    failed_diagnostics.reserve_limited = true;
                    failed_diagnostics.min_reserve_per_shrimp_g =
                        condition_breakdown.reserve_per_shrimp_g;
                    failed_diagnostics.reserve_target_g = condition_breakdown.reserve_target_g;
                }
            }

            match stage_index {
                0 => state.animal.juvenile.molt_timer_days = 0.0,
                1 => state.animal.sub_adult.molt_timer_days = 0.0,
                _ => state.animal.adult.molt_timer_days = 0.0,
            }
        }
    }

    state.animal.molt_readiness = max_readiness;

    if any_resolved {
        if failed_stage_count > 0 {
            state.animal.last_molt_success = false;
            state.animal.failed_molt_accum =
                (state.animal.failed_molt_accum + 0.3 * f64::from(failed_stage_count)).min(1.0);
        } else {
            state.animal.last_molt_success = true;
            state.animal.failed_molt_accum = (state.animal.failed_molt_accum
                - 0.35 * f64::from(successful_stage_count))
            .max(0.0);
        }
    }

    state.animal.inter_molt_timer_days = state.animal.adult.molt_timer_days;

    if failed_stage_count > 0 {
        emit_molt_failure(
            state,
            failed_stage_count,
            gh_d,
            ca_mg_per_l,
            mg_mg_per_l,
            failed_diagnostics,
        );
    }

    // Derive molt_stress_index from failed_molt_accum and current factors.
    // This grounds the existing index in explicit state rather than independent
    // integration. The update_molt_stress call has already set a baseline; we
    // intentionally blend in failed_molt_accum here as a separate stress path
    // even though mortality also reads failed_molt_accum directly below.
    let accum_stress = state.animal.failed_molt_accum * 0.5;
    state.animal.molt_stress_index =
        (state.animal.molt_stress_index + accum_stress).clamp(0.0, 1.0);
}

fn update_reproductive_readiness(state: &mut TankState) {
    let target = reproductive_readiness_target(state);

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
    let volume_l = state.water_volume_l();
    let density_per_l = if volume_l > f64::EPSILON {
        f64::from(state.animal.total_count()) / volume_l
    } else {
        0.0
    };
    let density_factor = density_repro_factor(state.animal.total_count(), volume_l, params);
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

        let mut causes = vec![EventCause::RoutineAction];
        let density_detail = if density_factor < 1.0 - f64::EPSILON {
            causes.push(EventCause::HighDensity);
            format!(
                ", density {:.2}/L (factor {:.2})",
                density_per_l, density_factor
            )
        } else {
            String::new()
        };

        crate::systems::events::emit_once_per_day_pub(
            state,
            EventSeverity::Info,
            EventKind::ShrimpBerried,
            causes,
            format!(
                "{new_berried} female(s) became berried (readiness {:.2}{density_detail})",
                state.animal.reproductive_readiness_index
            ),
        );
    }
}

/// Egg dropping: berried females may lose their clutch when the
/// environment is unstable (temperature swings, chemistry swings).
///
/// The stability tracker is updated *before* the daily shrimp pipeline, so
/// `instability_index` already reflects today's chemistry/temperature swings.
/// `last_temp_swing_c` preserves the unsmoothed thermal shock so a first-day
/// 4 C jump can still drop eggs even if the smoothed instability index has not
/// risen above the broader instability threshold yet.
fn egg_dropping(state: &mut TankState) {
    if state.animal.egg_cohorts.is_empty() || state.animal.berried_females_count == 0 {
        return;
    }

    let params = &state.shrimp_params;
    let instability = state.stability_tracker.instability_index;
    let temp_swing_c = state.stability_tracker.last_temp_swing_c;

    let instability_pressure = egg_drop_instability_pressure(instability, params);
    let temp_swing_pressure = egg_drop_temp_swing_pressure(temp_swing_c, params);
    let drop_prob = instability_pressure
        .max(temp_swing_pressure)
        .clamp(0.0, 0.6);

    if drop_prob <= f64::EPSILON {
        return;
    }

    let mut total_dropped = 0u32;
    for cohort in &mut state.animal.egg_cohorts {
        let mut dropped = 0u32;
        for _ in 0..cohort.count {
            if state.rng.next_f64() < drop_prob {
                dropped += 1;
            }
        }
        dropped = dropped.min(cohort.count);
        cohort.count -= dropped;
        total_dropped += dropped;
    }

    // Remove empty cohorts
    state.animal.egg_cohorts.retain(|c| c.count > 0);

    if total_dropped > 0 {
        state.animal.berried_females_count = state
            .animal
            .berried_females_count
            .saturating_sub(total_dropped);

        let mut details = Vec::new();
        if instability_pressure > 0.0 {
            details.push(format!(
                "instability {instability:.2}>{:.2}",
                params.egg_drop_instability_threshold
            ));
        }
        if temp_swing_pressure > 0.0 {
            details.push(format!(
                "temp swing {temp_swing_c:.1}>{:.1} C/day",
                params.egg_drop_temp_swing_c
            ));
        }

        let causes = vec![EventCause::ChemistryInstability];

        crate::systems::events::emit_once_per_day_pub(
            state,
            EventSeverity::Warning,
            EventKind::EggDropping,
            causes,
            format!(
                "{total_dropped} berried female(s) dropped eggs \
                 ({}; p={drop_prob:.2})",
                details.join(", "),
            ),
        );
    }

    state.animal.sync_egg_progress_from_cohorts();
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
    let tan_mg_n_per_l = chemistry.tan_mg_n_per_l();
    let nitrite_mg_n_per_l = chemistry.nitrite_mg_n_per_l();
    let temp = state.water.temperature_c;
    let params = &state.shrimp_params;

    let f_condition = state.animal.adult.condition_index;
    let f_oxygen = (do_mg_l / 6.0).clamp(0.0, 1.0);
    let f_temp = temp_repro_factor(temp, params);
    let f_stability = (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0);
    let f_tan = tan_repro_factor(tan_mg_n_per_l, params);
    let f_no2 = no2_repro_factor(nitrite_mg_n_per_l, params);

    let gh_d = chemistry.gh_d();
    let f_mineral = gh_mineral_factor(gh_d, params);

    let hatch_rate = (params.hatch_success_base
        * f_condition
        * f_oxygen
        * f_temp
        * f_stability
        * f_mineral
        * f_tan
        * f_no2)
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
        if f_tan < 0.6 {
            causes.push(EventCause::HighAmmonia);
        }
        if f_no2 < 0.6 {
            causes.push(EventCause::HighNitrite);
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

    // Maturation should depend on stage-local condition, not on the shared
    // population-wide last_molt_success flag.
    let condition_gate = if state.animal.sub_adult.condition_index
        >= params.subadult_maturation_condition_threshold
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
    let mortality_probabilities = compute_mortality_probabilities(state);

    let mut adult_deaths = 0u32;
    for _ in 0..state.animal.adult.count {
        if state.rng.next_f64() < mortality_probabilities.adult {
            adult_deaths += 1;
        }
    }

    let mut sub_adult_deaths = 0u32;
    for _ in 0..state.animal.sub_adult.count {
        if state.rng.next_f64() < mortality_probabilities.sub_adult {
            sub_adult_deaths += 1;
        }
    }

    let mut juv_deaths = 0u32;
    for _ in 0..state.animal.juvenile.count {
        if state.rng.next_f64() < mortality_probabilities.juvenile {
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

fn compute_mortality_probabilities(state: &TankState) -> MortalityProbabilities {
    let base_rate = state.process_params.shrimp_base_mortality_per_day;
    let stress_scale = state.process_params.shrimp_stress_mortality_scale;

    let pop_condition = state.animal.population_condition_index();
    let stress_total = state.animal.hourly_nh3_stress_accum
        + state.animal.hourly_nitrite_stress_accum
        + state.animal.hourly_low_do_stress_accum
        + state.animal.hourly_heat_stress_accum
        + (state.animal.molt_stress_index - 0.5).max(0.0)
        + (0.5 - pop_condition).max(0.0);

    // Keep a direct lethality channel from failed molts in addition to the
    // stress/readiness path above so repeated exoskeleton failures remain
    // explicitly more deadly than generic background stress alone.
    let molt_mortality =
        state.animal.failed_molt_accum * state.shrimp_params.failed_molt_mortality_scale;
    let adult = (base_rate + stress_total * stress_scale + molt_mortality).clamp(0.0, 0.5);
    let sub_adult = (base_rate
        + stress_total * stress_scale * state.shrimp_params.sub_adult_sensitivity
        + molt_mortality)
        .clamp(0.0, 0.5);
    let juvenile = (base_rate
        + stress_total * stress_scale * state.shrimp_params.juvenile_sensitivity
        + molt_mortality)
        .clamp(0.0, 0.5);

    MortalityProbabilities {
        adult,
        sub_adult,
        juvenile,
    }
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

fn emit_molt_failure(
    state: &mut TankState,
    failed_stage_count: u32,
    gh_d: f64,
    ca_mg_per_l: f64,
    mg_mg_per_l: f64,
    failed_diagnostics: FailedMoltDiagnostics,
) {
    let params = &state.shrimp_params;
    let mut causes = Vec::new();
    let mut details = Vec::new();

    if gh_d < params.gh_min_d {
        causes.push(EventCause::LowMinerals);
        details.push(format!("GH {:.1}<{:.1} dGH", gh_d, params.gh_min_d));
    } else if gh_d > params.gh_max_d {
        causes.push(EventCause::HighMinerals);
        details.push(format!("GH {:.1}>{:.1} dGH", gh_d, params.gh_max_d));
    }
    if ca_mg_per_l < params.ca_min_mg_per_l {
        if !causes.contains(&EventCause::LowMinerals) {
            causes.push(EventCause::LowMinerals);
        }
        details.push(format!(
            "Ca {:.1}<{:.1} mg/L",
            ca_mg_per_l, params.ca_min_mg_per_l
        ));
    }
    if mg_mg_per_l < params.mg_min_mg_per_l {
        if !causes.contains(&EventCause::LowMinerals) {
            causes.push(EventCause::LowMinerals);
        }
        details.push(format!(
            "Mg {:.1}<{:.1} mg/L",
            mg_mg_per_l, params.mg_min_mg_per_l
        ));
    }
    if state.stability_tracker.instability_index > 0.3 {
        causes.push(EventCause::ChemistryInstability);
        details.push(format!(
            "instability {:.2}",
            state.stability_tracker.instability_index
        ));
    }
    if let Some((cause, detail)) = temperature_penalty_detail(state.water.temperature_c, params) {
        causes.push(cause);
        details.push(detail);
    }
    if failed_diagnostics.reserve_limited {
        causes.push(EventCause::Starvation);
        details.push(format!(
            "reserve {:.4}<{:.4} g/shrimp",
            failed_diagnostics.min_reserve_per_shrimp_g, failed_diagnostics.reserve_target_g
        ));
    }
    if failed_diagnostics.poor_condition_involved {
        causes.push(EventCause::PoorCondition);
        details.push(format!(
            "condition {:.2}",
            failed_diagnostics.min_condition_index
        ));
    }
    if causes.is_empty() {
        causes.push(EventCause::PoorCondition);
        details.push(format!(
            "condition {:.2}",
            failed_diagnostics.min_condition_index
        ));
    }

    crate::systems::events::emit_once_per_day_pub(
        state,
        EventSeverity::Warning,
        EventKind::MoltFailure,
        causes,
        format!(
            "{failed_stage_count} stage(s) failed to molt: {}",
            details.join(", ")
        ),
    );
}

fn emit_molt_stress_warning(state: &mut TankState) {
    if state.animal.molt_stress_index <= 0.6 {
        return;
    }

    let mut causes = Vec::new();
    let chemistry = state.concentrations();
    let gh_d = chemistry.gh_d();
    let ca_mg_per_l = chemistry.calcium_mg_per_l();
    let mg_mg_per_l = chemistry.magnesium_mg_per_l();

    if gh_d < state.shrimp_params.gh_min_d
        || ca_mg_per_l < state.shrimp_params.ca_min_mg_per_l
        || mg_mg_per_l < state.shrimp_params.mg_min_mg_per_l
    {
        causes.push(EventCause::LowMinerals);
    } else if gh_d > state.shrimp_params.gh_max_d {
        causes.push(EventCause::HighMinerals);
    }
    if state.stability_tracker.instability_index > 0.3 {
        causes.push(EventCause::ChemistryInstability);
    }
    if let Some((cause, _)) =
        temperature_penalty_detail(state.water.temperature_c, &state.shrimp_params)
    {
        causes.push(cause);
    }
    if causes.is_empty() {
        causes.push(EventCause::PoorCondition);
    }

    let temp_summary = temperature_summary(state.water.temperature_c, &state.shrimp_params);

    crate::systems::events::emit_once_per_day_pub(
        state,
        EventSeverity::Warning,
        EventKind::MoltStressWarning,
        causes,
        format!(
            "Molt stress elevated: {:.2} ({}, GH {:.1} dGH, Ca {:.1} mg/L, Mg {:.1} mg/L)",
            state.animal.molt_stress_index, temp_summary, gh_d, ca_mg_per_l, mg_mg_per_l
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

fn reproductive_readiness_factors(state: &TankState) -> [(&'static str, f64); 7] {
    let chemistry = state.concentrations();
    let params = &state.shrimp_params;

    [
        (
            "temperature",
            temp_repro_factor(state.water.temperature_c, params),
        ),
        ("condition", state.animal.adult.condition_index),
        (
            "stability",
            (1.0 - state.stability_tracker.instability_index).clamp(0.0, 1.0),
        ),
        (
            "molt_stress",
            (1.0 - state.animal.molt_stress_index).clamp(0.0, 1.0),
        ),
        (
            "density",
            density_repro_factor(state.animal.total_count(), chemistry.volume_l(), params),
        ),
        ("tan", tan_repro_factor(chemistry.tan_mg_n_per_l(), params)),
        (
            "nitrite",
            no2_repro_factor(chemistry.nitrite_mg_n_per_l(), params),
        ),
    ]
}

fn reproductive_readiness_target(state: &TankState) -> f64 {
    reproductive_readiness_factors(state)
        .iter()
        .map(|(_, factor)| *factor)
        .product::<f64>()
        .clamp(0.0, 1.0)
}

/// Keep snapshot diagnostics and runtime readiness updates on the same factor set.
pub(crate) fn dominant_repro_suppression_label(state: &TankState) -> &'static str {
    let (label, factor) = reproductive_readiness_factors(state)
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .copied()
        .expect("reproductive factor list is non-empty");

    if factor < 0.8 {
        label
    } else {
        "none"
    }
}

/// Public accessor for the temperature reproduction factor.
pub fn temp_repro_factor_pub(temp: f64, params: &ShrimpRuntimeParams) -> f64 {
    temp_repro_factor(temp, params)
}

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

/// Density-dependent per-capita reproduction suppression.
/// Returns 1.0 below the threshold, then declines hyperbolically toward the
/// half-suppression point.
fn density_repro_factor(total_count: u32, volume_l: f64, params: &ShrimpRuntimeParams) -> f64 {
    if volume_l <= f64::EPSILON || total_count == 0 {
        return 1.0;
    }
    let density = f64::from(total_count) / volume_l;
    if density <= params.density_repro_threshold_per_l {
        1.0
    } else {
        let excess = density - params.density_repro_threshold_per_l;
        let half = (params.density_repro_half_suppression_per_l
            - params.density_repro_threshold_per_l)
            .max(0.01);
        (1.0 / (1.0 + excess / half)).clamp(0.05, 1.0)
    }
}

fn egg_drop_instability_pressure(instability: f64, params: &ShrimpRuntimeParams) -> f64 {
    if instability > params.egg_drop_instability_threshold {
        ((instability - params.egg_drop_instability_threshold)
            / (1.0 - params.egg_drop_instability_threshold).max(0.01))
        .clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn egg_drop_temp_swing_pressure(temp_swing_c: f64, params: &ShrimpRuntimeParams) -> f64 {
    if temp_swing_c > params.egg_drop_temp_swing_c {
        ((temp_swing_c - params.egg_drop_temp_swing_c) / params.egg_drop_temp_swing_c.max(0.01))
            .clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// TAN-dependent reproduction suppression.
/// Returns 1.0 below the threshold, then linearly declines.
fn tan_repro_factor(tan_mg_n_per_l: f64, params: &ShrimpRuntimeParams) -> f64 {
    if tan_mg_n_per_l <= params.tan_repro_threshold_mg_n_per_l {
        1.0
    } else {
        let excess = tan_mg_n_per_l - params.tan_repro_threshold_mg_n_per_l;
        (1.0 - excess / (params.tan_repro_threshold_mg_n_per_l.max(0.01) * 2.0)).clamp(0.05, 1.0)
    }
}

/// NO2-dependent reproduction suppression.
/// Returns 1.0 below the threshold, then linearly declines.
fn no2_repro_factor(no2_mg_n_per_l: f64, params: &ShrimpRuntimeParams) -> f64 {
    if no2_mg_n_per_l <= params.no2_repro_threshold_mg_n_per_l {
        1.0
    } else {
        let excess = no2_mg_n_per_l - params.no2_repro_threshold_mg_n_per_l;
        (1.0 - excess / (params.no2_repro_threshold_mg_n_per_l.max(0.01) * 2.0)).clamp(0.05, 1.0)
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

fn temperature_penalty_detail(
    temp_c: f64,
    params: &ShrimpRuntimeParams,
) -> Option<(EventCause, String)> {
    if temp_c < params.optimal_temp_min_c {
        Some((
            EventCause::LowTemperature,
            format!("temp {:.1}<{:.1} C", temp_c, params.optimal_temp_min_c),
        ))
    } else if temp_c > params.optimal_temp_max_c {
        Some((
            EventCause::HighTemperature,
            format!("temp {:.1}>{:.1} C", temp_c, params.optimal_temp_max_c),
        ))
    } else {
        None
    }
}

fn temperature_summary(temp_c: f64, params: &ShrimpRuntimeParams) -> String {
    if temp_c < params.optimal_temp_min_c {
        format!(
            "temp {:.1} C below {:.1}-{:.1} C optimal",
            temp_c, params.optimal_temp_min_c, params.optimal_temp_max_c
        )
    } else if temp_c > params.optimal_temp_max_c {
        format!(
            "temp {:.1} C above {:.1}-{:.1} C optimal",
            temp_c, params.optimal_temp_min_c, params.optimal_temp_max_c
        )
    } else {
        format!("temp {:.1} C", temp_c)
    }
}

pub fn molt_mineral_modifier(
    gh_d: f64,
    ca_mg_per_l: f64,
    mg_mg_per_l: f64,
    params: &ShrimpRuntimeParams,
) -> f64 {
    let gh_min_d = params.gh_min_d.max(0.01);
    let ca_min_mg_per_l = params.ca_min_mg_per_l.max(0.01);
    let mg_min_mg_per_l = params.mg_min_mg_per_l.max(0.01);
    let gh_factor = if gh_d >= params.gh_min_d && gh_d <= params.gh_max_d {
        1.0
    } else if gh_d < params.gh_min_d {
        let gh_ratio = (gh_d / gh_min_d).clamp(0.0, 1.0);
        gh_ratio * gh_ratio
    } else {
        (1.0 - (gh_d - params.gh_max_d) / 10.0).clamp(0.3, 1.0)
    };
    let ca_factor = if params.ca_min_mg_per_l <= 0.0 {
        1.0
    } else {
        (ca_mg_per_l / ca_min_mg_per_l).clamp(0.3, 1.0)
    };
    let mg_factor = if params.mg_min_mg_per_l <= 0.0 {
        1.0
    } else {
        (mg_mg_per_l / mg_min_mg_per_l).clamp(0.3, 1.0)
    };

    (gh_factor * ca_factor.sqrt() * mg_factor.sqrt()).clamp(0.0, 1.0)
}

fn molt_condition_breakdown(
    condition_index: f64,
    reserve_g: f64,
    count: u32,
    wet_biomass_g: f64,
    params: &ShrimpRuntimeParams,
) -> MoltConditionBreakdown {
    if count == 0 {
        return MoltConditionBreakdown {
            modifier: 1.0,
            reserve_factor: 1.0,
            reserve_per_shrimp_g: 0.0,
            reserve_target_g: 0.0,
        };
    }

    let reserve_per_shrimp_g = reserve_g / f64::from(count);
    let reserve_target_g =
        wet_biomass_g * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G * params.molt_reserve_fraction;
    let reserve_factor = if reserve_target_g > f64::EPSILON {
        (reserve_per_shrimp_g / reserve_target_g).clamp(params.molt_reserve_factor_floor, 1.0)
    } else {
        1.0
    };

    MoltConditionBreakdown {
        modifier: (params.molt_condition_weight * condition_index.clamp(0.0, 1.0)
            + params.molt_reserve_weight * reserve_factor)
            .clamp(0.0, 1.0),
        reserve_factor,
        reserve_per_shrimp_g,
        reserve_target_g,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compute_effective_nitrite_hazard, compute_mortality_probabilities, refresh_carbonate_state,
        route_consumed_food, shrimp_feeding, shrimp_grazing_access_factor,
        shrimp_target_food_route_g, step_daily_shrimp, step_hourly_shrimp_stress, update_condition,
        MG_N_PER_MEQ_AMMONIA,
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

    // ── Chloride protection tests ──────────────────────────────────────────

    #[test]
    fn test_chloride_reduces_nitrite_hazard() {
        let cpf = 0.5;
        let no2 = 1.0;

        let hazard_no_cl = compute_effective_nitrite_hazard(no2, 0.0, cpf);
        assert_close(hazard_no_cl, 1.0, 1e-12);

        let hazard_30_cl = compute_effective_nitrite_hazard(no2, 30.0, cpf);
        assert!(
            hazard_30_cl < hazard_no_cl,
            "30 mg/L Cl should reduce hazard: {hazard_30_cl} >= {hazard_no_cl}"
        );

        // Verify monotonic decrease with increasing chloride
        let mut prev_hazard = hazard_no_cl;
        for cl in [5.0, 10.0, 20.0, 30.0, 50.0, 100.0] {
            let h = compute_effective_nitrite_hazard(no2, cl, cpf);
            assert!(
                h < prev_hazard,
                "hazard should decrease monotonically: Cl={cl}, h={h} >= prev={prev_hazard}"
            );
            prev_hazard = h;
        }
    }

    #[test]
    fn test_chloride_protection_ratio() {
        let cpf = 0.5;
        let no2 = 1.0;
        let unprotected = compute_effective_nitrite_hazard(no2, 0.0, cpf);

        // At Cl:NO2 > 10:1, effective hazard < 20% of unprotected
        let cl_10_ratio = compute_effective_nitrite_hazard(no2, 10.0 * no2, cpf);
        assert!(
            cl_10_ratio < 0.20 * unprotected,
            "Cl:NO2=10:1 should give <20% hazard: {cl_10_ratio} vs 20% of {unprotected}"
        );

        // Also check at higher ratios
        let cl_20_ratio = compute_effective_nitrite_hazard(no2, 20.0 * no2, cpf);
        assert!(
            cl_20_ratio < cl_10_ratio,
            "higher ratio should give less hazard"
        );
    }

    #[test]
    fn test_zero_nitrite_zero_hazard() {
        let cpf = 0.5;
        // Zero nitrite regardless of chloride level
        assert_close(compute_effective_nitrite_hazard(0.0, 0.0, cpf), 0.0, 1e-15);
        assert_close(compute_effective_nitrite_hazard(0.0, 50.0, cpf), 0.0, 1e-15);
        assert_close(
            compute_effective_nitrite_hazard(0.0, 200.0, cpf),
            0.0,
            1e-15,
        );
    }

    #[test]
    fn test_protection_factor_is_species_parameter() {
        // Verify that different chloride_protection_factor values produce
        // different effective hazards, proving it's a configurable parameter.
        let no2 = 2.0;
        let cl = 20.0;
        let h_low_cpf = compute_effective_nitrite_hazard(no2, cl, 0.2);
        let h_high_cpf = compute_effective_nitrite_hazard(no2, cl, 0.8);
        assert!(
            h_high_cpf < h_low_cpf,
            "higher cpf should give more protection: cpf=0.8 -> {h_high_cpf}, cpf=0.2 -> {h_low_cpf}"
        );

        // Also verify the parameter is a named field in ShrimpRuntimeParams
        let params = crate::types::ShrimpRuntimeParams::default();
        assert!(
            params.chloride_protection_factor > 0.0,
            "chloride_protection_factor should be a positive species parameter"
        );
    }

    #[test]
    fn test_stress_accounting_integrates_with_existing() {
        // Chloride-modified nitrite stress feeds into hourly_nitrite_stress_accum,
        // not a parallel stress system.
        let mut state = TankState::new(SimSeed(20_001));
        state.geometry.length_cm = 40.0;
        state.geometry.width_cm = 30.0;
        state.geometry.height_cm = 35.0;
        state.geometry.fill_height_cm = 30.0;
        state.water = WaterState::default_for_volume_l(state.water_volume_l());
        state.water.temperature_c = 24.0;
        state.environment.ambient_temp_c = 24.0;
        state.animal.adult.count = 10;

        let volume_l = state.water_volume_l();
        state.water.dissolved_oxygen_mg_total = 8.0 * volume_l;

        // High nitrite, zero chloride — should accumulate stress
        state.water.nitrite_mg_n_total = 3.0 * volume_l;
        state.water.chloride_mg_total = 0.0;

        state.reseed_stability_tracker();
        step_hourly_shrimp_stress(&mut state);

        let stress_no_cl = state.animal.hourly_nitrite_stress_accum;
        assert!(
            stress_no_cl > 0.0,
            "3 mg/L nitrite with no chloride should accumulate stress"
        );

        // Same nitrite, high chloride — should accumulate less stress
        let mut state_cl = state.clone();
        state_cl.animal.hourly_nitrite_stress_accum = 0.0;
        state_cl.water.chloride_mg_total = 100.0 * volume_l;

        step_hourly_shrimp_stress(&mut state_cl);

        let stress_with_cl = state_cl.animal.hourly_nitrite_stress_accum;
        assert!(
            stress_with_cl < stress_no_cl,
            "chloride should reduce nitrite stress: with_cl={stress_with_cl}, no_cl={stress_no_cl}"
        );

        // Stress goes through the same hourly_nitrite_stress_accum field
        // (not a separate accumulator), confirming integration with existing path.
    }

    #[test]
    fn test_high_nitrite_low_chloride_vs_high_chloride() {
        // High nitrite + low chloride → more nitrite stress than high nitrite + high chloride
        let mut low_cl = TankState::new(SimSeed(20_002));
        low_cl.geometry.length_cm = 40.0;
        low_cl.geometry.width_cm = 30.0;
        low_cl.geometry.height_cm = 35.0;
        low_cl.geometry.fill_height_cm = 30.0;
        low_cl.water = WaterState::default_for_volume_l(low_cl.water_volume_l());
        low_cl.water.temperature_c = 24.0;
        low_cl.environment.ambient_temp_c = 24.0;
        low_cl.animal.adult.count = 10;

        let volume_l = low_cl.water_volume_l();
        low_cl.water.dissolved_oxygen_mg_total = 8.0 * volume_l;
        low_cl.water.nitrite_mg_n_total = 5.0 * volume_l; // 5 mg/L — severe
        low_cl.water.chloride_mg_total = 0.0; // no protection

        let mut high_cl = low_cl.clone();
        high_cl.water.chloride_mg_total = 100.0 * volume_l; // 100 mg/L Cl

        low_cl.reseed_stability_tracker();
        high_cl.reseed_stability_tracker();

        // Accumulate stress over 24 hours
        for _ in 0..24 {
            step_hourly_shrimp_stress(&mut low_cl);
            step_hourly_shrimp_stress(&mut high_cl);
        }

        assert!(
            low_cl.animal.hourly_nitrite_stress_accum > high_cl.animal.hourly_nitrite_stress_accum,
            "low chloride should produce more nitrite stress: low_cl={}, high_cl={}",
            low_cl.animal.hourly_nitrite_stress_accum,
            high_cl.animal.hourly_nitrite_stress_accum
        );

        // The low-chloride scenario should produce substantially more stress
        assert!(
            low_cl.animal.hourly_nitrite_stress_accum
                > 3.0 * high_cl.animal.hourly_nitrite_stress_accum,
            "at 100 mg/L Cl and cpf=0.5, protection should be substantial"
        );

        let low_cl_mortality = compute_mortality_probabilities(&low_cl).adult;
        let high_cl_mortality = compute_mortality_probabilities(&high_cl).adult;
        assert!(
            low_cl_mortality > high_cl_mortality,
            "low chloride should produce higher adult mortality probability: \
             low_cl={low_cl_mortality}, high_cl={high_cl_mortality}"
        );
    }
}
