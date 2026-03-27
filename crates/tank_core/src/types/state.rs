use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rng::{SimRng, SimSeed};

use super::{
    AlgaeState, AnimalState, DetritusState, EnvironmentState, FilterState, HardwareState,
    MicrobeState, MicrofaunaState, PlantGuild, PlantGuildState, ProcessParams, ShrimpRuntimeParams,
    SimEvent, SourceWaterProfile, StabilityTracker, SubstrateKind, SubstrateLayerState,
    TankGeometry, WaterState,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
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
        let substrate_layers = vec![SubstrateLayerState::default()];
        let water = WaterState::default_for_volume_l(
            geometry.water_volume_l_with_substrate_depth(
                substrate_layers
                    .iter()
                    .map(|layer| layer.depth_cm.max(0.0))
                    .sum(),
            ),
        );
        let mut state = Self {
            meta: SimMeta::default(),
            geometry: geometry.clone(),
            environment: EnvironmentState::default(),
            hardware: HardwareState::default(),
            water,
            substrate_layers,
            filter_state: FilterState::default(),
            plant_guilds: vec![
                PlantGuildState::default(),
                PlantGuildState {
                    guild: PlantGuild::RootFeedingRosette,
                    biomass_g: 3.0,
                    health_index: 0.8,
                    crowding_index: 0.1,
                    habitat_index: 0.7,
                    water_column_uptake_bias: Some(0.3),
                    substrate_uptake_bias: Some(0.9),
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
        };
        // Seed stability baseline from the freshly built water state so the
        // first daily update does not register a false chemistry swing.
        let volume_l = state.water_volume_l();
        state
            .stability_tracker
            .seed_from_water(&state.water, volume_l);
        state
    }

    pub fn seeded_example(seed: SimSeed) -> Self {
        let mut state = Self::new(seed);
        let old_volume_l = state.water_volume_l();
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
        let volume_l = state.water_volume_l();
        state
            .water
            .rescale_totals_for_volume(old_volume_l, volume_l);
        state
            .stability_tracker
            .seed_from_water(&state.water, volume_l);
        state
    }

    pub fn avg_substrate_index<F>(&self, select: F) -> f64
    where
        F: Fn(&SubstrateLayerState) -> f64,
    {
        if self.substrate_layers.is_empty() {
            return 0.0;
        }

        let total_depth: f64 = self
            .substrate_layers
            .iter()
            .map(|layer| layer.depth_cm.max(0.0))
            .sum();
        if total_depth <= f64::EPSILON {
            return 0.0;
        }

        let weighted_sum = self
            .substrate_layers
            .iter()
            .map(|layer| layer.depth_cm.max(0.0) * select(layer).clamp(0.0, 1.0))
            .sum::<f64>();

        (weighted_sum / total_depth).clamp(0.0, 1.0)
    }

    pub fn substrate_depth_cm(&self) -> f64 {
        self.substrate_layers
            .iter()
            .map(|layer| layer.depth_cm.max(0.0))
            .sum()
    }

    pub fn substrate_volume_l(&self) -> f64 {
        self.geometry
            .substrate_displacement_l(self.substrate_depth_cm())
            .max(0.0)
    }

    pub fn water_volume_l(&self) -> f64 {
        self.geometry
            .water_volume_l_with_substrate_depth(self.substrate_depth_cm())
            .max(0.0)
    }

    pub fn tan_mg_n_per_l(&self) -> f64 {
        self.water.tan_mg_n_per_l(self.water_volume_l())
    }

    pub fn nitrite_mg_n_per_l(&self) -> f64 {
        self.water.nitrite_mg_n_per_l(self.water_volume_l())
    }

    pub fn nitrate_mg_n_per_l(&self) -> f64 {
        self.water.nitrate_mg_n_per_l(self.water_volume_l())
    }

    pub fn don_mg_n_per_l(&self) -> f64 {
        self.water.don_mg_n_per_l(self.water_volume_l())
    }

    pub fn doc_mg_c_per_l(&self) -> f64 {
        self.water.doc_mg_c_per_l(self.water_volume_l())
    }

    pub fn dic_mg_c_per_l(&self) -> f64 {
        self.water.dic_mg_c_per_l(self.water_volume_l())
    }

    pub fn do_mg_per_l(&self) -> f64 {
        self.water.do_mg_per_l(self.water_volume_l())
    }

    pub fn phosphate_mg_p_per_l(&self) -> f64 {
        self.water.phosphate_mg_p_per_l(self.water_volume_l())
    }

    pub fn alkalinity_meq_per_l(&self) -> f64 {
        self.water.alkalinity_meq_per_l(self.water_volume_l())
    }

    pub fn calcium_mg_per_l(&self) -> f64 {
        self.water.calcium_mg_per_l(self.water_volume_l())
    }

    pub fn magnesium_mg_per_l(&self) -> f64 {
        self.water.magnesium_mg_per_l(self.water_volume_l())
    }

    pub fn gh_d(&self) -> f64 {
        self.water.gh_d(self.water_volume_l())
    }

    pub fn kh_d(&self) -> f64 {
        self.water.kh_d(self.water_volume_l())
    }

    pub fn tds_mg_per_l(&self) -> f64 {
        self.water.tds_mg_per_l(self.water_volume_l())
    }

    pub fn conductivity_us_cm(&self) -> f64 {
        self.water.conductivity_us_cm(self.water_volume_l())
    }
}

impl Default for TankState {
    fn default() -> Self {
        Self::new(SimSeed(0))
    }
}
