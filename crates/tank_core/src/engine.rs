use std::collections::VecDeque;

use crate::{
    invariants::enforce_invariants,
    rng::SimSeed,
    types::{
        EventCause, EventKind, EventSeverity, PlayerAction, SimError, SimEvent, TankSnapshot,
        TankState,
    },
};

pub trait SimulationEngine {
    fn apply_action(&mut self, action: PlayerAction) -> Result<(), SimError>;
    fn step_hours(&mut self, hours: u32) -> Result<(), SimError>;
    fn snapshot(&self) -> TankSnapshot;
    fn full_state(&self) -> &TankState;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    state: TankState,
    queued_actions: VecDeque<PlayerAction>,
}

impl Engine {
    pub fn new(seed: SimSeed) -> Self {
        Self {
            state: TankState::new(seed),
            queued_actions: VecDeque::new(),
        }
    }

    pub fn from_parts(state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        Self {
            state,
            queued_actions: queued_actions.into(),
        }
    }

    pub fn queued_actions(&self) -> Vec<PlayerAction> {
        self.queued_actions.iter().cloned().collect()
    }

    fn step_one_hour(&mut self) -> Result<(), SimError> {
        while let Some(action) = self.queued_actions.pop_front() {
            self.process_action(action);
        }

        self.state.environment.hour_of_day = (self.state.environment.hour_of_day + 1) % 24;
        if self.state.environment.hour_of_day == 0 {
            self.state.environment.day += 1;
        }

        enforce_invariants(&mut self.state)
    }

    fn process_action(&mut self, action: PlayerAction) {
        match action {
            PlayerAction::Feed { grams } => {
                self.state.detritus.particulate_organics_g_total += grams;
                self.push_event(
                    EventSeverity::Info,
                    EventKind::CycleProgressing,
                    vec![EventCause::Overfeeding],
                    format!("Queued feed processed: {grams:.2} g"),
                );
            }
            PlayerAction::WaterChangePercent { percent, .. } => {
                let retention = 1.0 - (percent / 100.0);
                self.state.water.ammonia_total_mg_n_total *= retention;
                self.state.water.nitrite_mg_n_total *= retention;
                self.state.water.nitrate_mg_n_total *= retention;
                self.state.water.phosphate_mg_p_total *= retention;
                self.state.water.dissolved_inorganic_carbon_mg_c_total *= retention;
                self.state.water.dissolved_organic_carbon_mg_c_total *= retention;
                self.state.water.dissolved_organic_nitrogen_mg_n_total *= retention;
                self.state.water.alkalinity_meq_total *= retention;
                self.state.water.calcium_mg_total *= retention;
                self.state.water.magnesium_mg_total *= retention;
                self.state.water.sodium_mg_total *= retention;
                self.state.water.potassium_mg_total *= retention;
                self.state.water.bicarbonate_mg_total *= retention;
                self.state.water.chloride_mg_total *= retention;
                self.state.water.sulfate_mg_total *= retention;
                self.push_event(
                    EventSeverity::Info,
                    EventKind::StabilityImproving,
                    vec![EventCause::WaterChange],
                    format!("Queued water change processed: {percent:.1}%"),
                );
            }
            PlayerAction::TrimPlants { fraction } => {
                for plant in &mut self.state.plant_guilds {
                    plant.biomass_g *= 1.0 - fraction;
                }
            }
            PlayerAction::SiphonDetritus { fraction } => {
                self.state.detritus.particulate_organics_g_total *= 1.0 - fraction;
                self.state.detritus.fine_detritus_g_total *= 1.0 - fraction;
            }
            PlayerAction::CleanFilter { intensity } => {
                self.state.hardware.filter.cleanliness_index = 1.0;
                self.state.filter_state.biofilter_maturity_index *= 1.0 - (intensity * 0.5);
                self.state.filter_state.clogging_index *= 1.0 - intensity;
                self.push_event(
                    EventSeverity::Warning,
                    EventKind::FilterCleaningSetback,
                    vec![EventCause::FilterMaintenance],
                    format!("Filter cleaned at intensity {intensity:.2}"),
                );
            }
            PlayerAction::AddShrimp { count } => {
                self.state.animal.adults_count =
                    self.state.animal.adults_count.saturating_add(count);
            }
            PlayerAction::RemoveShrimp { count } => {
                self.state.animal.adults_count =
                    self.state.animal.adults_count.saturating_sub(count);
            }
            PlayerAction::ChangePhotoperiod { hours } => {
                self.state.hardware.light.photoperiod_hours = hours;
            }
            PlayerAction::ChangeLightIntensity { intensity_index } => {
                self.state.hardware.light.intensity_index = intensity_index;
            }
            PlayerAction::ChangeHeaterSetpoint { setpoint_c } => {
                self.state.hardware.heater.setpoint_c = setpoint_c;
            }
            PlayerAction::ChangeAmbientTemperature { target_c } => {
                self.state.environment.ambient_temp_c = target_c;
            }
            PlayerAction::ChangeAeration { enabled, intensity } => {
                self.state.hardware.aeration.enabled = enabled;
                self.state.hardware.aeration.intensity = intensity;
            }
        }
    }

    fn push_event(
        &mut self,
        severity: EventSeverity,
        kind: EventKind,
        cause_codes: Vec<EventCause>,
        summary: String,
    ) {
        self.state.event_log.push(SimEvent::new(
            self.state.environment.day,
            self.state.environment.hour_of_day,
            severity,
            kind,
            cause_codes,
            summary,
        ));
    }
}

impl SimulationEngine for Engine {
    fn apply_action(&mut self, action: PlayerAction) -> Result<(), SimError> {
        action.validate()?;
        self.queued_actions.push_back(action);
        Ok(())
    }

    fn step_hours(&mut self, hours: u32) -> Result<(), SimError> {
        for _ in 0..hours {
            self.step_one_hour()?;
        }
        enforce_invariants(&mut self.state)
    }

    fn snapshot(&self) -> TankSnapshot {
        TankSnapshot::from_state(&self.state)
    }

    fn full_state(&self) -> &TankState {
        &self.state
    }
}
