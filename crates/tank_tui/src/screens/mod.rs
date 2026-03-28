pub mod actions;
pub mod biology;
pub mod chemistry;
pub mod log;
pub mod overview;

use ratatui::prelude::*;

use crate::{Screen, TuiApp};

pub(crate) const ESTIMATED_TDS_SCOPE_LINES: [&str; 3] = [
    "7 ions: Ca Mg Na K HCO3 Cl SO4",
    "Omits TAN/NH3 NO2 NO3 PO4",
    "Omits organics + trace ions",
];

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
