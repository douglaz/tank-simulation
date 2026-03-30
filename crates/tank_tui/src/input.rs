use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::{screens::actions::ActionKind, Screen, StatusLevel, TuiApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOutcome {
    Continue,
    Quit,
}

pub fn handle_key_event(app: &mut TuiApp, key: KeyEvent) -> InputOutcome {
    if key.kind != KeyEventKind::Press {
        return InputOutcome::Continue;
    }

    match key.code {
        KeyCode::Char('q') => return InputOutcome::Quit,
        KeyCode::Tab => app.next_screen(),
        KeyCode::Char(' ') => app.toggle_auto_advance(),
        KeyCode::Char('h') => app.step_hours(1),
        KeyCode::Char('d') => app.step_hours(24),
        KeyCode::Char('w') => app.step_hours(24 * 7),
        KeyCode::Char('s') => {
            if let Err(error) = app.save_to_disk() {
                app.show_status(StatusLevel::Error, format!("Save failed: {error:#}"));
            }
        }
        KeyCode::Char('l') => {
            if let Err(error) = app.load_from_disk() {
                app.show_status(StatusLevel::Error, format!("Load failed: {error:#}"));
            }
        }
        KeyCode::Esc if app.active_screen == Screen::Actions => {
            app.set_screen(Screen::Overview);
        }
        KeyCode::Char(digit) if Screen::from_digit(digit).is_some() => {
            // On Actions screen, digits go to the numeric field editor first;
            // only switch screens if the field rejects the input.
            // Use Esc to leave Actions when a numeric field is focused.
            let consumed = app.active_screen == Screen::Actions && app.action_form.edit_char(digit);
            if !consumed {
                if let Some(screen) = Screen::from_digit(digit) {
                    app.set_screen(screen);
                }
            }
        }
        _ => {
            if app.active_screen == Screen::Actions {
                handle_actions_input(app, key.code);
            }
        }
    }

    InputOutcome::Continue
}

fn handle_actions_input(app: &mut TuiApp, code: KeyCode) {
    match code {
        KeyCode::Up => app.action_form.select_previous_action(),
        KeyCode::Down => app.action_form.select_next_action(),
        KeyCode::Left => app.action_form.select_previous_field(),
        KeyCode::Right => app.action_form.select_next_field(),
        KeyCode::Char('[') if !app.action_form.cycle_choice(false) => {
            app.show_status(StatusLevel::Warning, "Selected field is not a choice field");
        }
        KeyCode::Char(']') if !app.action_form.cycle_choice(true) => {
            app.show_status(StatusLevel::Warning, "Selected field is not a choice field");
        }
        KeyCode::Char('t') if !app.action_form.toggle_boolean() => {
            app.show_status(StatusLevel::Warning, "Selected field is not a toggle");
        }
        KeyCode::Backspace if !app.action_form.backspace() => {
            app.show_status(StatusLevel::Warning, "Selected field is not numeric");
        }
        KeyCode::Enter => match app.action_form.submit() {
            Ok(action) => app.apply_action(action),
            Err(error) => app.show_status(StatusLevel::Error, error),
        },
        KeyCode::Char(ch)
            if (ch.is_ascii_digit() || ch == '.') && !app.action_form.edit_char(ch) =>
        {
            let target = app.action_form.selected_action();
            let message = if matches!(
                target,
                ActionKind::WaterChangePercent | ActionKind::ChangeAeration
            ) {
                "Use [ ] or t for non-numeric fields"
            } else {
                "Selected field only accepts numeric input"
            };
            app.show_status(StatusLevel::Warning, message);
        }
        _ => {}
    }
}
