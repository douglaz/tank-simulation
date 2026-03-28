use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use super::ESTIMATED_TDS_SCOPE_LINES;
use crate::TuiApp;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let snapshot = &app.snapshot;
    let layout = Layout::vertical([Constraint::Length(9), Constraint::Min(10)]).split(area);
    let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);
    let bottom = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
    ])
    .split(layout[1]);

    let dissolved = Paragraph::new(vec![
        Line::from(format!("pH {:.3}", snapshot.ph)),
        Line::from(format!("TAN {:.3} mg N/L", snapshot.tan_mg_n_per_l)),
        Line::from(format!(
            "Free NH3-N {:.5} mg NH3-N/L",
            snapshot.nh3_mg_n_per_l
        )),
        Line::from(format!("Nitrite {:.3} mg N/L", snapshot.nitrite_mg_n_per_l)),
        Line::from(format!("Nitrate {:.3} mg N/L", snapshot.nitrate_mg_n_per_l)),
        Line::from(format!(
            "Phosphate {:.3} mg P/L",
            snapshot.phosphate_mg_p_per_l
        )),
        Line::from(format!(
            "DIC {:.3} mg C/L",
            snapshot.dissolved_inorganic_carbon_mg_c_per_l
        )),
    ])
    .block(
        Block::default()
            .title("Dissolved chemistry")
            .borders(Borders::ALL),
    );
    frame.render_widget(dissolved, top[0]);

    let mut derived_lines = vec![
        Line::from(format!(
            "DO {:.3}/{:.3} mg/L",
            snapshot.do_mg_l, snapshot.do_sat_mg_l
        )),
        Line::from(format!(
            "GH (Ca+Mg) {:.1} d  KH (alk) {:.1} d",
            snapshot.gh_d, snapshot.kh_d
        )),
        Line::from(format!(
            "Est. TDS (7-ion) {:.0} mg/L",
            snapshot.estimated_tds_7_ion_mg_per_l
        )),
        Line::from(format!(
            "Est. conductivity (7-ion) {:.0} uS/cm",
            snapshot.estimated_conductivity_us_cm
        )),
        Line::from(format!("Temp {:.2} C", snapshot.water_temp_c)),
    ];
    derived_lines.extend(ESTIMATED_TDS_SCOPE_LINES.into_iter().map(Line::from));
    let derived = Paragraph::new(derived_lines)
    .block(
        Block::default()
            .title("Derived display values")
            .borders(Borders::ALL),
    );
    frame.render_widget(derived, top[1]);

    render_trend(
        frame,
        bottom[0],
        "pH trend",
        snapshot.ph,
        app.history_values(|item| item.ph),
        Color::Cyan,
    );
    render_trend(
        frame,
        bottom[1],
        "TAN trend (mg N/L)",
        snapshot.tan_mg_n_per_l,
        app.history_values(|item| item.tan_mg_n_per_l),
        Color::Yellow,
    );
    render_trend(
        frame,
        bottom[2],
        "DO trend",
        snapshot.do_mg_l,
        app.history_values(|item| item.do_mg_l),
        Color::Green,
    );
    render_trend(
        frame,
        bottom[3],
        "Temperature trend",
        snapshot.water_temp_c,
        app.history_values(|item| item.water_temp_c),
        Color::Magenta,
    );
}

fn render_trend(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    current_value: f64,
    history: Vec<f64>,
    color: Color,
) {
    let arrow = trend_arrow(&history);
    let sparkline_data = scale_history(history);
    let block = Block::default()
        .title(format!("{title} {arrow}  current {current_value:.3}"))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sparkline = Sparkline::default()
        .data(&sparkline_data)
        .style(Style::default().fg(color));
    frame.render_widget(sparkline, inner);
}

fn scale_history(history: Vec<f64>) -> Vec<u64> {
    if history.is_empty() {
        return vec![0];
    }

    let min = history.iter().copied().fold(f64::INFINITY, f64::min);
    let max = history.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(0.000_001);

    history
        .into_iter()
        .map(|value| (((value - min) / span) * 100.0).round() as u64 + 1)
        .collect()
}

fn trend_arrow(history: &[f64]) -> &'static str {
    if history.len() < 2 {
        return "->";
    }

    let last = history[history.len() - 1];
    let previous = history[history.len() - 2];
    let delta = last - previous;
    if delta > 0.000_1 {
        "^"
    } else if delta < -0.000_1 {
        "v"
    } else {
        "->"
    }
}
