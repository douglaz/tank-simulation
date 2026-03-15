use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlayerAction {
    Feed {
        grams: f64,
    },
    WaterChangePercent {
        percent: f64,
        source_profile_id: String,
    },
    TrimPlants {
        fraction: f64,
    },
    SiphonDetritus {
        fraction: f64,
    },
    CleanFilter {
        intensity: f64,
    },
    AddShrimp {
        count: u32,
    },
    RemoveShrimp {
        count: u32,
    },
    ChangePhotoperiod {
        hours: f64,
    },
    ChangeLightIntensity {
        intensity_index: f64,
    },
    ChangeHeaterSetpoint {
        setpoint_c: f64,
    },
    ChangeAmbientTemperature {
        target_c: f64,
    },
    ChangeAeration {
        enabled: bool,
        intensity: f64,
    },
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum SimError {
    #[error("field `{field}` must be finite, got {value}")]
    NonFiniteValue { field: &'static str, value: f64 },
    #[error("field `{field}` must be non-negative, got {value}")]
    NegativeValue { field: &'static str, value: f64 },
    #[error("field `{field}` must be in the range 0.0..=1.0, got {value}")]
    FractionOutOfRange { field: &'static str, value: f64 },
    #[error("field `{field}` must be in the range 0.0..=100.0, got {value}")]
    PercentOutOfRange { field: &'static str, value: f64 },
    #[error("field `{field}` must be in the range 0.0..=24.0, got {value}")]
    HoursOutOfRange { field: &'static str, value: f64 },
    #[error("field `{field}` must be above 0.0 C, got {value}")]
    TemperatureTooLow { field: &'static str, value: f64 },
    #[error("source profile id must not be empty")]
    EmptySourceProfileId,
    #[error("invariant violation for `{field}` with value {value}")]
    InvariantViolation { field: &'static str, value: f64 },
    #[error("unknown source water profile: `{id}`")]
    UnknownSourceProfile { id: String },
    #[error("invalid source water profile `{id}`: field `{field}` has invalid value {value}")]
    InvalidSourceProfile {
        id: String,
        field: &'static str,
        value: f64,
    },
    #[error("schema version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch { expected: u32, actual: u32 },
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("deserialization error: {0}")]
    Deserialization(String),
}

impl PlayerAction {
    pub fn validate(&self) -> Result<(), SimError> {
        match self {
            Self::Feed { grams } => validate_non_negative("grams", *grams),
            Self::WaterChangePercent {
                percent,
                source_profile_id,
            } => {
                validate_percent("percent", *percent)?;
                if source_profile_id.trim().is_empty() {
                    Err(SimError::EmptySourceProfileId)
                } else {
                    Ok(())
                }
            }
            Self::TrimPlants { fraction }
            | Self::SiphonDetritus { fraction }
            | Self::CleanFilter {
                intensity: fraction,
            } => validate_fraction(field_name(self), *fraction),
            Self::AddShrimp { .. } | Self::RemoveShrimp { .. } => Ok(()),
            Self::ChangePhotoperiod { hours } => {
                validate_finite("hours", *hours)?;
                if (0.0..=24.0).contains(hours) {
                    Ok(())
                } else {
                    Err(SimError::HoursOutOfRange {
                        field: "hours",
                        value: *hours,
                    })
                }
            }
            Self::ChangeLightIntensity { intensity_index } => {
                validate_fraction("intensity_index", *intensity_index)
            }
            Self::ChangeHeaterSetpoint { setpoint_c } => {
                validate_temperature("setpoint_c", *setpoint_c)
            }
            Self::ChangeAmbientTemperature { target_c } => {
                validate_temperature("target_c", *target_c)
            }
            Self::ChangeAeration { intensity, .. } => validate_fraction("intensity", *intensity),
        }
    }
}

fn field_name(action: &PlayerAction) -> &'static str {
    match action {
        PlayerAction::TrimPlants { .. } => "fraction",
        PlayerAction::SiphonDetritus { .. } => "fraction",
        PlayerAction::CleanFilter { .. } => "intensity",
        _ => "value",
    }
}

fn validate_non_negative(field: &'static str, value: f64) -> Result<(), SimError> {
    validate_finite(field, value)?;
    if value < 0.0 {
        Err(SimError::NegativeValue { field, value })
    } else {
        Ok(())
    }
}

fn validate_fraction(field: &'static str, value: f64) -> Result<(), SimError> {
    validate_finite(field, value)?;
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(SimError::FractionOutOfRange { field, value })
    }
}

fn validate_percent(field: &'static str, value: f64) -> Result<(), SimError> {
    validate_finite(field, value)?;
    if (0.0..=100.0).contains(&value) {
        Ok(())
    } else {
        Err(SimError::PercentOutOfRange { field, value })
    }
}

fn validate_temperature(field: &'static str, value: f64) -> Result<(), SimError> {
    validate_finite(field, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(SimError::TemperatureTooLow { field, value })
    }
}

fn validate_finite(field: &'static str, value: f64) -> Result<(), SimError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SimError::NonFiniteValue { field, value })
    }
}
