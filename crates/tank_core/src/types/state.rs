use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::rng::{SimRng, SimSeed};

use super::{
    AlgaeState, AnimalState, BudgetTotals, ConcentrationView, DetritusState, EnvironmentState,
    FilterState, HabitatEntry, HardwareState, MicrobeState, MicrofaunaState, PlantGuild,
    PlantGuildState, ProcessParams, ShrimpRuntimeParams, SimEvent, SourceWaterProfile,
    StabilityTracker, SubstrateKind, SubstrateLayerState, SubstrateZone, SubstrateZoneGeometry,
    SubstrateZoneNutrientAvailability, TankGeometry, WaterState,
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
    /// Derived serialized habitat registry: colonizable areas and exposure
    /// modifiers for each ecological zone. Refresh whenever geometry,
    /// hardware, substrate, filter state, or plant state changes.
    #[serde(default)]
    pub habitat_registry: Vec<HabitatEntry>,
    /// Cumulative nitrogen permanently removed from the system as N₂ gas
    /// via denitrification (mg N). This is an explicit export pathway in
    /// the nitrogen budget — not recycled, not unexplained loss.
    #[serde(default)]
    pub cumulative_n2_export_mg_n: f64,
}

impl TankState {
    pub fn new(seed: SimSeed) -> Self {
        let geometry = TankGeometry::default();
        let default_substrate = SubstrateLayerState::default();
        let substrate_layers = vec![SubstrateLayerState {
            colonizable_area_cm2: default_substrate
                .derived_colonizable_area_cm2(geometry.footprint_area_cm2()),
            ..default_substrate
        }];
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
            habitat_registry: Vec::new(),
            cumulative_n2_export_mg_n: 0.0,
        };
        // Seed stability baseline from the freshly built water state so the
        // first daily update does not register a false chemistry swing.
        state.reseed_stability_tracker();
        state.refresh_habitat_registry();
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
                colonizable_area_factor: SubstrateKind::ActivePlanted
                    .default_colonizable_area_factor(),
                colonizable_area_cm2: 400.0,
                low_oxygen_tendency_index: 0.5,
                grazing_surface_index: 0.5,
                porosity: SubstrateKind::ActivePlanted.default_porosity(),
                o2_penetration_depth_cm: 2.0,
            },
            SubstrateLayerState {
                kind: SubstrateKind::CoarsePorous,
                depth_cm: 1.0,
                nutrient_store_mg_n_total: 5.0,
                nutrient_store_mg_p_total: 1.0,
                cation_exchange_capacity_index: 0.2,
                detritus_trapping_index: 0.6,
                colonizable_area_factor: SubstrateKind::CoarsePorous
                    .default_colonizable_area_factor(),
                colonizable_area_cm2: 250.0,
                low_oxygen_tendency_index: 0.6,
                grazing_surface_index: 0.7,
                porosity: SubstrateKind::CoarsePorous.default_porosity(),
                o2_penetration_depth_cm: 1.0,
            },
        ];
        let volume_l = state.water_volume_l();
        state
            .water
            .rescale_totals_for_volume(old_volume_l, volume_l);
        state.reseed_stability_tracker();
        state.refresh_habitat_registry();
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

    pub fn water_depth_above_substrate_cm(&self) -> f64 {
        (self.geometry.fill_height_cm - self.substrate_depth_cm())
            .clamp(0.0, self.geometry.fill_height_cm.max(0.0))
    }

    /// Canonical shared O₂ penetration boundary depth (cm below the
    /// substrate surface) for the full stacked substrate bed.
    pub fn substrate_o2_penetration_depth_cm(&self) -> f64 {
        let total_depth_cm = self.substrate_depth_cm();
        if total_depth_cm <= f64::EPSILON {
            return 0.0;
        }

        self.substrate_layers
            .iter()
            .map(|layer| layer.resolved_o2_penetration_depth_cm(total_depth_cm))
            .fold(total_depth_cm, f64::min)
            .clamp(0.0, total_depth_cm)
    }

    pub fn substrate_zone_geometry(&self, zone: SubstrateZone) -> SubstrateZoneGeometry {
        let footprint_area_cm2 = self.geometry.footprint_area_cm2();
        let o2_penetration_depth_cm = self.substrate_o2_penetration_depth_cm();
        let mut geometry = SubstrateZoneGeometry::default();
        let mut layer_top_depth_cm = 0.0;

        for layer in &self.substrate_layers {
            geometry.depth_cm +=
                layer.zone_depth_cm(layer_top_depth_cm, o2_penetration_depth_cm, zone);
            geometry.volume_cm3 += layer.zone_volume_cm3(
                layer_top_depth_cm,
                o2_penetration_depth_cm,
                footprint_area_cm2,
                zone,
            );
            geometry.pore_volume_cm3 += layer.zone_pore_volume_cm3(
                layer_top_depth_cm,
                o2_penetration_depth_cm,
                footprint_area_cm2,
                zone,
            );
            geometry.colonizable_area_cm2 += layer.zone_colonizable_area_cm2(
                layer_top_depth_cm,
                o2_penetration_depth_cm,
                footprint_area_cm2,
                zone,
            );
            layer_top_depth_cm += layer.depth_cm.max(0.0);
        }

        geometry
    }

    pub fn substrate_oxic_zone_geometry(&self) -> SubstrateZoneGeometry {
        self.substrate_zone_geometry(SubstrateZone::Oxic)
    }

    pub fn substrate_suboxic_zone_geometry(&self) -> SubstrateZoneGeometry {
        self.substrate_zone_geometry(SubstrateZone::Suboxic)
    }

    pub fn substrate_oxic_volume_cm3(&self) -> f64 {
        self.substrate_oxic_zone_geometry().volume_cm3
    }

    pub fn substrate_suboxic_volume_cm3(&self) -> f64 {
        self.substrate_suboxic_zone_geometry().volume_cm3
    }

    pub fn substrate_oxic_pore_volume_cm3(&self) -> f64 {
        self.substrate_oxic_zone_geometry().pore_volume_cm3
    }

    pub fn substrate_suboxic_pore_volume_cm3(&self) -> f64 {
        self.substrate_suboxic_zone_geometry().pore_volume_cm3
    }

    pub fn substrate_zone_nutrient_availability(
        &self,
        zone: SubstrateZone,
    ) -> SubstrateZoneNutrientAvailability {
        let footprint_area_m2 = self.geometry.footprint_area_m2();
        let o2_penetration_depth_cm = self.substrate_o2_penetration_depth_cm();
        let mut nutrients = SubstrateZoneNutrientAvailability::default();
        let mut layer_top_depth_cm = 0.0;

        for layer in &self.substrate_layers {
            let layer_nutrients = layer.zone_nutrient_availability(
                layer_top_depth_cm,
                o2_penetration_depth_cm,
                footprint_area_m2,
                zone,
            );
            nutrients.nitrogen_mg_total += layer_nutrients.nitrogen_mg_total;
            nutrients.phosphorus_mg_total += layer_nutrients.phosphorus_mg_total;
            layer_top_depth_cm += layer.depth_cm.max(0.0);
        }

        nutrients.nitrogen_mg_per_m2 =
            area_density_mg_per_m2(nutrients.nitrogen_mg_total, footprint_area_m2);
        nutrients.phosphorus_mg_per_m2 =
            area_density_mg_per_m2(nutrients.phosphorus_mg_total, footprint_area_m2);

        nutrients
    }

    pub fn substrate_oxic_nutrient_availability(&self) -> SubstrateZoneNutrientAvailability {
        self.substrate_zone_nutrient_availability(SubstrateZone::Oxic)
    }

    pub fn substrate_suboxic_nutrient_availability(&self) -> SubstrateZoneNutrientAvailability {
        self.substrate_zone_nutrient_availability(SubstrateZone::Suboxic)
    }

    /// Extinction coefficient k (1/cm) for Beer-Lambert light attenuation.
    ///
    /// k = k_water + k_algae × \[algae\] + k_doc × \[DOC\] + k_detritus × \[detritus\]
    ///
    /// where brackets denote concentrations derived from biomass totals and
    /// water volume.
    pub fn extinction_coefficient(&self) -> f64 {
        let volume_l = self.water_volume_l().max(f64::EPSILON);
        let algae_conc = self.algae.suspended_biomass_g / volume_l;
        let doc_conc = self.water.dissolved_organic_carbon_mg_c_total / volume_l;
        let detritus_conc = self.detritus.fine_detritus_g_total / volume_l;
        (self.process_params.base_extinction_coeff_per_cm
            + self.process_params.algae_extinction_coeff_per_cm_per_g_l * algae_conc
            + self.process_params.doc_extinction_coeff_per_cm_per_mg_c_l * doc_conc
            + self.process_params.detritus_extinction_coeff_per_cm_per_g_l * detritus_conc)
            .max(0.0)
    }

    pub fn derived_plant_crowding_index(&self) -> f64 {
        let surface_area_m2 = self.geometry.footprint_area_m2().max(f64::MIN_POSITIVE);
        let total_plant_biomass_g: f64 =
            self.plant_guilds.iter().map(|plant| plant.biomass_g).sum();
        (total_plant_biomass_g
            / (surface_area_m2 * self.process_params.plant_crowding_biomass_g_per_m2.max(1.0)))
        .clamp(0.0, 1.0)
    }

    pub fn substrate_n_mg_n_per_m2(&self) -> f64 {
        area_density_mg_per_m2(
            self.substrate_layers
                .iter()
                .map(|layer| layer.nutrient_store_mg_n_total)
                .sum(),
            self.geometry.footprint_area_m2(),
        )
    }

    pub fn substrate_p_mg_p_per_m2(&self) -> f64 {
        area_density_mg_per_m2(
            self.substrate_layers
                .iter()
                .map(|layer| layer.nutrient_store_mg_p_total)
                .sum(),
            self.geometry.footprint_area_m2(),
        )
    }

    pub fn reseed_stability_tracker(&mut self) {
        let volume_l = self.water_volume_l();
        self.stability_tracker
            .seed_from_water(&self.water, volume_l);
    }

    pub fn refresh_habitat_registry(&mut self) {
        let crowding_index = self.derived_plant_crowding_index();
        for plant in &mut self.plant_guilds {
            plant.crowding_index = crowding_index;
        }
        let footprint_area_cm2 = self.geometry.footprint_area_cm2();
        for layer in &mut self.substrate_layers {
            layer.colonizable_area_factor = layer.resolved_colonizable_area_factor();
            layer.colonizable_area_cm2 = layer.derived_colonizable_area_cm2(footprint_area_cm2);
        }
        let registry = super::habitat::compute_habitat_registry(self);
        self.habitat_registry = registry;

        // Reconcile per-habitat pools against the refreshed registry.
        self.ensure_habitat_pools();
    }

    /// Normalize per-habitat biomass maps against the current habitat registry.
    ///
    /// Legacy saves still backfill from lumped totals, while existing maps are
    /// also pruned and redistributed when habitats disappear or appear.
    fn ensure_habitat_pools(&mut self) {
        self.algae
            .normalize_periphyton_habitats(&self.habitat_registry);
        self.microbe
            .normalize_decomposer_habitats(&self.habitat_registry);
    }

    pub fn concentrations(&self) -> ConcentrationView<'_> {
        self.water.concentration_view(self.water_volume_l())
    }

    pub fn tan_mg_n_per_l(&self) -> f64 {
        self.concentrations().tan_mg_n_per_l()
    }

    pub fn nitrite_mg_n_per_l(&self) -> f64 {
        self.concentrations().nitrite_mg_n_per_l()
    }

    pub fn nitrate_mg_n_per_l(&self) -> f64 {
        self.concentrations().nitrate_mg_n_per_l()
    }

    pub fn don_mg_n_per_l(&self) -> f64 {
        self.concentrations().don_mg_n_per_l()
    }

    pub fn doc_mg_c_per_l(&self) -> f64 {
        self.concentrations().doc_mg_c_per_l()
    }

    pub fn dic_mg_c_per_l(&self) -> f64 {
        self.concentrations().dic_mg_c_per_l()
    }

    pub fn do_mg_per_l(&self) -> f64 {
        self.concentrations().do_mg_per_l()
    }

    pub fn total_nitrogen(&self) -> f64 {
        BudgetTotals::from_state(self).nitrogen_mg
    }

    pub fn total_carbon(&self) -> f64 {
        BudgetTotals::from_state(self).carbon_mg
    }

    pub fn budget_totals(&self) -> BudgetTotals {
        BudgetTotals::from_state(self)
    }

    pub fn phosphate_mg_p_per_l(&self) -> f64 {
        self.concentrations().phosphate_mg_p_per_l()
    }

    pub fn alkalinity_meq_per_l(&self) -> f64 {
        self.concentrations().alkalinity_meq_per_l()
    }

    pub fn calcium_mg_per_l(&self) -> f64 {
        self.concentrations().calcium_mg_per_l()
    }

    pub fn magnesium_mg_per_l(&self) -> f64 {
        self.concentrations().magnesium_mg_per_l()
    }

    pub fn gh_d(&self) -> f64 {
        self.concentrations().gh_d()
    }

    pub fn kh_d(&self) -> f64 {
        self.concentrations().kh_d()
    }

    pub fn tds_mg_per_l(&self) -> f64 {
        self.concentrations().tds_mg_per_l()
    }

    pub fn conductivity_us_cm(&self) -> f64 {
        self.concentrations().conductivity_us_cm()
    }
}

impl Default for TankState {
    fn default() -> Self {
        Self::new(SimSeed(0))
    }
}

fn area_density_mg_per_m2(total_mg: f64, area_m2: f64) -> f64 {
    if !total_mg.is_finite() || !area_m2.is_finite() || area_m2 <= f64::EPSILON {
        0.0
    } else {
        (total_mg / area_m2).max(0.0)
    }
}
