use serde::{Deserialize, Serialize};

use crate::types::substrate::SubstrateLayerState;

/// A cohort of berried females that became berried on the same day.
/// Separate cohorts ensure that only completed clutches resolve,
/// preserving the clutch-resolution invariant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EggCohort {
    pub count: u32,
    pub progress_days: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlantGuild {
    FastStem,
    RootFeedingRosette,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantGuildState {
    pub guild: PlantGuild,
    pub biomass_g: f64,
    pub health_index: f64,
    pub crowding_index: f64,
    pub habitat_index: f64,
    /// `None` in old saves → accessor falls back to guild-specific preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water_column_uptake_bias: Option<f64>,
    /// `None` in old saves → accessor falls back to guild-specific preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub substrate_uptake_bias: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlgaeState {
    pub suspended_biomass_g: f64,
    pub periphyton_biomass_g: f64,
    pub nuisance_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicrobeState {
    pub decomposer_biomass_g: f64,
    pub ammonia_oxidizer_biomass_g: f64,
    pub nitrite_oxidizer_biomass_g: f64,
    pub comammox_biomass_g: f64,
    pub maturity_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicrofaunaState {
    pub population_index: f64,
    pub grazing_pressure_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimalState {
    pub adults_count: u32,
    pub juveniles_count: u32,
    pub berried_females_count: u32,
    pub condition_index: f64,
    pub molt_stress_index: f64,
    pub reproductive_readiness_index: f64,
    pub egg_progress_days: f64,
    /// Per-cohort egg tracking for clutch-resolution invariant.
    #[serde(default)]
    pub egg_cohorts: Vec<EggCohort>,
    /// Hidden hourly stress accumulators, reset after daily processing.
    #[serde(default)]
    pub hourly_nh3_stress_accum: f64,
    #[serde(default)]
    pub hourly_nitrite_stress_accum: f64,
    #[serde(default)]
    pub hourly_low_do_stress_accum: f64,
    #[serde(default)]
    pub hourly_heat_stress_accum: f64,
    #[serde(default)]
    pub hourly_instability_stress_accum: f64,
    #[serde(default)]
    pub daily_food_consumed_g: f64,
    /// Fractional maturation accumulator for juvenile → adult promotion.
    #[serde(default)]
    pub maturation_accum: f64,
    /// Assimilated organic reserve (grams of organic matter).
    /// Tracks the retained share of feeding that has not yet been used for
    /// growth/reproduction. Carries both N and C at the feed_n_to_c_ratio.
    /// This pool is the explicit "retained" destination in the consumer
    /// routing contract (see docs/ROUTING.md).
    #[serde(default)]
    pub reserve_g: f64,
}

/// Species-specific shrimp parameters materialized from ShrimpPreset.
/// Stored in TankState for deterministic save/load.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShrimpRuntimeParams {
    pub optimal_temp_min_c: f64,
    pub optimal_temp_max_c: f64,
    pub gh_min_d: f64,
    pub gh_max_d: f64,
    pub base_spawn_rate: f64,
    pub egg_duration_days: u32,
    pub hatch_success_base: f64,
    pub juvenile_sensitivity: f64,
    pub high_temp_repro_penalty_start_c: f64,
    pub high_temp_repro_penalty_full_c: f64,
}

/// Tracks recent chemistry swings for shrimp stress calculations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StabilityTracker {
    pub prev_temp_c: f64,
    pub prev_ph: f64,
    pub prev_gh_d: f64,
    pub prev_do_mg_l: f64,
    pub instability_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetritusState {
    pub particulate_organics_g_total: f64,
    pub fine_detritus_g_total: f64,
    pub dissolved_feed_residue_g_total: f64,
}

impl Default for PlantGuildState {
    fn default() -> Self {
        Self {
            guild: PlantGuild::FastStem,
            biomass_g: 5.0,
            health_index: 0.8,
            crowding_index: 0.1,
            habitat_index: 0.8,
            water_column_uptake_bias: Some(0.9),
            substrate_uptake_bias: Some(0.2),
        }
    }
}

impl Default for AlgaeState {
    fn default() -> Self {
        Self {
            suspended_biomass_g: 0.0,
            periphyton_biomass_g: 0.2,
            nuisance_index: 0.1,
        }
    }
}

impl Default for MicrobeState {
    fn default() -> Self {
        Self {
            decomposer_biomass_g: 0.1,
            ammonia_oxidizer_biomass_g: 0.05,
            nitrite_oxidizer_biomass_g: 0.05,
            comammox_biomass_g: 0.01,
            maturity_index: 0.1,
        }
    }
}

impl Default for MicrofaunaState {
    fn default() -> Self {
        Self {
            population_index: 0.2,
            grazing_pressure_index: 0.2,
        }
    }
}

impl Default for AnimalState {
    fn default() -> Self {
        Self {
            adults_count: 0,
            juveniles_count: 0,
            berried_females_count: 0,
            condition_index: 0.8,
            molt_stress_index: 0.1,
            reproductive_readiness_index: 0.4,
            egg_progress_days: 0.0,
            egg_cohorts: Vec::new(),
            hourly_nh3_stress_accum: 0.0,
            hourly_nitrite_stress_accum: 0.0,
            hourly_low_do_stress_accum: 0.0,
            hourly_heat_stress_accum: 0.0,
            hourly_instability_stress_accum: 0.0,
            daily_food_consumed_g: 0.0,
            maturation_accum: 0.0,
            reserve_g: 0.0,
        }
    }
}

impl Default for ShrimpRuntimeParams {
    fn default() -> Self {
        Self {
            optimal_temp_min_c: 22.0,
            optimal_temp_max_c: 26.0,
            gh_min_d: 5.0,
            gh_max_d: 10.0,
            base_spawn_rate: 0.15,
            egg_duration_days: 21,
            hatch_success_base: 0.7,
            juvenile_sensitivity: 1.5,
            high_temp_repro_penalty_start_c: 28.0,
            high_temp_repro_penalty_full_c: 33.0,
        }
    }
}

impl Default for StabilityTracker {
    fn default() -> Self {
        Self {
            prev_temp_c: 25.0,
            prev_ph: 7.0,
            prev_gh_d: 7.0,
            prev_do_mg_l: 8.0,
            instability_index: 0.0,
        }
    }
}

impl StabilityTracker {
    /// Re-seed baselines from actual water state so the first stability update
    /// does not register a false chemistry swing.
    pub fn seed_from_water(&mut self, water: &super::WaterState, volume_l: f64) {
        self.prev_temp_c = water.temperature_c;
        self.prev_ph = water.ph;
        self.prev_gh_d = water.gh_d(volume_l);
        self.prev_do_mg_l = water.do_mg_per_l(volume_l);
        self.instability_index = 0.0;
    }
}

impl Default for DetritusState {
    fn default() -> Self {
        Self {
            particulate_organics_g_total: 0.0,
            fine_detritus_g_total: 0.0,
            dissolved_feed_residue_g_total: 0.0,
        }
    }
}

impl PlantGuildState {
    pub fn water_column_uptake_bias(&self) -> f64 {
        self.water_column_uptake_bias.unwrap_or(match self.guild {
            PlantGuild::FastStem => 0.9,
            PlantGuild::RootFeedingRosette => 0.3,
        })
    }

    pub fn substrate_uptake_bias(&self) -> f64 {
        self.substrate_uptake_bias.unwrap_or(match self.guild {
            PlantGuild::FastStem => 0.2,
            PlantGuild::RootFeedingRosette => 0.9,
        })
    }
}

impl AnimalState {
    pub fn with_adults(adults_count: u32) -> Self {
        Self {
            adults_count,
            ..Self::default()
        }
    }

    /// Ensures `berried_females_count <= adults_count` by trimming
    /// excess from the newest egg cohorts first.
    pub fn clamp_berried_to_adults(&mut self) {
        if self.berried_females_count > self.adults_count {
            let excess = self.berried_females_count - self.adults_count;
            self.berried_females_count = self.adults_count;
            trim_egg_cohorts(&mut self.egg_cohorts, excess);
        }
    }

    /// Derives `egg_progress_days` from the most-advanced cohort for display.
    pub fn sync_egg_progress_from_cohorts(&mut self) {
        self.egg_progress_days = self
            .egg_cohorts
            .iter()
            .map(|c| c.progress_days)
            .fold(0.0_f64, f64::max);
    }
}

/// Trims `to_remove` berried females from cohorts, starting from the newest (last).
fn trim_egg_cohorts(cohorts: &mut Vec<EggCohort>, mut to_remove: u32) {
    while to_remove > 0 && !cohorts.is_empty() {
        let last = cohorts.last_mut().unwrap();
        if last.count <= to_remove {
            to_remove -= last.count;
            cohorts.pop();
        } else {
            last.count -= to_remove;
            to_remove = 0;
        }
    }
}

pub fn total_colonizable_area_cm2(
    substrate_layers: &[SubstrateLayerState],
    wall_area_cm2: f64,
) -> f64 {
    wall_area_cm2
        + substrate_layers
            .iter()
            .map(|layer| layer.colonizable_area_cm2)
            .sum::<f64>()
}
