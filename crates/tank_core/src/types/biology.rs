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
    /// Retained organic matter from consumer routing (organic matter grams).
    /// Microfauna are index-based so this lightweight reserve pool serves as
    /// the explicit "retained" destination required by the routing contract.
    #[serde(default)]
    pub reserve_g: f64,
}

/// Per-stage cohort with its own count, reserve, condition, and maturation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageCohort {
    pub count: u32,
    /// Assimilated organic reserve (grams of organic matter) for this stage.
    #[serde(default)]
    pub reserve_g: f64,
    /// Condition index for this stage, in [0, 1].
    #[serde(default = "default_condition_index")]
    pub condition_index: f64,
    /// Fractional maturation accumulator for stage promotion.
    #[serde(default)]
    pub maturation_accum: f64,
}

fn default_condition_index() -> f64 {
    0.8
}

impl Default for StageCohort {
    fn default() -> Self {
        Self {
            count: 0,
            reserve_g: 0.0,
            condition_index: 0.8,
            maturation_accum: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimalState {
    /// Adult stage cohort.
    pub adult: StageCohort,
    /// Sub-adult stage cohort (intermediate between juvenile and adult).
    #[serde(default)]
    pub sub_adult: StageCohort,
    /// Juvenile stage cohort.
    pub juvenile: StageCohort,
    /// Subset bookkeeping only: these shrimp are already included in
    /// `adult.count` and therefore do not represent an extra biomass pool.
    pub berried_females_count: u32,
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
    /// Daily consumed food on the shrimp-routing organic-matter basis.
    /// Periphyton source biomass is converted onto that basis before this
    /// field is recorded so satiation and reserve routing use the same units.
    #[serde(default)]
    pub daily_food_consumed_g: f64,
    /// Signed rounding carry for deterministic adult -> berried transfers.
    #[serde(default)]
    pub spawn_progress_accum: f64,
    /// Signed rounding carry for deterministic clutch-resolution counts.
    #[serde(default)]
    pub hatch_success_carry: f64,
    /// Molt readiness index, in [0, 1].
    #[serde(default)]
    pub molt_readiness: f64,
    /// Failed molt accumulator, in [0, 1].
    #[serde(default)]
    pub failed_molt_accum: f64,
    /// Days since the last population-wide molt event.
    #[serde(default = "default_inter_molt_timer_days")]
    pub inter_molt_timer_days: f64,
    /// Whether the most recent molt cycle succeeded.
    #[serde(default = "default_last_molt_success")]
    pub last_molt_success: bool,
}

fn default_inter_molt_timer_days() -> f64 {
    14.0
}

fn default_last_molt_success() -> bool {
    true
}

pub(crate) const ADULT_FEEDING_WEIGHT: f64 = 1.0;
pub(crate) const SUB_ADULT_FEEDING_WEIGHT: f64 = 0.67;
pub(crate) const JUVENILE_FEEDING_WEIGHT: f64 = 0.3;

pub const DEFAULT_SHRIMP_BODY_NITROGEN_MG_PER_G_WET_MASS: f64 = 27.586206896551722;
pub const DEFAULT_SHRIMP_BODY_CARBON_MG_PER_G_WET_MASS: f64 = 172.41379310344828;

/// Species-specific shrimp parameters materialized from ShrimpPreset.
/// Stored in TankState for deterministic save/load.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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
    /// Species/body-composition nitrogen content used for mortality routing and
    /// closed-system shrimp biomass accounting.
    ///
    /// Default preserves the phase-1 tracked wet-mass composition that earlier
    /// releases derived implicitly from the generic detrital `feed_n_to_c_ratio`.
    #[serde(default = "default_shrimp_body_nitrogen_mg_per_g_wet_mass")]
    pub body_nitrogen_mg_per_g_wet_mass: f64,
    /// Species/body-composition carbon content used for mortality routing and
    /// closed-system shrimp biomass accounting.
    ///
    /// Default preserves the phase-1 tracked wet-mass composition that earlier
    /// releases derived implicitly from the generic detrital `feed_n_to_c_ratio`.
    #[serde(default = "default_shrimp_body_carbon_mg_per_g_wet_mass")]
    pub body_carbon_mg_per_g_wet_mass: f64,
    /// Base days for juvenile -> sub-adult transition at optimal conditions.
    #[serde(default = "default_juvenile_to_subadult_days")]
    pub juvenile_to_subadult_days: f64,
    /// Base days for sub-adult -> adult transition at optimal conditions.
    #[serde(default = "default_subadult_to_adult_days")]
    pub subadult_to_adult_days: f64,
    /// Minimum condition for juvenile maturation to proceed.
    #[serde(default = "default_juvenile_maturation_condition_threshold")]
    pub juvenile_maturation_condition_threshold: f64,
    /// Minimum condition for sub-adult maturation to proceed.
    #[serde(default = "default_subadult_maturation_condition_threshold")]
    pub subadult_maturation_condition_threshold: f64,
    /// Base inter-molt period at optimal conditions (days).
    #[serde(default = "default_base_molt_interval_days")]
    pub base_molt_interval_days: f64,
    /// Additional daily mortality fraction per unit of failed_molt_accum.
    #[serde(default = "default_failed_molt_mortality_scale")]
    pub failed_molt_mortality_scale: f64,
    /// Stress sensitivity multiplier for sub-adult mortality.
    #[serde(default = "default_sub_adult_sensitivity")]
    pub sub_adult_sensitivity: f64,
    /// Base juveniles per clutch at good condition.
    #[serde(default = "default_base_clutch_size")]
    pub base_clutch_size: u32,
    /// Condition below which clutch size is zero.
    #[serde(default = "default_min_clutch_condition")]
    pub min_clutch_condition: f64,
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
            reserve_g: 0.0,
        }
    }
}

