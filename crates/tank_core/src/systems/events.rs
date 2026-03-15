use crate::{
    systems::chemistry::compute_nh3_mg_l,
    types::{EventCause, EventKind, EventSeverity, SimEvent, TankState},
};

pub fn emit_hourly_threshold_events(state: &mut TankState) {
    let volume_l = state.geometry.water_volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let tan_mg_l = state.water.ammonia_total_mg_n_total / volume_l;
    let nh3_mg_l = compute_nh3_mg_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = state.water.nitrite_mg_n_total / volume_l;
    let do_mg_l = state.water.dissolved_oxygen_mg_total / volume_l;

    if nh3_mg_l >= 0.02 {
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::AmmoniaWarning,
            vec![EventCause::HighAmmonia],
            format!("Free ammonia reached {nh3_mg_l:.3} mg/L"),
        );
    }
    if nitrite_mg_l >= 0.5 {
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::NitriteWarning,
            vec![EventCause::HighNitrite],
            format!("Nitrite reached {nitrite_mg_l:.2} mg/L"),
        );
    }
    if do_mg_l < 4.0 {
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::OxygenDip,
            vec![EventCause::LowOxygen],
            format!("Dissolved oxygen dropped to {do_mg_l:.2} mg/L"),
        );
    }
}

fn emit_once_per_day(
    state: &mut TankState,
    severity: EventSeverity,
    kind: EventKind,
    cause_codes: Vec<EventCause>,
    summary: String,
) {
    let key = format!("{kind:?}");
    if state.last_event_day.get(&key) == Some(&state.environment.day) {
        return;
    }

    state.event_log.push(SimEvent::new(
        state.environment.day,
        state.environment.hour_of_day,
        severity,
        kind,
        cause_codes,
        summary,
    ));
    state.last_event_day.insert(key, state.environment.day);
}
