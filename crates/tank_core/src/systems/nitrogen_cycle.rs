use crate::types::{concentration_from_total, legacy_total_param_to_mg_per_l, TankState};

const FEED_P_TO_N_MASS_RATIO: f64 = 0.10;
const ADULT_SHRIMP_BIOMASS_G: f64 = 0.12;
const JUVENILE_SHRIMP_BIOMASS_G: f64 = 0.05;

/// Result of one hourly nitrogen cycle step, carrying coupling values
/// that downstream systems (DO, chemistry) need.
#[derive(Debug, Clone, Copy, Default)]
pub struct NitrogenCycleOutput {
    /// Total mg N oxidized to nitrate this tick (AOB + NOB + comammox pathway).
    pub total_mg_n_nitrified: f64,
}

/// Monod-style environmental factors shared across guilds.
struct EnvFactors {
    f_temp: f64,
    f_ph: f64,
    maturity_factor: f64,
    /// Dirty-filter penalty: 1.0 = clean, lower = clogged/dirty.
    dirty_filter_factor: f64,
    filter_enabled_factor: f64,
    flow_factor: f64,
}

fn safe_rate(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

/// Runs the full nitrogen-cycle phase for one hourly tick.
///
/// Order: feed leaching -> detritus breakdown/mineralization -> nitrification + guild growth/decay.
///
/// Returns coupling values for downstream DO and alkalinity systems.
pub fn step_nitrogen_cycle(state: &mut TankState) -> NitrogenCycleOutput {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return NitrogenCycleOutput::default();
    }

    let pp = &state.process_params;

    // ---- 1. Feed leaching: particulate -> fine detritus ----
    let trapping_factor =
        1.0 - (0.5 * state.avg_substrate_index(|layer| layer.detritus_trapping_index));
    let leach_rate = safe_rate(pp.feed_leach_rate_per_hour * trapping_factor);
    let leached = (state.detritus.particulate_organics_g_total * leach_rate)
        .min(state.detritus.particulate_organics_g_total);
    state.detritus.particulate_organics_g_total -= leached;
    state.detritus.fine_detritus_g_total += leached;

    // ---- 2. Fine detritus dissolution: fine detritus -> dissolved organic pools ----
    let diss_rate = safe_rate(pp.fine_detritus_dissolution_rate_per_hour);
    let dissolved = (state.detritus.fine_detritus_g_total * diss_rate)
        .min(state.detritus.fine_detritus_g_total);
    state.detritus.fine_detritus_g_total -= dissolved;
    // dissolved mass (g) -> mg for dissolved pools: 1 g = 1000 mg
    // Split into DOC and DON using N:C ratio
    let n_to_c = safe_rate(pp.feed_n_to_c_ratio);
    let doc_mg = dissolved * 1000.0 / (1.0 + n_to_c); // carbon fraction
    let don_mg = doc_mg * n_to_c; // nitrogen fraction
    let phosphate_mg = don_mg * FEED_P_TO_N_MASS_RATIO;
    state.water.dissolved_organic_carbon_mg_c_total += doc_mg;
    state.water.dissolved_organic_nitrogen_mg_n_total += don_mg;
    // Feed introduces dissolved phosphorus alongside organic C/N, supporting algae pressure.
    state.water.phosphate_mg_p_total += phosphate_mg;
    // Track flow through dissolved_feed_residue (bookkeeping, decremented by mineralization)
    state.detritus.dissolved_feed_residue_g_total += dissolved;

    // ---- 3. Decomposer mineralization: DOC/DON -> TAN ----
    let decomposer_biomass = state.microbe.decomposer_biomass_g;
    let doc_total = state.water.dissolved_organic_carbon_mg_c_total;
    let don_total = state.water.dissolved_organic_nitrogen_mg_n_total;
    let doc_mg_c_per_l = state.water.doc_mg_c_per_l(volume_l);
    let do_mg_per_l = state.water.do_mg_per_l(volume_l);

    // Environmental factors for decomposers
    let temp = state.water.temperature_c;
    let f_temp_decomp = temperature_factor(temp);
    let decomposer_k_do_mg_per_l = legacy_total_param_to_mg_per_l(pp.decomposer_k_do_mg).max(0.01);
    let decomposer_k_doc_mg_c_per_l =
        legacy_total_param_to_mg_per_l(pp.decomposer_k_doc_mg).max(0.01);
    let f_do_decomp = monod_factor(do_mg_per_l, decomposer_k_do_mg_per_l);

    // Microfauna modestly improve mineralization efficiency
    let microfauna_boost =
        1.0 + pp.microfauna_mineralization_boost * state.microfauna.population_index;
    let decomp_vmax = safe_rate(pp.decomposer_vmax_per_hour) * microfauna_boost;
    let monod_doc = monod_factor(doc_mg_c_per_l, decomposer_k_doc_mg_c_per_l);

    let potential_doc_consumed_mg = decomp_vmax * decomposer_biomass * 1000.0 // g->mg conversion for biomass effect
        * f_temp_decomp * f_do_decomp * monod_doc;
    let potential_doc_consumed_mg = safe_rate(potential_doc_consumed_mg);

    // Clamp: cannot consume more DOC than exists
    let doc_consumed_mg = potential_doc_consumed_mg.min(doc_total);
    // Proportional DON consumed
    let don_consumed_mg = if doc_total > f64::EPSILON {
        doc_consumed_mg * (don_total / doc_total)
    } else {
        0.0
    };
    let don_consumed_mg = don_consumed_mg.min(don_total);

    state.water.dissolved_organic_carbon_mg_c_total -= doc_consumed_mg;
    state.water.dissolved_organic_nitrogen_mg_n_total -= don_consumed_mg;
    // Mineralized DON becomes TAN
    state.water.ammonia_total_mg_n_total += don_consumed_mg;
    // Track residue pool depletion
    let residue_consumed_g = doc_consumed_mg / 1000.0 * (1.0 + n_to_c);
    state.detritus.dissolved_feed_residue_g_total =
        (state.detritus.dissolved_feed_residue_g_total - residue_consumed_g).max(0.0);

    // Decomposer growth/decay
    let decomp_growth = safe_rate(pp.decomposer_growth_yield) * doc_consumed_mg / 1000.0; // mg->g
    let decomp_decay = safe_rate(pp.decomposer_decay_rate_per_hour) * decomposer_biomass;
    state.microbe.decomposer_biomass_g =
        (state.microbe.decomposer_biomass_g + decomp_growth - decomp_decay).max(0.0);

    // ---- 4. Nitrification: TAN -> nitrite -> nitrate (+ comammox TAN -> nitrate) ----
    //
    // Shared DO and alkalinity budgets enforce that combined nitrification
    // can never consume more O2 or alkalinity than available this tick.
    let env = compute_env_factors(state);
    let combined_env = env.maturity_factor * env.dirty_filter_factor * env.filter_enabled_factor;

    let mut do_budget = state.water.dissolved_oxygen_mg_total;
    let alk_per_mg_n = safe_rate(pp.alkalinity_meq_per_mg_n_nitrified);
    let mut alk_budget = state.water.alkalinity_meq_total;

    // Stoichiometric O2 costs per mg N for each guild step
    let o2_for_aob = 3.43_f64; // TAN -> nitrite
    let o2_for_comammox = pp.o2_per_mg_n_nitrified; // TAN -> nitrate (4.57)
    let o2_for_nob = 1.14_f64; // nitrite -> nitrate
    let aob_k_tan_mg_n_per_l = legacy_total_param_to_mg_per_l(pp.aob_k_tan_mg).max(0.01);
    let aob_k_do_mg_per_l = legacy_total_param_to_mg_per_l(pp.aob_k_do_mg).max(0.01);
    let comammox_k_tan_mg_n_per_l = legacy_total_param_to_mg_per_l(pp.comammox_k_tan_mg).max(0.01);
    let comammox_k_do_mg_per_l = legacy_total_param_to_mg_per_l(pp.comammox_k_do_mg).max(0.01);
    let nob_k_nitrite_mg_n_per_l = legacy_total_param_to_mg_per_l(pp.nob_k_nitrite_mg).max(0.01);
    let nob_k_do_mg_per_l = legacy_total_param_to_mg_per_l(pp.nob_k_do_mg).max(0.01);

    // 4a. AOB: TAN -> nitrite
    let aob_env_factor = combined_env
        * env.f_temp
        * env.f_ph
        * monod_factor(
            concentration_from_total(do_budget, volume_l),
            aob_k_do_mg_per_l,
        );
    let aob_potential = monod_rate(
        safe_rate(pp.aob_vmax_mg_n_per_g_per_hour) * env.flow_factor,
        state.microbe.ammonia_oxidizer_biomass_g,
        aob_env_factor,
        state.water.tan_mg_n_per_l(volume_l),
        aob_k_tan_mg_n_per_l,
    );
    let aob_rate = safe_rate(aob_potential)
        .min(state.water.ammonia_total_mg_n_total)
        .min(if o2_for_aob > 0.0 {
            do_budget / o2_for_aob
        } else {
            f64::MAX
        })
        .min(if alk_per_mg_n > 0.0 {
            alk_budget / alk_per_mg_n
        } else {
            f64::MAX
        });

    // Debit shared budgets for AOB
    let aob_o2_cost = aob_rate * o2_for_aob;
    let aob_alk_cost = aob_rate * alk_per_mg_n;
    do_budget = (do_budget - aob_o2_cost).max(0.0);
    alk_budget = (alk_budget - aob_alk_cost).max(0.0);

    // 4b. Comammox: TAN -> nitrate directly (lower vmax)
    let comammox_vmax =
        safe_rate(pp.aob_vmax_mg_n_per_g_per_hour * pp.comammox_vmax_fraction) * env.flow_factor;
    let tan_after_aob = (state.water.ammonia_total_mg_n_total - aob_rate).max(0.0);
    let tan_after_aob_mg_n_per_l = concentration_from_total(tan_after_aob, volume_l);
    let comammox_env_factor = combined_env
        * env.f_temp
        * env.f_ph
        * monod_factor(
            concentration_from_total(do_budget, volume_l),
            comammox_k_do_mg_per_l,
        );
    let comammox_potential = monod_rate(
        comammox_vmax,
        state.microbe.comammox_biomass_g,
        comammox_env_factor,
        tan_after_aob_mg_n_per_l,
        comammox_k_tan_mg_n_per_l,
    );
    let comammox_rate = safe_rate(comammox_potential)
        .min(tan_after_aob)
        .min(if o2_for_comammox > 0.0 {
            do_budget / o2_for_comammox
        } else {
            f64::MAX
        })
        .min(if alk_per_mg_n > 0.0 {
            alk_budget / alk_per_mg_n
        } else {
            f64::MAX
        });

    // Debit shared budgets for comammox
    let comammox_o2_cost = comammox_rate * o2_for_comammox;
    do_budget = (do_budget - comammox_o2_cost).max(0.0);

    // Apply AOB + comammox TAN consumption
    let total_tan_consumed = aob_rate + comammox_rate;
    state.water.ammonia_total_mg_n_total =
        (state.water.ammonia_total_mg_n_total - total_tan_consumed).max(0.0);
    // AOB produces nitrite
    state.water.nitrite_mg_n_total += aob_rate;
    // Comammox produces nitrate directly
    state.water.nitrate_mg_n_total += comammox_rate;

    // Apply O2 costs so far (AOB + comammox) to state
    state.water.dissolved_oxygen_mg_total =
        (state.water.dissolved_oxygen_mg_total - aob_o2_cost - comammox_o2_cost).max(0.0);

    // 4c. NOB: nitrite -> nitrate (uses remaining DO/alk budget)
    let nob_env_factor = combined_env
        * env.f_temp
        * env.f_ph
        * monod_factor(
            concentration_from_total(do_budget, volume_l),
            nob_k_do_mg_per_l,
        );
    let nob_potential = monod_rate(
        safe_rate(pp.nob_vmax_mg_n_per_g_per_hour) * env.flow_factor,
        state.microbe.nitrite_oxidizer_biomass_g,
        nob_env_factor,
        state.water.nitrite_mg_n_per_l(volume_l),
        nob_k_nitrite_mg_n_per_l,
    );
    // NOB (nitrite -> nitrate) does not consume additional alkalinity beyond
    // what AOB already consumed for the TAN -> nitrite step.  Only DO limits NOB.
    let nob_rate = safe_rate(nob_potential)
        .min(state.water.nitrite_mg_n_total)
        .min(if o2_for_nob > 0.0 {
            do_budget / o2_for_nob
        } else {
            f64::MAX
        });

    let nob_o2_cost = nob_rate * o2_for_nob;

    state.water.nitrite_mg_n_total = (state.water.nitrite_mg_n_total - nob_rate).max(0.0);
    state.water.nitrate_mg_n_total += nob_rate;
    state.water.dissolved_oxygen_mg_total =
        (state.water.dissolved_oxygen_mg_total - nob_o2_cost).max(0.0);

    // Total N fully nitrified to nitrate (for alkalinity coupling)
    // AOB only takes TAN -> nitrite, NOB takes nitrite -> nitrate, comammox takes TAN -> nitrate
    // Full pathway N: nob_rate (came from AOB path) + comammox_rate
    let total_mg_n_nitrified = nob_rate + comammox_rate;

    // ---- 5. Guild growth and decay ----
    // Logistic carrying capacity: biofilter surface area limits total nitrifier
    // biomass. Growth is suppressed as total biomass approaches the capacity.
    let capacity_g = 0.5;
    let total_nitrifier_g = state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let logistic_factor = (1.0 - total_nitrifier_g / capacity_g).clamp(0.0, 1.0);

    // AOB growth from TAN oxidized
    let aob_growth = safe_rate(pp.aob_growth_yield) * aob_rate * logistic_factor;
    let aob_decay =
        safe_rate(pp.aob_decay_rate_per_hour) * state.microbe.ammonia_oxidizer_biomass_g;
    state.microbe.ammonia_oxidizer_biomass_g =
        (state.microbe.ammonia_oxidizer_biomass_g + aob_growth - aob_decay).max(0.0);

    // NOB growth from nitrite oxidized
    let nob_growth = safe_rate(pp.nob_growth_yield) * nob_rate * logistic_factor;
    let nob_decay =
        safe_rate(pp.nob_decay_rate_per_hour) * state.microbe.nitrite_oxidizer_biomass_g;
    state.microbe.nitrite_oxidizer_biomass_g =
        (state.microbe.nitrite_oxidizer_biomass_g + nob_growth - nob_decay).max(0.0);

    // Comammox growth from TAN fully oxidized
    let comammox_growth = safe_rate(pp.comammox_growth_yield) * comammox_rate * logistic_factor;
    let comammox_decay =
        safe_rate(pp.comammox_decay_rate_per_hour) * state.microbe.comammox_biomass_g;
    state.microbe.comammox_biomass_g =
        (state.microbe.comammox_biomass_g + comammox_growth - comammox_decay).max(0.0);

    // ---- 6. Alkalinity consumption from nitrification ----
    // Only AOB and comammox consume alkalinity (TAN oxidation step).
    // NOB (nitrite -> nitrate) does not consume additional alkalinity.
    let total_alk_consumed = (aob_rate + comammox_rate) * alk_per_mg_n;
    state.water.alkalinity_meq_total =
        (state.water.alkalinity_meq_total - total_alk_consumed).max(0.0);
    // Deplete bicarbonate proportionally so TDS/conductivity stay consistent
    // with the alkalinity drop.  1 meq alkalinity ≈ 61 mg HCO₃⁻.
    let bicarb_consumed_mg = total_alk_consumed * 61.0;
    state.water.bicarbonate_mg_total =
        (state.water.bicarbonate_mg_total - bicarb_consumed_mg).max(0.0);

    // Nitrification is a chemoautotrophic process that produces some DIC fixation
    // but for simplicity we model it as a net DIC producer via mineralization pathway above.

    NitrogenCycleOutput {
        total_mg_n_nitrified,
    }
}