impl Default for AnimalState {
    fn default() -> Self {
        Self {
            adult: StageCohort::default(),
            sub_adult: StageCohort::default(),
            juvenile: StageCohort::default(),
            berried_females_count: 0,
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
            spawn_progress_accum: 0.0,
            hatch_success_carry: 0.0,
            molt_readiness: 0.0,
            failed_molt_accum: 0.0,
            inter_molt_timer_days: 14.0,
            last_molt_success: true,
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
            body_nitrogen_mg_per_g_wet_mass: default_shrimp_body_nitrogen_mg_per_g_wet_mass(),
            body_carbon_mg_per_g_wet_mass: default_shrimp_body_carbon_mg_per_g_wet_mass(),
            juvenile_to_subadult_days: default_juvenile_to_subadult_days(),
            subadult_to_adult_days: default_subadult_to_adult_days(),
            juvenile_maturation_condition_threshold:
                default_juvenile_maturation_condition_threshold(),
            subadult_maturation_condition_threshold:
                default_subadult_maturation_condition_threshold(),
            base_molt_interval_days: default_base_molt_interval_days(),
            failed_molt_mortality_scale: default_failed_molt_mortality_scale(),
            sub_adult_sensitivity: default_sub_adult_sensitivity(),
            base_clutch_size: default_base_clutch_size(),
            min_clutch_condition: default_min_clutch_condition(),
        }
    }
}

impl ShrimpRuntimeParams {
    /// Splits the legacy total maturation period into juvenile and sub-adult
    /// stage durations while preserving the default 30:20 ratio.
    pub fn split_legacy_total_maturation_days(total_days: f64) -> (f64, f64) {
        let juvenile_ratio = default_juvenile_to_subadult_days()
            / (default_juvenile_to_subadult_days() + default_subadult_to_adult_days());
        let juvenile_to_subadult_days = total_days * juvenile_ratio;
        let subadult_to_adult_days = total_days - juvenile_to_subadult_days;
        (juvenile_to_subadult_days, subadult_to_adult_days)
    }

    pub fn apply_legacy_total_maturation_days(&mut self, total_days: f64) {
        let (juvenile_to_subadult_days, subadult_to_adult_days) =
            Self::split_legacy_total_maturation_days(total_days);
        self.juvenile_to_subadult_days = juvenile_to_subadult_days;
        self.subadult_to_adult_days = subadult_to_adult_days;
    }
}

fn default_shrimp_body_nitrogen_mg_per_g_wet_mass() -> f64 {
    DEFAULT_SHRIMP_BODY_NITROGEN_MG_PER_G_WET_MASS
}

fn default_shrimp_body_carbon_mg_per_g_wet_mass() -> f64 {
    DEFAULT_SHRIMP_BODY_CARBON_MG_PER_G_WET_MASS
}

fn default_juvenile_to_subadult_days() -> f64 {
    30.0
}

fn default_subadult_to_adult_days() -> f64 {
    20.0
}

fn default_juvenile_maturation_condition_threshold() -> f64 {
    0.3
}

fn default_subadult_maturation_condition_threshold() -> f64 {
    0.4
}

fn default_base_molt_interval_days() -> f64 {
    28.0
}

fn default_failed_molt_mortality_scale() -> f64 {
    0.15
}

