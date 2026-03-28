use crate::types::{
    BudgetDelta, ElementBudget, TankState, WaterState, ADULT_SHRIMP_BIOMASS_G,
    JUVENILE_SHRIMP_BIOMASS_G,
};

// ---------------------------------------------------------------------------
// Carbonate equilibrium constants
// ---------------------------------------------------------------------------

/// Temperature-corrected first dissociation constant pKa1 (Harned & Davis, 1943).
///
/// Accurate across the 15-35°C freshwater aquarium target range.
fn pka1(temperature_c: f64) -> f64 {
    let t_k = temperature_c + 273.15;
    3404.71 / t_k + 0.032786 * t_k - 14.8435
}

/// Fixed second dissociation constant pKa2 (first-pass approximation).
/// CO3-- is a minor fraction across pH 5.5-8.5 so temperature correction
/// is deferred.
const PKA2: f64 = 10.33;

/// Storage and gameplay pH bounds. The solver reprojects species at these
/// bounds so cached carbonate outputs remain self-consistent.
pub const CARBONATE_PH_MIN: f64 = 5.5;
pub const CARBONATE_PH_MAX: f64 = 8.5;

const DIAGNOSTIC_PH_MIN: f64 = 4.0;
const DIAGNOSTIC_PH_MAX: f64 = 10.0;

/// Molar mass of HCO3- in mg/mmol, used to project solved bicarbonate
/// back to the cached `bicarbonate_mg_total` field.
const HCO3_MG_PER_MMOL: f64 = 61.0;

// ---------------------------------------------------------------------------
// Solver output
// ---------------------------------------------------------------------------

/// Output from the carbonate equilibrium solver.
///
/// All species concentrations are in mmol/L. pH is dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarbonateEquilibrium {
    pub ph: f64,
    pub co2_aq_mmol_per_l: f64,
    pub hco3_mmol_per_l: f64,
    pub co3_mmol_per_l: f64,
}

const NEUTRAL_FALLBACK: CarbonateEquilibrium = CarbonateEquilibrium {
    ph: 7.0,
    co2_aq_mmol_per_l: 0.0,
    hco3_mmol_per_l: 0.0,
    co3_mmol_per_l: 0.0,
};

fn clamp_carbonate_ph(ph: f64) -> f64 {
    if ph.is_finite() {
        ph.clamp(CARBONATE_PH_MIN, CARBONATE_PH_MAX)
    } else {
        7.0
    }
}

fn carbonate_species_for_ph(
    dic_mol_per_l: f64,
    ph: f64,
    ka1: f64,
    ka2: f64,
) -> (f64, f64, f64) {
    let h = 10.0_f64.powf(-ph);
    let denom = h * h + ka1 * h + ka1 * ka2;
    let co2 = dic_mol_per_l * h * h / denom;
    let hco3 = dic_mol_per_l * ka1 * h / denom;
    let co3 = dic_mol_per_l * ka1 * ka2 / denom;
    (co2 * 1000.0, hco3 * 1000.0, co3 * 1000.0)
}

fn acid_only_fallback_ph(dic_mol_per_l: f64, ka1: f64) -> f64 {
    let discriminant = ka1 * ka1 + 4.0 * ka1 * dic_mol_per_l.max(0.0);
    let h = (-ka1 + discriminant.sqrt()) / 2.0;

    if h.is_finite() && h > 0.0 {
        clamp_carbonate_ph(-h.log10())
    } else {
        7.0
    }
}

