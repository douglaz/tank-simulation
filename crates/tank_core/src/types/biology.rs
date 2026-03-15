use serde::{Deserialize, Serialize};

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
        }
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