/// Temperature factor: peaks around 25-30°C, drops off at extremes.
fn temperature_factor(temp_c: f64) -> f64 {
    // Bell-shaped factor peaking at 28°C with a half-width of ~10°C
    let opt = 28.0;
    let sigma = 10.0;
    let diff = temp_c - opt;
    (-(diff * diff) / (2.0 * sigma * sigma)).exp()
}

/// pH factor: nitrifiers prefer 7-8, reduced at extremes.
fn ph_factor(ph: f64) -> f64 {
    let opt = 7.5;
    let sigma = 1.5;
    let diff = ph - opt;
    (-(diff * diff) / (2.0 * sigma * sigma)).exp()
}

fn compute_env_factors(state: &TankState) -> EnvFactors {
    let f_temp = temperature_factor(state.water.temperature_c);
    let f_ph = ph_factor(state.water.ph);

    // Maturity factor: immature filters slow nitrification
    let maturity_factor = state.filter_state.biofilter_maturity_index.clamp(0.05, 1.0);

    // Dirty-filter factor: clogged filters reduce nitrification efficiency.
    // clogging_index of 0 = clean (factor=1), clogging_index of 1 = fully clogged (factor=0.2)
    let dirty_filter_factor = (1.0 - 0.8 * state.filter_state.clogging_index).clamp(0.2, 1.0);
    let filter_enabled_factor = if state.hardware.filter.enabled {
        1.0
    } else {
        0.1
    };
    let volume_l = state.water_volume_l().max(f64::EPSILON);
    let flow_factor = if state.hardware.filter.enabled {
        (state.hardware.filter.flow_lph / volume_l).clamp(0.1, 1.0)
    } else {
        1.0
    };

    EnvFactors {
        f_temp,
        f_ph,
        maturity_factor,
        dirty_filter_factor,
        filter_enabled_factor,
        flow_factor,
    }
}

