use crate::{
    systems::chemistry::compute_nh3_mg_n_per_l,
    systems::shrimp::compute_nitrite_stress_diagnostics,
    types::{EventCause, EventKind, EventSeverity, SimEvent, TankState},
};

pub fn emit_hourly_threshold_events(state: &mut TankState) {
    let chemistry = state.concentrations();
    let volume_l = chemistry.volume_l();
    if volume_l <= f64::EPSILON {
        return;
    }

    let tan_mg_l = chemistry.tan_mg_n_per_l();
    let nh3_mg_l = compute_nh3_mg_n_per_l(tan_mg_l, state.water.ph, state.water.temperature_c);
    let nitrite_mg_l = chemistry.nitrite_mg_n_per_l();
    let chloride_mg_l = chemistry.chloride_mg_per_l();
    let do_mg_l = chemistry.do_mg_per_l();

    if nh3_mg_l >= 0.02 {
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::AmmoniaWarning,
            vec![EventCause::HighAmmonia],
            format!("Free ammonia (NH3-N) reached {nh3_mg_l:.3} mg NH3-N/L"),
        );
    }
    if nitrite_mg_l >= 0.5 {
        let nitrite_diagnostics = compute_nitrite_stress_diagnostics(
            nitrite_mg_l,
            chloride_mg_l,
            state.shrimp_params.chloride_protection_factor,
        );
        let cl_no2_ratio = if nitrite_mg_l > f64::EPSILON {
            chloride_mg_l / nitrite_mg_l
        } else {
            0.0
        };
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::NitriteWarning,
            vec![EventCause::HighNitrite],
            format!(
                "Nitrite-N {nitrite_mg_l:.2} mg N/L, Cl {chloride_mg_l:.1} mg/L \
                 (Cl:NO2 {cl_no2_ratio:.1}:1), effective hazard \
                 {effective_hazard:.3}, nitrite stress +{stress_increment:.4}/h",
                effective_hazard = nitrite_diagnostics.effective_hazard_mg_l,
                stress_increment = nitrite_diagnostics.hourly_stress_increment,
            ),
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

pub fn emit_once_per_day_pub(
    state: &mut TankState,
    severity: EventSeverity,
    kind: EventKind,
    cause_codes: Vec<EventCause>,
    summary: String,
) {
    emit_once_per_day(state, severity, kind, cause_codes, summary);
}

pub fn emit_daily_algae_events(
    state: &mut TankState,
    previous_nuisance_index: f64,
    periphyton_capacity_g: f64,
) {
    let volume_l = state.water_volume_l();
    if previous_nuisance_index < 0.5 && state.algae.nuisance_index >= 0.5 {
        let mut causes = vec![EventCause::HighNutrients];
        // Only attribute PlantCrowding if the tank actually has significant plant biomass.
        let total_plant_biomass: f64 = state.plant_guilds.iter().map(|p| p.biomass_g).sum();
        if total_plant_biomass > 1.0 {
            let avg_crowding = state
                .plant_guilds
                .iter()
                .map(|p| p.crowding_index)
                .sum::<f64>()
                / state.plant_guilds.len().max(1) as f64;
            if avg_crowding > 0.3 {
                causes.push(EventCause::PlantCrowding);
            }
        }
        emit_once_per_day(
            state,
            EventSeverity::Warning,
            EventKind::AlgaeRiskRising,
            causes,
            format!(
                "Algae nuisance pressure rose to {:.2}",
                state.algae.nuisance_index
            ),
        );
    }

    if volume_l > f64::EPSILON {
        let suspended_g_l = state.algae.suspended_biomass_g / volume_l;
        if suspended_g_l >= state.process_params.algae_bloom_threshold_g_per_l {
            emit_once_per_day(
                state,
                EventSeverity::Warning,
                EventKind::AlgaeBloom,
                vec![EventCause::HighNutrients],
                format!("Suspended algae reached {suspended_g_l:.3} g/L"),
            );
        }
    }

    if periphyton_capacity_g > f64::EPSILON {
        let occupancy = state.algae.periphyton_biomass_g / periphyton_capacity_g;
        let mean_low_oxygen_tendency = if state.substrate_layers.is_empty() {
            0.0
        } else {
            state
                .substrate_layers
                .iter()
                .map(|layer| layer.low_oxygen_tendency_index)
                .sum::<f64>()
                / state.substrate_layers.len() as f64
        };
        if occupancy >= 0.8 && mean_low_oxygen_tendency >= 0.55 {
            emit_once_per_day(
                state,
                EventSeverity::Warning,
                EventKind::SubstrateFoulingWarning,
                vec![EventCause::SurfaceSaturation, EventCause::LowOxygen],
                format!(
                    "Periphyton occupancy reached {:.0}% of surface capacity",
                    occupancy * 100.0
                ),
            );
        }
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
