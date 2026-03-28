pub mod actions;
pub mod biology;
pub mod chemistry;
pub mod log;
pub mod overview;

use ratatui::prelude::*;
pub(crate) use tank_core::ESTIMATED_TDS_SCOPE_LINES;

use crate::{Screen, TuiApp};

pub fn render_screen(screen: Screen, frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    match screen {
        Screen::Overview => overview::render(frame, area, app),
        Screen::Chemistry => chemistry::render(frame, area, app),
        Screen::Biology => biology::render(frame, area, app),
        Screen::Actions => actions::render(frame, area, app),
        Screen::Log => log::render(frame, area, app),
    }
}

#[cfg(test)]
mod tests {
    use super::ESTIMATED_TDS_SCOPE_LINES;

    #[test]
    fn estimated_tds_scope_lines_name_included_and_omitted_species() {
        let text = ESTIMATED_TDS_SCOPE_LINES.join(" ");

        assert!(text.contains("Ca"));
        assert!(text.contains("HCO3"));
        assert!(text.contains("TAN/NH3"));
        assert!(text.contains("PO4"));
        assert!(text.contains("trace ions"));
    }
}
