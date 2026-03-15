use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem},
};
use tank_core::EventSeverity;

use crate::TuiApp;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let items = if app.snapshot.recent_events.is_empty() {
        vec![ListItem::new("No events yet")]
    } else {
        app.snapshot
            .recent_events
            .iter()
            .map(|event| {
                let style = match event.severity {
                    EventSeverity::Info => Style::default().fg(Color::DarkGray),
                    EventSeverity::Warning => Style::default().fg(Color::Yellow),
                    EventSeverity::Critical => Style::default().fg(Color::Red),
                };
                let causes = event
                    .cause_codes
                    .iter()
                    .map(|cause| format!("{cause:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                ListItem::new(vec![
                    Line::from(Span::styled(
                        format!(
                            "Day {} {:02}:00 {:?} {:?}",
                            event.day, event.hour, event.severity, event.kind
                        ),
                        style,
                    )),
                    Line::from(format!("{}  [{causes}]", event.summary)),
                ])
            })
            .collect()
    };

    let log = List::new(items).block(
        Block::default()
            .title("Recent events")
            .borders(Borders::ALL),
    );
    frame.render_widget(log, area);
}