fn fallback_all_co2(dic_mol_per_l: f64, ka1: f64) -> CarbonateEquilibrium {
    CarbonateEquilibrium {
        ph: acid_only_fallback_ph(dic_mol_per_l, ka1),
        co2_aq_mmol_per_l: dic_mol_per_l * 1000.0,
        hco3_mmol_per_l: 0.0,
        co3_mmol_per_l: 0.0,
    }
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

/// Solves the freshwater carbonate equilibrium analytically.
///
/// Given total DIC, total alkalinity, temperature, and water volume, computes
/// pH and the three carbonate species concentrations using a closed-form
/// quadratic in \[H+\]. See `docs/carbonate_state_contract.md` for derivation.
///
/// The solver is deterministic, O(1), and uses no iteration.
pub fn solve_carbonate_equilibrium(
    dic_mg_c_total: f64,
    alkalinity_meq_total: f64,
    temperature_c: f64,
    volume_l: f64,
) -> CarbonateEquilibrium {
    if volume_l <= f64::EPSILON {
        return NEUTRAL_FALLBACK;
    }

    // Convert stored totals to molar concentrations.
    let dic = dic_mg_c_total / 12_000.0 / volume_l;
    let alk = alkalinity_meq_total / 1_000.0 / volume_l;

    if dic <= 1e-12 {
        return NEUTRAL_FALLBACK;
    }

    let ka1 = 10.0_f64.powf(-pka1(temperature_c));
    let ka2 = 10.0_f64.powf(-PKA2);

    // Negligible alkalinity: treat the water as carbonic-acid dominated so
    // gameplay surfaces an acid crash instead of snapping back to neutral pH.
    if alk <= 1e-9 {
        return fallback_all_co2(dic, ka1);
    }

    // Quadratic in [H+]:  a·H² + b·H + c = 0
    //   a = Alk
    //   b = Ka1·(Alk − DIC)
    //   c = Ka1·Ka2·(Alk − 2·DIC)
    let a = alk;
    let b = ka1 * (alk - dic);
    let c = ka1 * ka2 * (alk - 2.0 * dic);

    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return fallback_all_co2(dic, ka1);
    }

    let sqrt_d = discriminant.sqrt();
    let h1 = (-b + sqrt_d) / (2.0 * a);
    let h2 = (-b - sqrt_d) / (2.0 * a);

    // Pick the positive root. In the common DIC > Alk regime exactly one
    // root is positive. If both are (unusual), prefer the one inside the
    // diagnostic guard band pH 4.0-10.0.
    let h = if h1 > 0.0 && h2 <= 0.0 {
        h1
    } else if h2 > 0.0 && h1 <= 0.0 {
        h2
    } else if h1 > 0.0 && h2 > 0.0 {
        let ph1 = -h1.log10();
        let ph2 = -h2.log10();
        let in1 = (DIAGNOSTIC_PH_MIN..=DIAGNOSTIC_PH_MAX).contains(&ph1);
        let in2 = (DIAGNOSTIC_PH_MIN..=DIAGNOSTIC_PH_MAX).contains(&ph2);
        match (in1, in2) {
            (true, false) => h1,
            (false, true) => h2,
            _ => h1.max(h2),
        }
    } else {
        // No positive root: fallback.
        return fallback_all_co2(dic, ka1);
    };

    // Reproject the species fractions at the storage pH bounds so pH and the
    // cached carbonate species remain idempotent across snapshots and save-load.
    let ph = clamp_carbonate_ph(-h.log10());
    let (co2_aq_mmol_per_l, hco3_mmol_per_l, co3_mmol_per_l) =
        carbonate_species_for_ph(dic, ph, ka1, ka2);

    CarbonateEquilibrium {
        ph,
        co2_aq_mmol_per_l,
        hco3_mmol_per_l,
        co3_mmol_per_l,
    }
}

/// Resolves the carbonate equilibrium and updates `WaterState.ph` and
/// `WaterState.bicarbonate_mg_total` from the solver output.
///
/// Call this after any mutation of DIC, alkalinity, or temperature to keep
/// derived caches consistent.
pub fn resolve_carbonate_state(water: &mut WaterState, volume_l: f64) {
    let eq = solve_carbonate_equilibrium(
        water.dissolved_inorganic_carbon_mg_c_total,
        water.alkalinity_meq_total,
        water.temperature_c,
        volume_l,
    );
    water.ph = eq.ph;
    water.bicarbonate_mg_total = eq.hco3_mmol_per_l * HCO3_MG_PER_MMOL * volume_l;
}

// ---------------------------------------------------------------------------
// NH3 speciation (unchanged)
// ---------------------------------------------------------------------------

