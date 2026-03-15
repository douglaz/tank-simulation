use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rng::{SimRng, SimSeed};

use super::{
    AlgaeState, AnimalState, DetritusState, EnvironmentState, FilterState, HardwareState,
    MicrobeState, MicrofaunaState, PlantGuild, PlantGuildState, ProcessParams, ShrimpRuntimeParams,
    SimEvent, SourceWaterProfile, StabilityTracker, SubstrateKind, SubstrateLayerState,
    TankGeometry, WaterState,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimMeta {
    pub scenario_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TankState {
    pub meta: SimMeta,
    pub geometry: TankGeometry,
    pub environment: EnvironmentState,
    pub hardware: HardwareState,
    pub water: WaterState,
    pub substrate_layers: Vec<SubstrateLayerState>,
    pub filter_state: FilterState,
    pub plant_guilds: Vec<PlantGuildState>,
    pub algae: AlgaeState,
    pub microbe: MicrobeState,
    pub microfauna: MicrofaunaState,
    pub animal: AnimalState,
    pub detritus: DetritusState,
    pub event_log: Vec<SimEvent>,
    #[serde(default)]
    pub last_event_day: BTreeMap<String, u32>,
    pub rng: SimRng,
    /// Runtime-resolved source-water catalog keyed by preset id.
    pub source_water_catalog: BTreeMap<String, SourceWaterProfile>,
    /// Runtime-resolved process parameters for heat transfer and chemistry systems.
    pub process_params: ProcessParams,
    /// Materialized shrimp species parameters for deterministic save/load.
    #[serde(default)]
    pub shrimp_params: ShrimpRuntimeParams,
    /// Tracks recent chemistry swings for shrimp stress.
    #[serde(default)]
    pub stability_tracker: StabilityTracker,
}

impl TankState {
    pub fn new(seed: SimSeed) -> Self {
        let geometry = TankGeometry::default();
        Self {
            meta: SimMeta::default(),
            geometry: geometry.clone(),
            environment: EnvironmentState::default(),
            hardware: HardwareState::default(),
            water: WaterState::default_for_geometry(&geometry),
            substrate_layers: vec![SubstrateLayerState::default()],
            filter_state: FilterState::default(),
            plant_guilds: vec![
                PlantGuildState::default(),
                PlantGuildState {
                    guild: PlantGuild::RootFeedingRosette,
                    biomass_g: 3.0,
                    health_index: 0.8,
                    crowding_index: 0.1,
                    habitat_index: 0.7,
                },
            ],
            algae: AlgaeState::default(),
            microbe: MicrobeState::default(),
            microfauna: MicrofaunaState::default(),
            animal: AnimalState::default(),
            detritus: DetritusState::default(),
            event_log: Vec::new(),
            last_event_day: BTreeMap::new(),
            rng: SimRng::new(seed),
            source_water_catalog: BTreeMap::new(),
            process_params: ProcessParams::default(),
            shrimp_params: ShrimpRuntimeParams::default(),
            stability_tracker: StabilityTracker::default(),
        }
    }

    pub fn seeded_example(seed: SimSeed) -> Self {
        let mut state = Self::new(seed);
        state.meta.scenario_id = Some("seeded_example".to_string());
        state.substrate_layers = vec![
            SubstrateLayerState {
                kind: SubstrateKind::ActivePlanted,
                depth_cm: 2.0,
                nutrient_store_mg_n_total: 40.0,
                nutrient_store_mg_p_total: 10.0,
                cation_exchange_capacity_index: 0.8,
                detritus_trapping_index: 0.4,
                colonizable_area_cm2: 400.0,
                low_oxygen_tendency_index: 0.5,
                grazing_surface_index: 0.5,
            },
            SubstrateLayerState {
                kind: SubstrateKind::CoarsePorous,
                depth_cm: 1.0,
                nutrient_store_mg_n_total: 5.0,
                nutrient_store_mg_p_total: 1.0,
                cation_exchange_capacity_index: 0.2,
                detritus_trapping_index: 0.6,
                colonizable_area_cm2: 250.0,
                low_oxygen_tendency_index: 0.6,
                grazing_surface_index: 0.7,
            },
        ];
        state
    }
}

impl Default for SimMeta {
    fn default() -> Self {
        Self {
            scenario_id: None,
            notes: None,
        }
    }
}

impl Default for TankState {
    fn default() -> Self {
        Self::new(SimSeed(0))
    }
}
