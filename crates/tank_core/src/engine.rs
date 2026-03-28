use std::collections::VecDeque;

use crate::{
    invariants::enforce_invariants,
    rng::SimSeed,
    systems,
    tracing::{PoolSnapshot, SimTracer, SystemTraceEntry, TickTraceBuilder, Verbosity},
    types::{
        live_biomass_carbon_mg, live_biomass_nitrogen_mg, BudgetDelta, BudgetEntry, BudgetLedger,
        BudgetRecordingKind, BudgetSnapshot, ElementBudget, EventCause, EventKind, EventSeverity,
        PlayerAction, SimError, SimEvent, TankSnapshot, TankState, TickBudgetRecord,
    },
};

const BUDGET_GUARD_TOLERANCE_MG: f64 = 1e-6;

pub trait SimulationEngine {
    fn apply_action(&mut self, action: PlayerAction) -> Result<(), SimError>;
    fn step_hours(&mut self, hours: u32) -> Result<(), SimError>;
    fn snapshot(&self) -> TankSnapshot;
    fn full_state(&self) -> &TankState;
}

pub struct Engine {
    state: TankState,
    queued_actions: VecDeque<PlayerAction>,
    budget_ledger: Option<BudgetLedger>,
    tracer: Option<SimTracer>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("state", &self.state)
            .field("queued_actions", &self.queued_actions)
            .field("budget_ledger", &self.budget_ledger)
            .field("tracer", &self.tracer)
            .finish()
    }
}

/// Cloned engines intentionally start with tracing disabled.
///
/// `SimTracer` can own non-cloneable external sinks, so clones keep the
/// simulation state, queued actions, and budget ledger but drop the tracer.
/// Re-enable tracing on scenario forks that still need diagnostics.
impl Clone for Engine {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            queued_actions: self.queued_actions.clone(),
            budget_ledger: self.budget_ledger.clone(),
            tracer: None,
        }
    }
}

impl PartialEq for Engine {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && self.queued_actions == other.queued_actions
            && self.budget_ledger == other.budget_ledger
    }
}

/// Per-tick working state for both budget tracking and tracing.
struct TickContext {
    budget: Option<TickBudgetRecord>,
    trace: Option<TickTraceBuilder>,
}

struct StageTrace {
    enabled: bool,
    notes: Vec<String>,
}

impl StageTrace {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            notes: Vec::new(),
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn note(&mut self, note: impl Into<String>) {
        if self.enabled {
            self.notes.push(note.into());
        }
    }

    fn into_notes(self) -> Vec<String> {
        if self.enabled {
            self.notes
        } else {
            Vec::new()
        }
    }
}

impl Engine {
    pub fn new(seed: SimSeed) -> Self {
        Self {
            state: TankState::new(seed),
            queued_actions: VecDeque::new(),
            budget_ledger: None,
            tracer: None,
        }
    }

