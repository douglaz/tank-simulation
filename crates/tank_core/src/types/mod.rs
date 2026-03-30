pub mod actions;
pub mod biology;
pub mod budget;
pub mod environment;
pub mod events;
pub mod geometry;
pub mod habitat;
pub mod hardware;
pub mod process;
pub mod provenance;
pub mod snapshot;
pub mod source_water;
pub mod state;
pub mod substrate;
pub mod water;

pub use actions::{PlayerAction, SimError};
pub use biology::{
    total_colonizable_area_cm2, AlgaeState, AnimalState, DetritusState, EggCohort, MicrobeState,
    MicrofaunaState, PlantGuild, PlantGuildState, ShrimpRuntimeParams, StabilityTracker,
    StageCohort, DEFAULT_SHRIMP_BODY_CARBON_MG_PER_G_WET_MASS,
    DEFAULT_SHRIMP_BODY_NITROGEN_MG_PER_G_WET_MASS,
};
pub(crate) use budget::BudgetSnapshot;
pub use budget::{
    algae_carbon_mg, algae_detrital_mass_g, algae_nitrogen_mg, carbon_budget_components,
    detritus_carbon_mg, detritus_nitrogen_mg, live_biomass_carbon_mg, live_biomass_detrital_mass_g,
    live_biomass_nitrogen_mg, nitrogen_budget_components, plant_carbon_mg, plant_nitrogen_mg,
    shrimp_biomass_g, shrimp_body_carbon_mg, shrimp_body_detrital_mass_g, shrimp_body_nitrogen_mg,
    shrimp_carbon_mg, shrimp_nitrogen_mg, total_nitrogen_mg, BudgetComponent, BudgetDelta,
    BudgetEntry, BudgetLedger, BudgetMetric, BudgetMetricUnit, BudgetRecordingKind, BudgetTotals,
    ElementBudget, TickBudgetRecord, ADULT_SHRIMP_BIOMASS_G, ALGAE_N_MG_PER_G_BIOMASS,
    JUVENILE_SHRIMP_BIOMASS_G, LIVE_BIOMASS_ORGANIC_FRACTION_G_PER_G, PLANT_N_MG_PER_G_BIOMASS,
    SHRIMP_C_MG_PER_G_WET_MASS, SHRIMP_N_MG_PER_G_WET_MASS, SUB_ADULT_SHRIMP_BIOMASS_G,
};
pub use environment::EnvironmentState;
pub use events::{EventCause, EventKind, EventSeverity, SimEvent};
pub use geometry::TankGeometry;
pub use habitat::{compute_habitat_registry, find_habitat, HabitatEntry, HabitatKind};
pub use hardware::{
    AerationState, FilterHardware, FilterState, HardwareState, HeaterState, LightState,
};
pub use process::{
    legacy_total_param_to_mg_per_l, legacy_total_param_to_mg_per_m2, ProcessParams,
    DENITRIFICATION_ALK_MEQ_PER_MG_N, LEGACY_KINETIC_REFERENCE_FOOTPRINT_M2,
    LEGACY_KINETIC_REFERENCE_VOLUME_L, MG_N_PER_MEQ_AMMONIA, NITRIFICATION_ALK_MEQ_PER_MG_N,
};
pub use provenance::{
    check_all_ranges, check_param_range, format_param, ConfidenceLevel, ParamMeta, RangeWarning,
};
pub use snapshot::{TankSnapshot, LEGACY_SNAPSHOT_CHEMISTRY_FIELD_ALIASES};
pub use source_water::SourceWaterProfile;
pub use state::{SimMeta, TankState};
pub use substrate::{
    SubstrateKind, SubstrateLayerState, SubstrateZone, SubstrateZoneGeometry,
    SubstrateZoneNutrientAvailability,
};
pub(crate) use water::concentration_from_total;
pub use water::{
    ConcentrationView, WaterState, ESTIMATED_TDS_OMITTED_CONTRIBUTORS, ESTIMATED_TDS_SCOPE_LINES,
    ESTIMATED_TDS_TRACKED_MAJOR_IONS,
};
