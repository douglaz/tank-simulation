use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventKind {
    CycleProgressing,
    AmmoniaWarning,
    NitriteWarning,
    OxygenDip,
    AlgaeRiskRising,
    AlgaeBloom,
    BiofilmMaturityIncrease,
    ShrimpBerried,
    EggFailure,
    MoltFailure,
    MoltStressWarning,
    FilterCleaningSetback,
    SubstrateFoulingWarning,
    StabilityImproving,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventCause {
    RoutineAction,
    Overfeeding,
    WaterChange,
    FilterMaintenance,
    HighAmmonia,
    HighNitrite,
    LowOxygen,
    LowTemperature,
    HighTemperature,
    LowMinerals,
    BiofilterImmature,
    SurfaceExchangeRestricted,
    HighNutrients,
    PlantCrowding,
    SurfaceSaturation,
    SubstrateExhausted,
    ChemistryInstability,
    Starvation,
    PoorCondition,
    PlantTrimming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimEvent {
    pub day: u32,
    pub hour: u8,
    pub severity: EventSeverity,
    pub kind: EventKind,
    pub cause_codes: Vec<EventCause>,
    pub summary: String,
}

impl SimEvent {
    pub fn new(
        day: u32,
        hour: u8,
        severity: EventSeverity,
        kind: EventKind,
        cause_codes: Vec<EventCause>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            day,
            hour,
            severity,
            kind,
            cause_codes,
            summary: summary.into(),
        }
    }
}
