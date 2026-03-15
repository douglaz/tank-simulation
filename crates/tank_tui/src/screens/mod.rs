pub mod actions;
pub mod biology;
pub mod chemistry;
pub mod log;
pub mod overview;

use ratatui::prelude::*;

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
