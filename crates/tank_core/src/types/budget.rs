use serde::{Deserialize, Serialize};

use super::TankState;

pub const ADULT_SHRIMP_BIOMASS_G: f64 = 0.12;
pub const JUVENILE_SHRIMP_BIOMASS_G: f64 = 0.05;
pub const PLANT_N_MG_PER_G_BIOMASS: f64 = 28.0;
pub const ALGAE_N_MG_PER_G_BIOMASS: f64 = 35.0;
pub const LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G: f64 = 0.20;
const DEFAULT_N_TO_C_RATIO: f64 = 0.16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct ElementBudget {
    pub in_mg: f64,
    pub out_mg: f64,
}

impl ElementBudget {
    pub fn from_delta(delta_mg: f64) -> Self {
        if delta_mg >= 0.0 {
            Self {
                in_mg: delta_mg,
                out_mg: 0.0,
            }
        } else {
            Self {
                in_mg: 0.0,
                out_mg: -delta_mg,
            }
        }
    }

    pub fn net_mg(&self) -> f64 {
        self.in_mg - self.out_mg
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct BudgetDelta {
    pub nitrogen: ElementBudget,
    pub carbon: ElementBudget,
    pub oxygen: ElementBudget,
}

impl BudgetDelta {
    pub fn between(before: BudgetTotals, after: BudgetTotals) -> Self {
        Self {
            nitrogen: ElementBudget::from_delta(after.nitrogen_mg - before.nitrogen_mg),
            carbon: ElementBudget::from_delta(after.carbon_mg - before.carbon_mg),
            oxygen: ElementBudget::from_delta(after.oxygen_mg - before.oxygen_mg),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct BudgetTotals {
    pub nitrogen_mg: f64,
    pub carbon_mg: f64,
    pub oxygen_mg: f64,
}

impl BudgetTotals {
    pub fn from_state(state: &TankState) -> Self {
        Self {
            nitrogen_mg: total_nitrogen_mg(state),
            carbon_mg: total_carbon_mg(state),
            oxygen_mg: state.water.dissolved_oxygen_mg_total.max(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetEntry {
    pub label: String,
    pub delta: BudgetDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TickBudgetRecord {
    pub tick_index: usize,
    pub day: u32,
    pub hour: u32,
    pub before: BudgetTotals,
    pub after: BudgetTotals,
    pub net_delta: BudgetDelta,
    pub entries: Vec<BudgetEntry>,
}

impl TickBudgetRecord {
    pub fn start(state: &TankState, tick_index: usize) -> Self {
        let totals = BudgetTotals::from_state(state);
        Self {
            tick_index,
            day: state.environment.day,
            hour: state.environment.hour_of_day,
            before: totals,
            after: totals,
            net_delta: BudgetDelta::default(),
            entries: Vec::new(),
        }
    }

    pub fn record_stage(&mut self, label: &str, before: BudgetTotals, after: BudgetTotals) {
        self.after = after;
        self.net_delta = BudgetDelta::between(self.before, after);
        self.entries.push(BudgetEntry {
            label: label.to_owned(),
            delta: BudgetDelta::between(before, after),
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BudgetLedger {
    pub ticks: Vec<TickBudgetRecord>,
}

impl BudgetLedger {
    pub fn push_tick(&mut self, tick: TickBudgetRecord) {
        self.ticks.push(tick);
    }
}

pub fn total_nitrogen_mg(state: &TankState) -> f64 {
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let plant_n_mg: f64 = state
        .plant_guilds
        .iter()
        .map(|plant| plant_nitrogen_mg(plant.biomass_g))
        .sum();
    let algae_n_mg = algae_nitrogen_mg(state.algae.suspended_biomass_g)
        + algae_nitrogen_mg(state.algae.periphyton_biomass_g);
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;
    let substrate_n_mg: f64 = state
        .substrate_layers
        .iter()
        .map(|layer| layer.nutrient_store_mg_n_total)
        .sum();

    state.water.ammonia_total_mg_n_total
        + state.water.nitrite_mg_n_total
        + state.water.nitrate_mg_n_total
        + state.water.dissolved_organic_nitrogen_mg_n_total
        + substrate_n_mg
        + plant_n_mg
        + algae_n_mg
        + live_biomass_nitrogen_mg(microbe_biomass_g, n_to_c_ratio)
        + shrimp_nitrogen_mg(
            state.animal.adults_count,
            state.animal.juveniles_count,
            n_to_c_ratio,
        )
        + detritus_nitrogen_mg(
            state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total,
            n_to_c_ratio,
        )
}

pub fn total_carbon_mg(state: &TankState) -> f64 {
    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    let plant_c_mg: f64 = state
        .plant_guilds
        .iter()
        .map(|plant| plant_carbon_mg(plant.biomass_g, n_to_c_ratio))
        .sum();
    let algae_c_mg = algae_carbon_mg(state.algae.suspended_biomass_g, n_to_c_ratio)
        + algae_carbon_mg(state.algae.periphyton_biomass_g, n_to_c_ratio);
    let microbe_biomass_g = state.microbe.decomposer_biomass_g
        + state.microbe.ammonia_oxidizer_biomass_g
        + state.microbe.nitrite_oxidizer_biomass_g
        + state.microbe.comammox_biomass_g;

    state.water.dissolved_inorganic_carbon_mg_c_total
        + state.water.dissolved_organic_carbon_mg_c_total
        + plant_c_mg
        + algae_c_mg
        + live_biomass_carbon_mg(microbe_biomass_g, n_to_c_ratio)
        + shrimp_carbon_mg(
            state.animal.adults_count,
            state.animal.juveniles_count,
            n_to_c_ratio,
        )
        + detritus_carbon_mg(
            state.detritus.particulate_organics_g_total + state.detritus.fine_detritus_g_total,
            n_to_c_ratio,
        )
}

pub fn plant_nitrogen_mg(biomass_g: f64) -> f64 {
    biomass_g.max(0.0) * PLANT_N_MG_PER_G_BIOMASS
}

pub fn plant_carbon_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    carbon_from_nitrogen_mg(plant_nitrogen_mg(biomass_g), n_to_c_ratio)
}

pub fn algae_nitrogen_mg(biomass_g: f64) -> f64 {
    biomass_g.max(0.0) * ALGAE_N_MG_PER_G_BIOMASS
}

pub fn algae_carbon_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    carbon_from_nitrogen_mg(algae_nitrogen_mg(biomass_g), n_to_c_ratio)
}

pub fn live_biomass_nitrogen_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    organic_nitrogen_mg(
        biomass_g.max(0.0) * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
        n_to_c_ratio,
    )
}

pub fn live_biomass_carbon_mg(biomass_g: f64, n_to_c_ratio: f64) -> f64 {
    organic_carbon_mg(
        biomass_g.max(0.0) * LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G,
        n_to_c_ratio,
    )
}

pub fn shrimp_biomass_g(adults_count: u32, juveniles_count: u32) -> f64 {
    (f64::from(adults_count) * ADULT_SHRIMP_BIOMASS_G)
        + (f64::from(juveniles_count) * JUVENILE_SHRIMP_BIOMASS_G)
}

pub fn shrimp_nitrogen_mg(adults_count: u32, juveniles_count: u32, n_to_c_ratio: f64) -> f64 {
    live_biomass_nitrogen_mg(
        shrimp_biomass_g(adults_count, juveniles_count),
        n_to_c_ratio,
    )
}

pub fn shrimp_carbon_mg(adults_count: u32, juveniles_count: u32, n_to_c_ratio: f64) -> f64 {
    live_biomass_carbon_mg(shrimp_biomass_g(adults_count, juveniles_count), n_to_c_ratio)
}

pub fn detritus_nitrogen_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    organic_nitrogen_mg(mass_g, n_to_c_ratio)
}

pub fn detritus_carbon_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    organic_carbon_mg(mass_g, n_to_c_ratio)
}

fn organic_nitrogen_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    let ratio = sanitize_n_to_c_ratio(n_to_c_ratio);
    mass_g.max(0.0) * 1000.0 * ratio / (1.0 + ratio)
}

fn organic_carbon_mg(mass_g: f64, n_to_c_ratio: f64) -> f64 {
    let ratio = sanitize_n_to_c_ratio(n_to_c_ratio);
    mass_g.max(0.0) * 1000.0 / (1.0 + ratio)
}

fn carbon_from_nitrogen_mg(nitrogen_mg: f64, n_to_c_ratio: f64) -> f64 {
    nitrogen_mg.max(0.0) / sanitize_n_to_c_ratio(n_to_c_ratio)
}

fn sanitize_n_to_c_ratio(n_to_c_ratio: f64) -> f64 {
    if n_to_c_ratio.is_finite() && n_to_c_ratio > f64::EPSILON {
        n_to_c_ratio
    } else {
        DEFAULT_N_TO_C_RATIO
    }
}
