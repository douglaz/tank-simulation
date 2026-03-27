use std::collections::VecDeque;

use crate::{
    invariants::enforce_invariants,
    rng::SimSeed,
    systems,
    types::{
        BudgetLedger, BudgetTotals, EventCause, EventKind, EventSeverity, PlayerAction, SimError,
        SimEvent, TankSnapshot, TankState, TickBudgetRecord,
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
    budget_ledger: Option<BudgetLedger>,
}

impl Engine {
    pub fn new(seed: SimSeed) -> Self {
        Self {
            state: TankState::new(seed),
            queued_actions: VecDeque::new(),
            budget_ledger: None,
        }
    }

    pub fn from_parts(state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        Self {
            state,
            queued_actions: queued_actions.into(),
            budget_ledger: None,
        }
    }

    pub fn queued_actions(&self) -> Vec<PlayerAction> {
        self.queued_actions.iter().cloned().collect()
    }

    pub fn enable_budget_tracking(&mut self) {
        if self.budget_ledger.is_none() {
            self.budget_ledger = Some(BudgetLedger::default());
        }
    }

    pub fn disable_budget_tracking(&mut self) {
        self.budget_ledger = None;
    }

    pub fn budget_ledger(&self) -> Option<&BudgetLedger> {
        self.budget_ledger.as_ref()
    }

    fn step_one_hour(&mut self) -> Result<(), SimError> {
        let mut tick = self
            .budget_ledger
            .as_ref()
            .map(|ledger| TickBudgetRecord::start(&self.state, ledger.ticks.len()));
        let actions_slice: Vec<_> = self.queued_actions.iter().cloned().collect();
        // Validate all queued actions (covers from_parts callers that bypass apply_action).
        let mut available_shrimp = self.state.animal.adults_count as i64;
        for action in &actions_slice {
            action.validate()?;
            // State-aware check: RemoveShrimp must not exceed available adults.
            match action {
                PlayerAction::RemoveShrimp { count } => {
                    if (*count as i64) > available_shrimp {
                        return Err(SimError::ShrimpRemovalExceedsAvailable {
                            requested: *count,
                            available: available_shrimp.max(0) as u32,
                        });
                    }
                    available_shrimp -= *count as i64;
                }
                PlayerAction::AddShrimp { count } => {
                    available_shrimp += *count as i64;
                }
                _ => {}
            }
        }
        systems::water_change::validate_water_changes(&self.state, &actions_slice)?;

        while let Some(action) = self.queued_actions.pop_front() {
            let label = action_budget_label(&action);
            self.maybe_record_stage(&mut tick, label, move |engine| {
                engine.process_action(action);
            });
        }

        // Step 4: update water temperature from ambient and heater
        self.maybe_record_stage(&mut tick, "system:temperature", |engine| {
            systems::temperature::step_temperature(&mut engine.state);
        });

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
        self.maybe_record_stage(&mut tick, "system:nitrogen_cycle", |engine| {
            let _ = systems::nitrogen_cycle::step_nitrogen_cycle(&mut engine.state);
        });

        // Step 9: update DIC, alkalinity, and pH.
        self.maybe_record_stage(&mut tick, "system:chemistry", |engine| {
            systems::chemistry::step_hourly_chemistry(&mut engine.state, light_on);
        });

        // Step 10 / 11-partial: update dissolved oxygen with background respiration and
        // light-driven photosynthetic support from existing biomass.
        self.maybe_record_stage(&mut tick, "system:dissolved_oxygen", |engine| {
            systems::dissolved_oxygen::step_dissolved_oxygen(&mut engine.state, light_on);
        });

        // Step 12: hourly shrimp stress accumulation.
        self.maybe_record_stage(&mut tick, "system:shrimp_stress", |engine| {
            systems::shrimp::step_hourly_shrimp_stress(&mut engine.state);
        });

        // Step 13: emit threshold-based chemistry warnings.
        self.maybe_record_stage(&mut tick, "system:hourly_events", |engine| {
            systems::events::emit_hourly_threshold_events(&mut engine.state);
        });

        self.state.environment.hour_of_day = (self.state.environment.hour_of_day + 1) % 24;
        if self.state.environment.hour_of_day == 0 {
            self.state.environment.day += 1;
            // Daily pipeline
            self.run_daily_update(&mut tick);
        }

        self.maybe_record_stage(&mut tick, "system:invariants", |engine| {
            enforce_invariants(&mut engine.state)
        })?;

        if let (Some(ledger), Some(tick)) = (self.budget_ledger.as_mut(), tick) {
            ledger.push_tick(tick);
        }

        Ok(())
    }

    fn run_daily_update(&mut self, tick: &mut Option<TickBudgetRecord>) {
        // Daily pipeline: plants → algae → microfauna → stability → shrimp → biofilter
        self.maybe_record_stage(tick, "system:daily_plants", |engine| {
            systems::plant_growth::step_daily_plants(&mut engine.state);
        });
        self.maybe_record_stage(tick, "system:daily_algae", |engine| {
            systems::algae_growth::step_daily_algae(&mut engine.state);
        });
        self.maybe_record_stage(tick, "system:daily_microfauna", |engine| {
            systems::microfauna::step_daily_microfauna(&mut engine.state);
        });

        // Update stability metrics before shrimp so that same-day chemistry
        // swings (water changes, temperature shifts) are reflected in the
        // instability_index that shrimp condition/mortality reads.
        self.maybe_record_stage(tick, "system:stability_tracker", |engine| {
            systems::shrimp::update_stability_tracker(&mut engine.state);
        });

        self.maybe_record_stage(tick, "system:daily_shrimp", |engine| {
            systems::shrimp::step_daily_shrimp(&mut engine.state);
        });
        self.maybe_record_stage(tick, "system:daily_filter_clogging", |engine| {
            systems::nitrogen_cycle::update_daily_filter_clogging(&mut engine.state);
        });

        // Biofilter maturity summary update
        let maturity_delta =
            self.maybe_record_stage(tick, "system:daily_biofilter_maturity", |engine| {
                systems::nitrogen_cycle::update_daily_biofilter_maturity(&mut engine.state)
            });
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

    fn maybe_record_stage<F, R>(
        &mut self,
        tick: &mut Option<TickBudgetRecord>,
        label: &'static str,
        stage: F,
    ) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let before = tick.as_ref().map(|_| BudgetTotals::from_state(&self.state));
        let result = stage(self);
        if let (Some(tick), Some(before)) = (tick.as_mut(), before) {
            let after = BudgetTotals::from_state(&self.state);
            tick.record_stage(label, before, after);
        }
        result
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
                    systems::water_change::apply_water_change(&mut self.state, percent, &profile);
                    self.push_event(
                        EventSeverity::Info,
                        EventKind::StabilityImproving,
                        vec![EventCause::WaterChange],
                        format!("Water change processed: {percent:.1}% with {source_profile_id}"),
                    );
                }
            }
            PlayerAction::TrimPlants { fraction } => {
                let mut trimmed_biomass_g = 0.0;
                for plant in &mut self.state.plant_guilds {
                    let trimmed = plant.biomass_g * fraction;
                    plant.biomass_g -= trimmed;
                    trimmed_biomass_g += trimmed;
                }
                self.state.detritus.fine_detritus_g_total += trimmed_biomass_g;
            }
            PlayerAction::SiphonDetritus { fraction } => {
                self.state.detritus.particulate_organics_g_total *= 1.0 - fraction;
                self.state.detritus.fine_detritus_g_total *= 1.0 - fraction;
            }
            PlayerAction::CleanFilter { intensity } => {
                let current_cleanliness =
                    self.state.hardware.filter.cleanliness_index.clamp(0.0, 1.0);
                self.state.hardware.filter.cleanliness_index = (current_cleanliness
                    + ((1.0 - current_cleanliness) * intensity))
                    .clamp(0.0, 1.0);
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
                // Preserve berried_females_count <= adults_count (also trims egg cohorts)
                self.state.animal.clamp_berried_to_adults();
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

        // State-aware validation for RemoveShrimp
        if let PlayerAction::RemoveShrimp { count } = &action {
            let mut available = self.state.animal.adults_count as i64;
            for queued in &self.queued_actions {
                match queued {
                    PlayerAction::RemoveShrimp { count: c } => available -= *c as i64,
                    PlayerAction::AddShrimp { count: c } => available += *c as i64,
                    _ => {}
                }
            }
            if (*count as i64) > available {
                return Err(SimError::ShrimpRemovalExceedsAvailable {
                    requested: *count,
                    available: available.max(0) as u32,
                });
            }
        }

        if let PlayerAction::WaterChangePercent {
            source_profile_id, ..
        } = &action
        {
            systems::water_change::validate_source_profile(&self.state, source_profile_id)?;
        }

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

fn action_budget_label(action: &PlayerAction) -> &'static str {
    match action {
        PlayerAction::Feed { .. } => "action:feed",
        PlayerAction::WaterChangePercent { .. } => "action:water_change",
        PlayerAction::TrimPlants { .. } => "action:trim_plants",
        PlayerAction::SiphonDetritus { .. } => "action:siphon_detritus",
        PlayerAction::CleanFilter { .. } => "action:clean_filter",
        PlayerAction::AddShrimp { .. } => "action:add_shrimp",
        PlayerAction::RemoveShrimp { .. } => "action:remove_shrimp",
        PlayerAction::ChangePhotoperiod { .. } => "action:change_photoperiod",
        PlayerAction::ChangeLightIntensity { .. } => "action:change_light_intensity",
        PlayerAction::ChangeHeaterSetpoint { .. } => "action:change_heater_setpoint",
        PlayerAction::ChangeAmbientTemperature { .. } => "action:change_ambient_temperature",
        PlayerAction::ChangeAeration { .. } => "action:change_aeration",
    }
}
