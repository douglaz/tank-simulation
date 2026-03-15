use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LightState {
    pub enabled: bool,
    pub photoperiod_hours: f64,
    pub intensity_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaterState {
    pub enabled: bool,
    pub setpoint_c: f64,
    pub deadband_c: f64,
    pub max_watts: f64,
    pub efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterHardware {
    pub enabled: bool,
    pub flow_lph: f64,
    pub cleanliness_index: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AerationState {
    pub enabled: bool,
    pub intensity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HardwareState {
    pub light: LightState,
    pub heater: HeaterState,
    pub filter: FilterHardware,
    pub aeration: AerationState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterState {
    pub biofilter_maturity_index: f64,
    pub clogging_index: f64,
    pub seeded_biomass_index: f64,
}

impl Default for LightState {
    fn default() -> Self {
        Self {
            enabled: true,
            photoperiod_hours: 8.0,
            intensity_index: 0.7,
        }
    }
}

impl Default for HeaterState {
    fn default() -> Self {
        Self {
            enabled: false,
            setpoint_c: 24.0,
            deadband_c: 0.5,
            max_watts: 50.0,
            efficiency: 1.0,
        }
    }
}

impl Default for FilterHardware {
    fn default() -> Self {
        Self {
            enabled: true,
            flow_lph: 200.0,
            cleanliness_index: 1.0,
        }
    }
}

impl Default for AerationState {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.0,
        }
    }
}

impl Default for HardwareState {
    fn default() -> Self {
        Self {
            light: LightState::default(),
            heater: HeaterState::default(),
            filter: FilterHardware::default(),
            aeration: AerationState::default(),
        }
    }
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            biofilter_maturity_index: 0.1,
            clogging_index: 0.0,
            seeded_biomass_index: 0.0,
        }
    }
}