fn default_sub_adult_sensitivity() -> f64 {
    1.2
}

fn default_base_clutch_size() -> u32 {
    25
}

fn default_min_clutch_condition() -> f64 {
    0.3
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
            adult: StageCohort {
                count: adults_count,
                ..StageCohort::default()
            },
            ..Self::default()
        }
    }

    /// Total shrimp across all stages.
    pub fn total_count(&self) -> u32 {
        self.adult.count + self.sub_adult.count + self.juvenile.count
    }

    /// Feeding demand units used by shrimp grazing and reserve routing.
    pub fn feeding_units(&self) -> f64 {
        f64::from(self.adult.count) * ADULT_FEEDING_WEIGHT
            + f64::from(self.sub_adult.count) * SUB_ADULT_FEEDING_WEIGHT
            + f64::from(self.juvenile.count) * JUVENILE_FEEDING_WEIGHT
    }

    /// Total organic reserve across all stages (grams).
    pub fn total_reserve_g(&self) -> f64 {
        self.adult.reserve_g + self.sub_adult.reserve_g + self.juvenile.reserve_g
    }

    /// Population-weighted condition index across all stages.
    pub fn population_condition_index(&self) -> f64 {
        let total = self.total_count();
        if total == 0 {
            return 0.0;
        }
        let weighted = f64::from(self.adult.count) * self.adult.condition_index
            + f64::from(self.sub_adult.count) * self.sub_adult.condition_index
            + f64::from(self.juvenile.count) * self.juvenile.condition_index;
        weighted / f64::from(total)
    }

    pub fn set_population_condition_index(&mut self, value: f64) {
        let clamped = value.clamp(0.0, 1.0);
        self.adult.condition_index = clamped;
        self.sub_adult.condition_index = clamped;
        self.juvenile.condition_index = clamped;
    }

    pub fn total_maturation_accum(&self) -> f64 {
        self.sub_adult.maturation_accum + self.juvenile.maturation_accum
    }

    /// Adds retained reserve back into the stage pools using the same weights
    /// that the legacy feeding model used for daily food demand.
    pub fn add_reserve_by_feeding_units(&mut self, reserve_g: f64) {
        if reserve_g <= f64::EPSILON {
            return;
        }

        let adult_weight = f64::from(self.adult.count) * ADULT_FEEDING_WEIGHT;
        let sub_adult_weight = f64::from(self.sub_adult.count) * SUB_ADULT_FEEDING_WEIGHT;
        let juvenile_weight = f64::from(self.juvenile.count) * JUVENILE_FEEDING_WEIGHT;
        let total_weight = adult_weight + sub_adult_weight + juvenile_weight;

        if total_weight <= f64::EPSILON {
            self.adult.reserve_g += reserve_g;
            return;
        }

        self.adult.reserve_g += reserve_g * adult_weight / total_weight;
        self.sub_adult.reserve_g += reserve_g * sub_adult_weight / total_weight;
        self.juvenile.reserve_g += reserve_g * juvenile_weight / total_weight;
    }

    pub fn transfer_dead_reserve_g(
        &mut self,
        adult_deaths: u32,
        sub_adult_deaths: u32,
        juvenile_deaths: u32,
    ) -> f64 {
        let adult_fraction = reserve_fraction(self.adult.count, adult_deaths);
        let sub_adult_fraction = reserve_fraction(self.sub_adult.count, sub_adult_deaths);
        let juvenile_fraction = reserve_fraction(self.juvenile.count, juvenile_deaths);

        let adult_transfer = self.adult.reserve_g * adult_fraction;
        let sub_adult_transfer = self.sub_adult.reserve_g * sub_adult_fraction;
        let juvenile_transfer = self.juvenile.reserve_g * juvenile_fraction;

        self.adult.reserve_g -= adult_transfer;
        self.sub_adult.reserve_g -= sub_adult_transfer;
        self.juvenile.reserve_g -= juvenile_transfer;

        adult_transfer + sub_adult_transfer + juvenile_transfer
    }

    /// Ensures `berried_females_count <= adult.count` by trimming
    /// excess from the newest egg cohorts first.
    pub fn clamp_berried_to_adults(&mut self) {
        if self.berried_females_count > self.adult.count {
            let excess = self.berried_females_count - self.adult.count;
            self.berried_females_count = self.adult.count;
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

fn reserve_fraction(total_count: u32, dead_count: u32) -> f64 {
    if total_count == 0 {
        0.0
    } else {
        (f64::from(dead_count) / f64::from(total_count)).clamp(0.0, 1.0)
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