    pub fn from_parts(state: TankState, queued_actions: Vec<PlayerAction>) -> Self {
        Self {
            state,
            queued_actions: queued_actions.into(),
            budget_ledger: None,
            tracer: None,
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

    /// Enable structured tracing with the given tracer configuration.
    ///
    /// Engine clones do not inherit this tracer because `SimTracer` may own a
    /// non-cloneable sink. Re-enable tracing on cloned engines explicitly when
    /// forked scenarios still need trace output.
    pub fn enable_tracing(&mut self, tracer: SimTracer) {
        self.tracer = Some(tracer);
    }

    /// Disable tracing, returning the tracer with all recorded data.
    pub fn disable_tracing(&mut self) -> Option<SimTracer> {
        self.tracer.take()
    }

    /// Access the active tracer (if any).
    pub fn tracer(&self) -> Option<&SimTracer> {
        self.tracer.as_ref()
    }

    /// Mutably access the active tracer (if any).
    pub fn tracer_mut(&mut self) -> Option<&mut SimTracer> {
        self.tracer.as_mut()
    }

    fn step_one_hour(&mut self) -> Result<(), SimError> {
        let mut ctx = TickContext {
            budget: self
                .budget_ledger
                .as_ref()
                .map(|ledger| TickBudgetRecord::start(&self.state, ledger.ticks.len())),
            trace: self
                .tracer
                .as_ref()
                .filter(|tracer| tracer.verbosity() > Verbosity::Off)
                .map(|tracer| tracer.begin_tick(&self.state)),
        };

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
            self.maybe_record_stage_with_explicit_budget(
                &mut ctx,
                label,
                move |engine, _stage_trace, tracking| ((), engine.process_action(action, tracking)),
            );
        }

        // Step 4: update water temperature from ambient and heater
        self.maybe_record_stage(&mut ctx, "system:temperature", |engine, _stage_trace| {
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
        self.maybe_record_stage(&mut ctx, "system:nitrogen_cycle", |engine, _stage_trace| {
            let _ = systems::nitrogen_cycle::step_nitrogen_cycle(&mut engine.state);
        });

        // Step 9: update DIC, alkalinity, and pH.
        self.maybe_record_stage_with_explicit_budget(
            &mut ctx,
            "system:chemistry",
            move |engine, stage_trace, tracking| {
                let ph_before = engine.state.water.ph;
                let dic_before = engine.state.water.dissolved_inorganic_carbon_mg_c_total;
                let alkalinity_before = engine.state.water.alkalinity_meq_total;
                let delta = if tracking {
                    Some(systems::chemistry::step_hourly_chemistry_with_budget(
                        &mut engine.state,
                        light_on,
                    ))
                } else {
                    systems::chemistry::step_hourly_chemistry(&mut engine.state, light_on);
                    None
                };
                if stage_trace.is_enabled() {
                    stage_trace.note(format!("chemistry.light_on={light_on}"));
                    stage_trace.note(format!("chemistry.ph.before={ph_before:.6}"));
                    stage_trace.note(format!("chemistry.ph.after={:.6}", engine.state.water.ph));
                    stage_trace.note(format!("chemistry.dic_mg_c.before={dic_before:.6}"));
                    stage_trace.note(format!(
                        "chemistry.dic_mg_c.after={:.6}",
                        engine.state.water.dissolved_inorganic_carbon_mg_c_total
                    ));
                    stage_trace.note(format!(
                        "chemistry.alkalinity_meq.before={alkalinity_before:.6}"
                    ));
                    stage_trace.note(format!(
                        "chemistry.alkalinity_meq.after={:.6}",
                        engine.state.water.alkalinity_meq_total
                    ));
                }
                ((), delta)
            },
        );

        // Step 10 / 11-partial: update dissolved oxygen with background respiration and
        // light-driven photosynthetic support from existing biomass.
        self.maybe_record_stage_with_explicit_budget(
            &mut ctx,
            "system:dissolved_oxygen",
            move |engine, stage_trace, tracking| {
                let oxygen_before = engine.state.water.dissolved_oxygen_mg_total;
                let oxygen_kla = stage_trace
                    .is_enabled()
                    .then(|| systems::dissolved_oxygen::compute_o2_kla(&engine.state));
                let delta = if tracking {
                    Some(
                        systems::dissolved_oxygen::step_dissolved_oxygen_with_budget(
                            &mut engine.state,
                            light_on,
                        ),
                    )
                } else {
                    systems::dissolved_oxygen::step_dissolved_oxygen(&mut engine.state, light_on);
                    None
                };
                if let Some(oxygen_kla) = oxygen_kla {
                    stage_trace.note(format!("dissolved_oxygen.light_on={light_on}"));
                    stage_trace.note(format!("dissolved_oxygen.kla={oxygen_kla:.6}"));
                    stage_trace.note(format!("dissolved_oxygen.do_mg.before={oxygen_before:.6}"));
                    stage_trace.note(format!(
                        "dissolved_oxygen.do_mg.after={:.6}",
                        engine.state.water.dissolved_oxygen_mg_total
                    ));
                }
                ((), delta)
            },
        );

        // Step 12: hourly shrimp stress accumulation.
        self.maybe_record_stage(&mut ctx, "system:shrimp_stress", |engine, _stage_trace| {
            systems::shrimp::step_hourly_shrimp_stress(&mut engine.state);
        });

        // Step 13: emit threshold-based chemistry warnings.
        self.maybe_record_stage(&mut ctx, "system:hourly_events", |engine, _stage_trace| {
            systems::events::emit_hourly_threshold_events(&mut engine.state);
        });

        self.state.environment.hour_of_day = (self.state.environment.hour_of_day + 1) % 24;
        if self.state.environment.hour_of_day == 0 {
            self.state.environment.day += 1;
            // Daily pipeline
            self.run_daily_update(&mut ctx);
        }

        self.maybe_record_stage(&mut ctx, "system:invariants", |engine, _stage_trace| {
            enforce_invariants(&mut engine.state)
        })?;

        if let Some(tick_record) = ctx.budget.as_ref() {
            enforce_tracked_tick_budget_guard(tick_record)?;
        }

        if let (Some(ledger), Some(tick)) = (self.budget_ledger.as_mut(), ctx.budget) {
            ledger.push_tick(tick);
        }

        if let (Some(tracer), Some(builder)) = (self.tracer.as_mut(), ctx.trace) {
            tracer.finish_tick(builder);
        }

        Ok(())
    }

    fn run_daily_update(&mut self, ctx: &mut TickContext) {
        // Daily pipeline: plants → algae → microfauna → stability → shrimp → biofilter
        self.maybe_record_stage(ctx, "system:daily_plants", |engine, _stage_trace| {
            systems::plant_growth::step_daily_plants(&mut engine.state);
        });
        self.maybe_record_stage(ctx, "system:daily_algae", |engine, _stage_trace| {
            systems::algae_growth::step_daily_algae(&mut engine.state);
        });
        self.maybe_record_stage(ctx, "system:daily_microfauna", |engine, _stage_trace| {
            systems::microfauna::step_daily_microfauna(&mut engine.state);
        });

        // Update stability metrics before shrimp so that same-day chemistry
        // swings (water changes, temperature shifts) are reflected in the
        // instability_index that shrimp condition/mortality reads.
        self.maybe_record_stage(ctx, "system:stability_tracker", |engine, _stage_trace| {
            systems::shrimp::update_stability_tracker(&mut engine.state);
        });

        self.maybe_record_stage(ctx, "system:daily_shrimp", |engine, _stage_trace| {
            systems::shrimp::step_daily_shrimp(&mut engine.state);
        });
        self.maybe_record_stage(
            ctx,
            "system:daily_filter_clogging",
            |engine, _stage_trace| {
                systems::nitrogen_cycle::update_daily_filter_clogging(&mut engine.state);
            },
        );

        // Biofilter maturity summary update
        self.maybe_record_stage(ctx, "system:daily_biofilter_maturity", |engine, stage_trace| {
                let maturity_before = engine.state.filter_state.biofilter_maturity_index;
                let total_nitrifier_g = engine.state.microbe.ammonia_oxidizer_biomass_g
                    + engine.state.microbe.nitrite_oxidizer_biomass_g
                    + engine.state.microbe.comammox_biomass_g;
                let maturity_delta =
                    systems::nitrogen_cycle::update_daily_biofilter_maturity(&mut engine.state);
                let biofilm_maturity_increase_emitted = maturity_delta > 0.005;
                if biofilm_maturity_increase_emitted {
                    engine.push_event(
                        EventSeverity::Info,
                        EventKind::BiofilmMaturityIncrease,
                        vec![EventCause::BiofilterImmature],
                        format!(
                            "Biofilter maturity increased to {:.3}",
                            engine.state.filter_state.biofilter_maturity_index
                        ),
                    );
                }
                let current_maturity = engine.state.filter_state.biofilter_maturity_index;
                let cycle_progressing_emitted =
                    maturity_delta > 0.001 && current_maturity < 0.9;
                if cycle_progressing_emitted {
                    systems::events::emit_once_per_day_pub(
                        &mut engine.state,
                        EventSeverity::Info,
                        EventKind::CycleProgressing,
                        vec![EventCause::BiofilterImmature],
                        format!("Nitrogen cycle progressing, maturity {current_maturity:.3}"),
                    );
                }
                if stage_trace.is_enabled() {
                    stage_trace.note(format!("biofilter_maturity.before={maturity_before:.6}"));
                    stage_trace
                        .note(format!("biofilter_maturity.total_nitrifier_g={total_nitrifier_g:.6}"));
                    stage_trace.note(format!("biofilter_maturity.delta={maturity_delta:.6}"));
                    stage_trace.note(format!("biofilter_maturity.after={current_maturity:.6}"));
                    stage_trace.note(format!(
                        "biofilter_maturity.emitted.biofilm_maturity_increase={biofilm_maturity_increase_emitted}"
                    ));
                    stage_trace.note(format!(
                        "biofilter_maturity.emitted.cycle_progressing={cycle_progressing_emitted}"
                    ));
                }
                maturity_delta
            });
    }

    fn maybe_record_stage<F, R>(
        &mut self,
        ctx: &mut TickContext,
        label: &'static str,
        stage: F,
    ) -> R
    where
        F: FnOnce(&mut Self, &mut StageTrace) -> R,
    {
        let budget_before = ctx
            .budget
            .as_ref()
            .map(|_| BudgetSnapshot::from_state(&self.state));
        let trace_before = ctx
            .trace
            .as_ref()
            .filter(|t| t.verbosity >= Verbosity::Detail)
            .map(|_| PoolSnapshot::capture(&self.state));
        let event_count_before = ctx.trace.as_ref().map(|_| self.state.event_log.len());
        let mut stage_trace = StageTrace::new(matches!(
            ctx.trace.as_ref(),
            Some(trace) if trace.verbosity >= Verbosity::Trace
        ));

        let result = stage(self, &mut stage_trace);

        if let (Some(tick), Some(before)) = (ctx.budget.as_mut(), budget_before) {
            let after = BudgetSnapshot::from_state(&self.state);
            let delta = BudgetDelta::from_snapshots(&before, &after);
            tick.record_stage(
                label,
                before.totals,
                after.totals,
                delta,
                BudgetRecordingKind::Snapshot,
            );
        }

        let notes = stage_trace.into_notes();
        if let Some(trace) = ctx.trace.as_mut() {
            let pool_deltas = if let Some(before) = trace_before {
                let after = PoolSnapshot::capture(&self.state);
                before.deltas_to(&after)
            } else {
                Vec::new()
            };
            let events_generated = event_count_before
                .map(|before| self.state.event_log.len().saturating_sub(before))
                .unwrap_or(0);
            trace.entries.push(SystemTraceEntry {
                system: label.to_owned(),
                pool_deltas,
                notes,
                events_generated,
            });
        }

        result
    }

    fn maybe_record_stage_with_explicit_budget<F, R>(
        &mut self,
        ctx: &mut TickContext,
        label: &'static str,
        stage: F,
    ) -> R
    where
        F: FnOnce(&mut Self, &mut StageTrace, bool) -> (R, Option<BudgetDelta>),
    {
        let budget_before = ctx
            .budget
            .as_ref()
            .map(|_| BudgetSnapshot::from_state(&self.state));
        let trace_before = ctx
            .trace
            .as_ref()
            .filter(|t| t.verbosity >= Verbosity::Detail)
            .map(|_| PoolSnapshot::capture(&self.state));
        let event_count_before = ctx.trace.as_ref().map(|_| self.state.event_log.len());
        let mut stage_trace = StageTrace::new(matches!(
            ctx.trace.as_ref(),
            Some(trace) if trace.verbosity >= Verbosity::Trace
        ));

        let (result, explicit_delta) = stage(self, &mut stage_trace, ctx.budget.is_some());

        if let (Some(tick), Some(before)) = (ctx.budget.as_mut(), budget_before) {
            let after = BudgetSnapshot::from_state(&self.state);
            let recording_kind = if explicit_delta.is_some() {
                BudgetRecordingKind::Explicit
            } else {
                BudgetRecordingKind::Snapshot
            };
            let delta =
                explicit_delta.unwrap_or_else(|| BudgetDelta::from_snapshots(&before, &after));
            tick.record_stage(label, before.totals, after.totals, delta, recording_kind);
        }

        let notes = stage_trace.into_notes();
        if let Some(trace) = ctx.trace.as_mut() {
            let pool_deltas = if let Some(before) = trace_before {
                let after = PoolSnapshot::capture(&self.state);
                before.deltas_to(&after)
            } else {
                Vec::new()
            };
            let events_generated = event_count_before
                .map(|before| self.state.event_log.len().saturating_sub(before))
                .unwrap_or(0);
            trace.entries.push(SystemTraceEntry {
                system: label.to_owned(),
                pool_deltas,
                notes,
                events_generated,
            });
        }

        result
    }

    fn process_action(
        &mut self,
        action: PlayerAction,
        tracking_budget: bool,
    ) -> Option<BudgetDelta> {
        match action {
            PlayerAction::Feed { grams } => {
                self.state.detritus.particulate_organics_g_total += grams;
                self.push_event(
                    EventSeverity::Info,
                    EventKind::CycleProgressing,
                    vec![EventCause::Overfeeding],
                    format!("Queued feed processed: {grams:.2} g"),
                );
                None
            }
            PlayerAction::WaterChangePercent {
                percent,
                source_profile_id,
            } => {
                if percent <= 0.0 {
                    return None;
                }
                // Profile was pre-validated; look it up (guaranteed to exist).
                let profile = self
                    .state
                    .source_water_catalog
                    .get(&source_profile_id)
                    .cloned();
                if let Some(profile) = profile {
                    let delta = if tracking_budget {
                        Some(systems::water_change::apply_water_change_with_budget(
                            &mut self.state,
                            percent,
                            &profile,
                        ))
                    } else {
                        systems::water_change::apply_water_change(
                            &mut self.state,
                            percent,
                            &profile,
                        );
                        None
                    };
                    self.push_event(
                        EventSeverity::Info,
                        EventKind::StabilityImproving,
                        vec![EventCause::WaterChange],
                        format!("Water change processed: {percent:.1}% with {source_profile_id}"),
                    );
                    delta
                } else {
                    None
                }
            }
            PlayerAction::TrimPlants { fraction } => {
                let mut trimmed_biomass_g = 0.0;
                for plant in &mut self.state.plant_guilds {
                    let trimmed = plant.biomass_g * fraction;
                    plant.biomass_g -= trimmed;
                    trimmed_biomass_g += trimmed;
                }
                self.state.detritus.fine_detritus_g_total +=
                    systems::plant_growth::plant_detrital_mass_g(
                        trimmed_biomass_g,
                        self.state.process_params.feed_n_to_c_ratio,
                    );
                None
            }
            PlayerAction::SiphonDetritus { fraction } => {
                self.state.detritus.particulate_organics_g_total *= 1.0 - fraction;
                self.state.detritus.fine_detritus_g_total *= 1.0 - fraction;
                None
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
                let removed_decomposer = self.state.microbe.decomposer_biomass_g * setback;
                let removed_aob = self.state.microbe.ammonia_oxidizer_biomass_g * setback;
                let removed_nob = self.state.microbe.nitrite_oxidizer_biomass_g * setback;
                let removed_comammox = self.state.microbe.comammox_biomass_g * setback;
                self.state.microbe.decomposer_biomass_g -= removed_decomposer;
                self.state.microbe.ammonia_oxidizer_biomass_g -= removed_aob;
                self.state.microbe.nitrite_oxidizer_biomass_g -= removed_nob;
                self.state.microbe.comammox_biomass_g -= removed_comammox;
                route_live_biomass_to_dissolved_organics(
                    &mut self.state,
                    removed_decomposer + removed_aob + removed_nob + removed_comammox,
                );
                self.push_event(
                    EventSeverity::Warning,
                    EventKind::FilterCleaningSetback,
                    vec![EventCause::FilterMaintenance],
                    format!("Filter cleaned at intensity {intensity:.2}"),
                );
                None
            }
            PlayerAction::AddShrimp { count } => {
                self.state.animal.adults_count =
                    self.state.animal.adults_count.saturating_add(count);
                None
            }
            PlayerAction::RemoveShrimp { count } => {
                // Export proportional share of reserve with removed shrimp.
                let total = self.state.animal.adults_count + self.state.animal.juveniles_count;
                if total > 0 && self.state.animal.reserve_g > f64::EPSILON {
                    let removed_frac = f64::from(count.min(total)) / f64::from(total);
                    self.state.animal.reserve_g -= self.state.animal.reserve_g * removed_frac;
                }
                self.state.animal.adults_count =
                    self.state.animal.adults_count.saturating_sub(count);
                // Preserve berried_females_count <= adults_count (also trims egg cohorts)
                self.state.animal.clamp_berried_to_adults();
                None
            }
            PlayerAction::ChangePhotoperiod { hours } => {
                self.state.hardware.light.photoperiod_hours = hours;
                None
            }
            PlayerAction::ChangeLightIntensity { intensity_index } => {
                self.state.hardware.light.intensity_index = intensity_index;
                None
            }
            PlayerAction::ChangeHeaterSetpoint { setpoint_c } => {
                self.state.hardware.heater.setpoint_c = setpoint_c;
                None
            }
            PlayerAction::ChangeAmbientTemperature { target_c } => {
                self.state.environment.ambient_temp_c = target_c;
                None
            }
            PlayerAction::ChangeAeration { enabled, intensity } => {
                self.state.hardware.aeration.enabled = enabled;
                self.state.hardware.aeration.intensity = intensity;
                None
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
        enforce_invariants(&mut self.state)?;
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

fn enforce_tracked_tick_budget_guard(tick: &TickBudgetRecord) -> Result<(), SimError> {
    let nitrogen_residual_mg = nitrogen_guard_delta_mg(tick);
    if nitrogen_residual_mg.abs() > BUDGET_GUARD_TOLERANCE_MG {
        return Err(SimError::BudgetImbalance {
            element: "nitrogen",
            delta_mg: nitrogen_residual_mg,
            tick_index: tick.tick_index,
            day: tick.day,
            hour: tick.hour,
        });
    }

    let carbon_residual_mg = carbon_guard_delta_mg(tick);
    if carbon_residual_mg.abs() > BUDGET_GUARD_TOLERANCE_MG {
        return Err(SimError::BudgetImbalance {
            element: "carbon",
            delta_mg: carbon_residual_mg,
            tick_index: tick.tick_index,
            day: tick.day,
            hour: tick.hour,
        });
    }

    Ok(())
}

fn route_live_biomass_to_dissolved_organics(state: &mut TankState, biomass_g: f64) {
    if biomass_g <= f64::EPSILON {
        return;
    }

    let n_to_c_ratio = state.process_params.feed_n_to_c_ratio;
    state.water.dissolved_organic_nitrogen_mg_n_total +=
        live_biomass_nitrogen_mg(biomass_g, n_to_c_ratio);
    state.water.dissolved_organic_carbon_mg_c_total +=
        live_biomass_carbon_mg(biomass_g, n_to_c_ratio);
}

#[cfg_attr(not(test), allow(dead_code))]
fn tick_has_closed_system_nitrogen(tick: &TickBudgetRecord) -> bool {
    !tick
        .entries
        .iter()
        .any(|entry| entry_has_open_action_flux(entry.label.as_str(), entry.delta.nitrogen))
}

#[cfg_attr(not(test), allow(dead_code))]
fn tick_has_closed_system_carbon(tick: &TickBudgetRecord) -> bool {
    !tick
        .entries
        .iter()
        .any(|entry| entry_has_open_action_flux(entry.label.as_str(), entry.delta.carbon))
}

fn nitrogen_guard_delta_mg(tick: &TickBudgetRecord) -> f64 {
    tick.net_delta.nitrogen.net_mg() - open_action_flux_mg(tick, |entry| entry.delta.nitrogen)
}

fn carbon_guard_delta_mg(tick: &TickBudgetRecord) -> f64 {
    // The hourly chemistry system can opt into an explicit atmospheric DIC
    // shortcut. Subtract that known open-system exchange so the guard still
    // catches unrelated carbon leaks elsewhere in the same tick.
    tick.net_delta.carbon.net_mg()
        - open_action_flux_mg(tick, |entry| entry.delta.carbon)
        - chemistry_external_carbon_flux_mg(tick)
}

fn chemistry_external_carbon_flux_mg(tick: &TickBudgetRecord) -> f64 {
    tick.entries
        .iter()
        .filter(|entry| entry.label == "system:chemistry")
        .map(|entry| entry.delta.carbon.net_mg())
        .sum()
}

fn open_action_flux_mg<F>(tick: &TickBudgetRecord, budget: F) -> f64
where
    F: Fn(&BudgetEntry) -> ElementBudget,
{
    tick.entries
        .iter()
        .filter(|entry| is_open_system_action_label(entry.label.as_str()))
        .map(|entry| budget(entry).net_mg())
        .sum()
}

#[cfg_attr(not(test), allow(dead_code))]
fn entry_has_open_action_flux(label: &str, budget: ElementBudget) -> bool {
    is_open_system_action_label(label) && element_budget_has_flux(budget)
}

fn is_open_system_action_label(label: &str) -> bool {
    matches!(
        label,
        "action:feed"
            | "action:water_change"
            | "action:siphon_detritus"
            | "action:add_shrimp"
            | "action:remove_shrimp"
    )
}

#[cfg_attr(not(test), allow(dead_code))]
fn element_budget_has_flux(budget: ElementBudget) -> bool {
    budget.in_mg.abs() > BUDGET_GUARD_TOLERANCE_MG
        || budget.out_mg.abs() > BUDGET_GUARD_TOLERANCE_MG
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BudgetEntry, BudgetRecordingKind, BudgetTotals};

    fn synthetic_entry(label: &str, delta: BudgetDelta) -> BudgetEntry {
        BudgetEntry {
            label: label.to_owned(),
            delta,
            recording_kind: BudgetRecordingKind::Snapshot,
        }
    }

    fn synthetic_tick(entries: Vec<BudgetEntry>) -> TickBudgetRecord {
        synthetic_tick_with_net_delta(entries, BudgetDelta::default())
    }

    fn synthetic_tick_with_net_delta(
        entries: Vec<BudgetEntry>,
        net_delta: BudgetDelta,
    ) -> TickBudgetRecord {
        TickBudgetRecord {
            tick_index: 0,
            day: 0,
            hour: 0,
            before: BudgetTotals::default(),
            after: BudgetTotals::default(),
            net_delta,
            entries,
        }
    }

    #[test]
    fn no_op_open_system_actions_do_not_disable_the_budget_guard() {
        let tick = synthetic_tick(vec![
            synthetic_entry("action:feed", BudgetDelta::default()),
            synthetic_entry("action:water_change", BudgetDelta::default()),
            synthetic_entry("action:siphon_detritus", BudgetDelta::default()),
            synthetic_entry("action:add_shrimp", BudgetDelta::default()),
            synthetic_entry("action:remove_shrimp", BudgetDelta::default()),
        ]);

        assert!(tick_has_closed_system_nitrogen(&tick));
        assert!(tick_has_closed_system_carbon(&tick));
    }

    #[test]
    fn carbon_guard_uses_carbon_flux_for_open_system_actions() {
        let tick = synthetic_tick(vec![synthetic_entry(
            "action:water_change",
            BudgetDelta {
                carbon: ElementBudget {
                    in_mg: 0.0,
                    out_mg: 8.0,
                },
                ..BudgetDelta::default()
            },
        )]);

        assert!(tick_has_closed_system_nitrogen(&tick));
        assert!(!tick_has_closed_system_carbon(&tick));
    }

    #[test]
    fn clean_filter_is_treated_as_closed_when_it_only_reroutes_internal_mass() {
        let tick = synthetic_tick(vec![synthetic_entry(
            "action:clean_filter",
            BudgetDelta {
                nitrogen: ElementBudget {
                    in_mg: 2.0,
                    out_mg: 2.0,
                },
                carbon: ElementBudget {
                    in_mg: 10.0,
                    out_mg: 10.0,
                },
                ..BudgetDelta::default()
            },
        )]);

        assert!(tick_has_closed_system_nitrogen(&tick));
        assert!(tick_has_closed_system_carbon(&tick));
    }

    #[test]
    fn chemistry_flux_does_not_open_the_carbon_guard() {
        let tick = synthetic_tick_with_net_delta(
            vec![synthetic_entry(
                "system:chemistry",
                BudgetDelta {
                    carbon: ElementBudget {
                        in_mg: 5.0,
                        out_mg: 3.0,
                    },
                    ..BudgetDelta::default()
                },
            )],
            BudgetDelta {
                carbon: ElementBudget {
                    in_mg: 5.0,
                    out_mg: 3.0,
                },
                ..BudgetDelta::default()
            },
        );

        assert!(tick_has_closed_system_carbon(&tick));
        assert!((carbon_guard_delta_mg(&tick) - 0.0).abs() <= f64::EPSILON);
    }

    #[test]
    fn chemistry_flux_is_subtracted_before_carbon_budget_guard_checks_other_leaks() {
        let tick = synthetic_tick_with_net_delta(
            vec![
                synthetic_entry(
                    "system:chemistry",
                    BudgetDelta {
                        carbon: ElementBudget {
                            in_mg: 5.0,
                            out_mg: 3.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
                synthetic_entry(
                    "system:daily_plants",
                    BudgetDelta {
                        carbon: ElementBudget {
                            in_mg: 0.0,
                            out_mg: 4.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
            ],
            BudgetDelta {
                carbon: ElementBudget {
                    in_mg: 5.0,
                    out_mg: 7.0,
                },
                ..BudgetDelta::default()
            },
        );

        let err = enforce_tracked_tick_budget_guard(&tick).expect_err(
            "chemistry exchange should not hide unrelated carbon drift in the same tick",
        );
        assert_eq!(
            err,
            SimError::BudgetImbalance {
                element: "carbon",
                delta_mg: -4.0,
                tick_index: 0,
                day: 0,
                hour: 0,
            }
        );
    }

    #[test]
    fn open_action_flux_is_subtracted_before_nitrogen_budget_guard_checks_other_leaks() {
        let tick = synthetic_tick_with_net_delta(
            vec![
                synthetic_entry(
                    "action:feed",
                    BudgetDelta {
                        nitrogen: ElementBudget {
                            in_mg: 10.0,
                            out_mg: 0.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
                synthetic_entry(
                    "system:daily_plants",
                    BudgetDelta {
                        nitrogen: ElementBudget {
                            in_mg: 0.0,
                            out_mg: 3.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
            ],
            BudgetDelta {
                nitrogen: ElementBudget {
                    in_mg: 10.0,
                    out_mg: 3.0,
                },
                ..BudgetDelta::default()
            },
        );

        let err = enforce_tracked_tick_budget_guard(&tick).expect_err(
            "external feed import should not hide unrelated nitrogen drift in the same tick",
        );
        assert_eq!(
            err,
            SimError::BudgetImbalance {
                element: "nitrogen",
                delta_mg: -3.0,
                tick_index: 0,
                day: 0,
                hour: 0,
            }
        );
    }

    #[test]
    fn open_action_flux_is_subtracted_before_carbon_budget_guard_checks_other_leaks() {
        let tick = synthetic_tick_with_net_delta(
            vec![
                synthetic_entry(
                    "action:water_change",
                    BudgetDelta {
                        carbon: ElementBudget {
                            in_mg: 0.0,
                            out_mg: 8.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
                synthetic_entry(
                    "system:daily_algae",
                    BudgetDelta {
                        carbon: ElementBudget {
                            in_mg: 0.0,
                            out_mg: 4.0,
                        },
                        ..BudgetDelta::default()
                    },
                ),
            ],
            BudgetDelta {
                carbon: ElementBudget {
                    in_mg: 0.0,
                    out_mg: 12.0,
                },
                ..BudgetDelta::default()
            },
        );

        let err = enforce_tracked_tick_budget_guard(&tick)
            .expect_err("external carbon export should not hide unrelated same-tick carbon drift");
        assert_eq!(
            err,
            SimError::BudgetImbalance {
                element: "carbon",
                delta_mg: -4.0,
                tick_index: 0,
                day: 0,
                hour: 0,
            }
        );
    }
}
