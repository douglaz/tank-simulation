use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tank_core::TankSnapshot;

use super::{ESTIMATED_TDS_SCOPE_LINES, NH3_N_DISPLAY_UNIT};
use crate::TuiApp;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let snapshot = &app.snapshot;
    let layout = Layout::vertical([
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Min(6),
    ])
    .split(area);
    let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);
    let middle = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);

    let chemistry = Paragraph::new(
        water_column_text(snapshot)
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>(),
    )
    .block(Block::default().title("Water column").borders(Borders::ALL));
    frame.render_widget(chemistry, top[0]);

    let systems = Paragraph::new(
        systems_text(snapshot)
            .into_iter()
            .map(Line::from)
            .collect::<Vec<_>>(),
    )
    .block(Block::default().title("Hardware").borders(Borders::ALL));
    frame.render_widget(systems, top[1]);

    let plant_summary = Paragraph::new(vec![
        Line::from(format!(
            "Total biomass {:.2} g",
            snapshot.total_plant_biomass_g
        )),
        Line::from(format!(
            "Fast stem {:.2} g  health {:.2}",
            snapshot.fast_stem_biomass_g, snapshot.fast_stem_health_index
        )),
        Line::from(format!(
            "Rosette {:.2} g  health {:.2}",
            snapshot.root_feeding_rosette_biomass_g, snapshot.root_feeding_rosette_health_index
        )),
    ])
    .block(Block::default().title("Plant status").borders(Borders::ALL));
    frame.render_widget(plant_summary, middle[0]);

    let shrimp_summary = Paragraph::new(vec![
        Line::from(format!(
            "Total {}  Adults {}  Sub-adults {}",
            snapshot.total_shrimp_count, snapshot.adult_shrimp_count, snapshot.sub_adult_count
        )),
        Line::from(format!(
            "Juveniles {}  Berried {}",
            snapshot.juveniles_count, snapshot.berried_females_count
        )),
        Line::from(format!(
            "Condition {:.2}  Molt stress {:.2}",
            snapshot.shrimp_condition_index, snapshot.shrimp_molt_stress_index
        )),
        Line::from(format!(
            "Repro readiness {:.2}",
            snapshot.shrimp_reproductive_readiness
        )),
    ])
    .block(
        Block::default()
            .title("Shrimp status")
            .borders(Borders::ALL),
    );
    frame.render_widget(shrimp_summary, middle[1]);

    let warnings = app.active_warning_lines();
    let warning_items = if warnings.is_empty() {
        vec![ListItem::new("No active warnings")]
    } else {
        warnings
            .into_iter()
            .map(|warning| ListItem::new(warning).style(Style::default().fg(Color::Yellow)))
            .collect()
    };
    let warning_list = List::new(warning_items).block(
        Block::default()
            .title("Active warnings")
            .borders(Borders::ALL),
    );
    frame.render_widget(warning_list, layout[2]);
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "on"
    } else {
        "off"
    }
}

fn water_column_text(snapshot: &TankSnapshot) -> Vec<String> {
    vec![
        format!("Day {}  Hour {:02}", snapshot.day, snapshot.hour),
        format!(
            "Temp {:.2} C  Ambient {:.2} C",
            snapshot.water_temp_c, snapshot.ambient_temp_c
        ),
        format!(
            "pH {:.2}  DO {:.2}/{:.2} mg/L",
            snapshot.ph, snapshot.do_mg_l, snapshot.do_sat_mg_l
        ),
        format!(
            "TAN {:.3} mg N/L  NH3-N {:.4} {NH3_N_DISPLAY_UNIT}",
            snapshot.tan_mg_n_per_l, snapshot.nh3_mg_n_per_l
        ),
        format!(
            "NO2 {:.3}  NO3 {:.3} mg N/L",
            snapshot.nitrite_mg_n_per_l, snapshot.nitrate_mg_n_per_l
        ),
    ]
}

fn systems_text(snapshot: &TankSnapshot) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Volume {:.1} L  DIC {:.2} mg C/L",
            snapshot.water_volume_l, snapshot.dissolved_inorganic_carbon_mg_c_per_l
        ),
        format!(
            "GH (Ca+Mg) {:.1} d  KH (alkalinity) {:.1} d",
            snapshot.gh_d, snapshot.kh_d
        ),
        format!(
            "Est. TDS (7-ion) {:.0} mg/L  Est. cond {:.0} uS/cm",
            snapshot.estimated_tds_7_ion_mg_per_l, snapshot.estimated_conductivity_us_cm
        ),
        format!(
            "Light {} @ {:.2} for {:.1}h",
            on_off(snapshot.light_enabled),
            snapshot.light_intensity_index,
            snapshot.photoperiod_hours
        ),
        format!(
            "Heater {} @ {:.1} C  Aeration {} @ {:.2}",
            on_off(snapshot.heater_enabled),
            snapshot.heater_setpoint_c,
            on_off(snapshot.aeration_enabled),
            snapshot.aeration_intensity
        ),
    ];
    lines.splice(
        3..3,
        ESTIMATED_TDS_SCOPE_LINES.into_iter().map(str::to_string),
    );
    lines
}

#[cfg(test)]
mod tests {
    use tank_core::{rng::SimSeed, TankState};

    use super::{systems_text, water_column_text};

    #[test]
    fn overview_labels_tds_and_conductivity_as_estimates() {
        let snapshot = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(912)));
        let summary = &systems_text(&snapshot)[2];

        assert!(summary.contains("Est. TDS (7-ion)"));
        assert!(summary.contains("Est. cond"));
        assert!(!summary.contains("7-ion TDS"));
    }

    #[test]
    fn overview_uses_shared_nh3_n_unit_label() {
        let snapshot = tank_core::TankSnapshot::from_state(&TankState::new(SimSeed(913)));
        let ammonia_line = &water_column_text(&snapshot)[3];

        assert!(ammonia_line.contains("NH3-N"));
        assert!(ammonia_line.contains("mg NH3-N/L"));
        assert!(ammonia_line.contains("mg N/L"));
    }
}
