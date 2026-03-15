use std::collections::VecDeque;

use crate::{
    invariants::enforce_invariants,
    rng::SimSeed,
    systems,
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
        let actions_slice: Vec<_> = self.queued_actions.iter().cloned().collect();
        systems::water_change::validate_water_changes(&self.state, &actions_slice)?;

        while let Some(action) = self.queued_actions.pop_front() {
            self.process_action(action);
        }

        // Step 4: update water temperature from ambient and heater
        systems::temperature::step_temperature(&mut self.state);

        // Step 5: compute light state for the current hour.
        let light_on = self.state.hardware.light.enabled
            && systems::light::is_light_on(
                self.state.environment.hour_of_day,
                self.state.hardware.light.photoperiod_hours,
            );

        // Step 6-8: nitrogen cycle phase (feed leaching, detritus breakdown,
        // mineralization, nitrification, guild growth/decay).
        // This runs between light-state resolution and chemistry/DO/event phases.
        // Nitrification O2 consumption and alkalinity depletion are handled
        // internally by the nitrogen cycle system.
        let _nc_output = systems::nitrogen_cycle::step_nitrogen_cycle(&mut self.state);

        // Step 9: update DIC, alkalinity, and pH.
        systems::chemistry::step_hourly_chemistry(&mut self.state, light_on);

        // Step 10 / 11-partial: update dissolved oxygen with background respiration and
        // light-driven photosynthetic support from existing biomass.
        systems::dissolved_oxygen::step_dissolved_oxygen(&mut self.state, light_on);

        // Step 13: emit threshold-based chemistry warnings.
        systems::events::emit_hourly_threshold_events(&mut self.state);

        self.state.environment.hour_of_day = (self.state.environment.hour_of_day + 1) % 24;
        if self.state.environment.hour_of_day == 0 {
            self.state.environment.day += 1;
            // Daily pipeline
            self.run_daily_update();
        }

        enforce_invariants(&mut self.state)
    }

    fn run_daily_update(&mut self) {
        // Step 6 (daily): biofilter maturity summary update
        let maturity_delta =
            systems::nitrogen_cycle::update_daily_biofilter_maturity(&mut self.state);
        if maturity_delta > 0.005 {
            self.push_event(
                EventSeverity::Info,
                EventKind::BiofilmMaturityIncrease,
                vec![EventCause::BiofilterImmature],
                format!(
                    "Biofilter maturity increased to {:.3}",
                    self.state.filter_state.biofilter_maturity_index
                ),
            );
        }
        // Emit CycleProgressing when maturity is actively growing
        let current_maturity = self.state.filter_state.biofilter_maturity_index;
        if maturity_delta > 0.001 && current_maturity < 0.9 {
            systems::events::emit_once_per_day_pub(
                &mut self.state,
                EventSeverity::Info,
                EventKind::CycleProgressing,
                vec![EventCause::BiofilterImmature],
                format!("Nitrogen cycle progressing, maturity {current_maturity:.3}"),
            );
        }
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
            PlayerAction::WaterChangePercent {
                percent,
                source_profile_id,
            } => {
                if percent <= 0.0 {
                    return;
                }
                // Profile was pre-validated; look it up (guaranteed to exist).
                let profile = self
                    .state
                    .source_water_catalog
                    .get(&source_profile_id)
                    .cloned();
                if let Some(profile) = profile {
                    systems::water_change::apply_water_change(
                        &mut self.state,
                        percent,
                        &profile,
                    );
                    self.push_event(
                        EventSeverity::Info,
                        EventKind::StabilityImproving,
                        vec![EventCause::WaterChange],
                        format!("Water change processed: {percent:.1}% with {source_profile_id}"),
                    );
                }
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
                // Proportional setback in active nitrifier and decomposer biomass
                let setback = intensity * 0.5;
                self.state.microbe.decomposer_biomass_g *= 1.0 - setback;
                self.state.microbe.ammonia_oxidizer_biomass_g *= 1.0 - setback;
                self.state.microbe.nitrite_oxidizer_biomass_g *= 1.0 - setback;
                self.state.microbe.comammox_biomass_g *= 1.0 - setback;
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
