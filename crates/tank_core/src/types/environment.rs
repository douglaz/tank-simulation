use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentState {
    pub ambient_temp_c: f64,
    pub hour_of_day: u8,
    pub day: u32,
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            ambient_temp_c: 24.0,
            hour_of_day: 0,
            day: 0,
        }
    }
}