/// Monod-style rate: vmax * biomass * environmental_factor * S/(K+S)
fn monod_rate(
    vmax: f64,
    biomass_g: f64,
    environmental_factor: f64,
    substrate: f64,
    k_substrate: f64,
) -> f64 {
    let monod = monod_factor(substrate, k_substrate);
    safe_rate(vmax * biomass_g * environmental_factor * monod)
}

fn monod_factor(substrate: f64, k_substrate: f64) -> f64 {
    let substrate = safe_rate(substrate);
    let k_substrate = safe_rate(k_substrate).max(f64::MIN_POSITIVE);
    substrate / (substrate + k_substrate)
}

/// Daily biofilter maturity update. Called every 24 ticks.
/// Recomputes `filter_state.biofilter_maturity_index` from guild biomass.
pub fn update_daily_biofilter_maturity(state: &mut TankState) -> f64 {
    let prev = state.filter_state.biofilter_maturity_index;

    // Capacity reference: a "mature" biofilter might have ~0.5g total nitrifier biomass
    let capacity_g = 0.5;
    let total_nitrifier_g = state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let raw_maturity = (total_nitrifier_g / capacity_g).clamp(0.0, 1.0);

    // Smooth towards raw_maturity
    let alpha = 0.1;
    let new_maturity = (prev + alpha * (raw_maturity - prev)).clamp(0.0, 1.0);
    state.filter_state.biofilter_maturity_index = new_maturity;

    new_maturity - prev
}