pub fn compute_nh3_mg_l(tan_mg_l: f64, ph: f64, temp_c: f64) -> f64 {
    let pka = 0.09018 + 2729.92 / (273.2 + temp_c);
    let fraction_nh3 = 1.0 / (1.0 + 10.0_f64.powf(pka - ph));
    tan_mg_l * fraction_nh3
}

// ---------------------------------------------------------------------------
// Hourly chemistry step
// ---------------------------------------------------------------------------

pub fn step_hourly_chemistry(state: &mut TankState, light_on: bool) {
    let Some(terms) = hourly_chemistry_terms(state, light_on) else {
        return;
    };

    apply_hourly_chemistry_terms(state, terms);
}

pub fn step_hourly_chemistry_with_budget(state: &mut TankState, light_on: bool) -> BudgetDelta {
    let Some(terms) = hourly_chemistry_terms(state, light_on) else {
        return BudgetDelta::default();
    };

    BudgetDelta {
        carbon: apply_hourly_chemistry_terms(state, terms),
        ..BudgetDelta::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct HourlyChemistryTerms {
    respiration_dic_mg: f64,
    photosynthesis_dic_mg: f64,
}

fn hourly_chemistry_terms(state: &TankState, light_on: bool) -> Option<HourlyChemistryTerms> {
    let volume_l = state.water_volume_l();
    if volume_l <= f64::EPSILON {
        return None;
    }

    // These DIC terms are an intentional atmospheric-exchange simplification:
    // respiration adds CO2 into the lumped DIC pool and light-driven uptake
    // removes it again, even though there is no paired organic-C store inside
    // this hourly chemistry pass. Closed-system carbon conservation therefore
    // only holds when these rates are zeroed unless the guard subtracts this
    // explicit chemistry-stage source/sink budget; preset packs may opt into
    // this open-system shortcut without disabling the rest of the tick guard.
    let respiration_dic_mg = state
        .process_params
        .respiration_dic_rate_mg_c_per_g_per_hour
        * respiring_biomass_g(state);
    let photosynthesis_dic_mg = if light_on {
        state
            .process_params
            .photosynthesis_dic_rate_mg_c_per_g_per_hour
            * photosynthetic_biomass_g(state)
            * state.hardware.light.intensity_index
    } else {
        0.0
    };

    Some(HourlyChemistryTerms {
        respiration_dic_mg,
        photosynthesis_dic_mg,
    })
}

fn apply_hourly_chemistry_terms(
    state: &mut TankState,
    terms: HourlyChemistryTerms,
) -> ElementBudget {
    let dic_before_mg = state.water.dissolved_inorganic_carbon_mg_c_total.max(0.0);
    let carbon_in_mg = terms.respiration_dic_mg.max(0.0);
    let carbon_out_requested_mg = terms.photosynthesis_dic_mg.max(0.0);
    let carbon_out_mg = carbon_out_requested_mg.min(dic_before_mg + carbon_in_mg);

    state.water.dissolved_inorganic_carbon_mg_c_total =
        (dic_before_mg + carbon_in_mg - carbon_out_mg).max(0.0);

    let volume_l = state.water_volume_l();
    resolve_carbonate_state(&mut state.water, volume_l);

    ElementBudget {
        in_mg: carbon_in_mg,
        out_mg: carbon_out_mg,
    }
}

pub(crate) fn respiring_biomass_g(state: &TankState) -> f64 {
    let plant_biomass_g: f64 = state.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
    let algae_biomass_g = state.algae.suspended_biomass_g + state.algae.periphyton_biomass_g;
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let shrimp_biomass_g = (f64::from(state.animal.adults_count) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(state.animal.juveniles_count) * JUVENILE_SHRIMP_BIOMASS_G);

    plant_biomass_g + algae_biomass_g + microbe_biomass_g + shrimp_biomass_g
}

pub(crate) fn photosynthetic_biomass_g(state: &TankState) -> f64 {
    let plant_biomass_g: f64 = state.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
    plant_biomass_g + state.algae.periphyton_biomass_g + state.algae.suspended_biomass_g
}