pub fn update_daily_filter_clogging(state: &mut TankState) -> f64 {
    if !state.hardware.filter.enabled {
        return 0.0;
    }

    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return 0.0;
    }

    let fine_detritus_g_l = state.detritus.fine_detritus_g_total / volume_l;
    let particulate_detritus_g_l = state.detritus.particulate_organics_g_total / volume_l;
    let dissolved_residue_g_l = state.detritus.dissolved_feed_residue_g_total / volume_l;
    let detritus_pressure =
        (fine_detritus_g_l + (0.35 * particulate_detritus_g_l) + (0.25 * dissolved_residue_g_l))
            .clamp(0.0, 2.0);
    let shrimp_biomass_g = (f64::from(state.animal.adults_count) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(state.animal.juveniles_count) * JUVENILE_SHRIMP_BIOMASS_G);
    let bioload_pressure = (shrimp_biomass_g / volume_l).clamp(0.0, 1.0);

    let cleanliness_before = state.hardware.filter.cleanliness_index.clamp(0.0, 1.0);
    let cleanliness_decay =
        ((0.03 * detritus_pressure) + (0.004 * bioload_pressure)).clamp(0.0, 0.08);
    state.hardware.filter.cleanliness_index =
        (cleanliness_before - cleanliness_decay).clamp(0.0, 1.0);

    let clogging_delta = (cleanliness_decay * (0.4 + (0.6 * cleanliness_before))).clamp(0.0, 1.0);

    state.filter_state.clogging_index =
        (state.filter_state.clogging_index + clogging_delta).clamp(0.0, 1.0);

    clogging_delta
}
