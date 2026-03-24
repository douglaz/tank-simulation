use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use tank_core::PlayerAction;

use crate::TuiApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Feed,
    WaterChangePercent,
    TrimPlants,
    SiphonDetritus,
    CleanFilter,
    AddShrimp,
    RemoveShrimp,
    ChangePhotoperiod,
    ChangeLightIntensity,
    ChangeHeaterSetpoint,
    ChangeAmbientTemperature,
    ChangeAeration,
}

impl ActionKind {
    pub const ALL: [Self; 12] = [
        Self::Feed,
        Self::WaterChangePercent,
        Self::TrimPlants,
        Self::SiphonDetritus,
        Self::CleanFilter,
        Self::AddShrimp,
        Self::RemoveShrimp,
        Self::ChangePhotoperiod,
        Self::ChangeLightIntensity,
        Self::ChangeHeaterSetpoint,
        Self::ChangeAmbientTemperature,
        Self::ChangeAeration,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Feed => "Feed",
            Self::WaterChangePercent => "Water change",
            Self::TrimPlants => "Trim plants",
            Self::SiphonDetritus => "Siphon detritus",
            Self::CleanFilter => "Clean filter",
            Self::AddShrimp => "Add shrimp",
            Self::RemoveShrimp => "Remove shrimp",
            Self::ChangePhotoperiod => "Photoperiod",
            Self::ChangeLightIntensity => "Light intensity",
            Self::ChangeHeaterSetpoint => "Heater setpoint",
            Self::ChangeAmbientTemperature => "Ambient temp",
            Self::ChangeAeration => "Aeration",
        }
    }

    fn field_count(self) -> usize {
        match self {
            Self::WaterChangePercent | Self::ChangeAeration => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionFormState {
    selected_action: usize,
    selected_field: usize,
    replace_next_numeric: bool,
    feed_grams: String,
    water_change_percent: String,
    water_change_source_index: usize,
    trim_fraction: String,
    siphon_fraction: String,
    clean_filter_intensity: String,
    add_shrimp_count: String,
    remove_shrimp_count: String,
    photoperiod_hours: String,
    light_intensity: String,
    heater_setpoint_c: String,
    ambient_temp_c: String,
    aeration_enabled: bool,
    aeration_intensity: String,
    source_water_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Numeric { integer_only: bool },
    Choice,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFieldView {
    pub label: String,
    pub value: String,
    pub hint: &'static str,
}

impl ActionFormState {
    pub fn new(source_water_ids: Vec<String>) -> Self {
        Self {
            selected_action: 0,
            selected_field: 0,
            replace_next_numeric: true,
            feed_grams: "0.20".to_string(),
            water_change_percent: "25.0".to_string(),
            water_change_source_index: 1.min(source_water_ids.len().saturating_sub(1)),
            trim_fraction: "0.25".to_string(),
            siphon_fraction: "0.30".to_string(),
            clean_filter_intensity: "0.50".to_string(),
            add_shrimp_count: "5".to_string(),
            remove_shrimp_count: "2".to_string(),
            photoperiod_hours: "8.0".to_string(),
            light_intensity: "0.70".to_string(),
            heater_setpoint_c: "24.0".to_string(),
            ambient_temp_c: "24.0".to_string(),
            aeration_enabled: true,
            aeration_intensity: "0.40".to_string(),
            source_water_ids,
        }
    }
}

impl Default for ActionFormState {
    fn default() -> Self {
        Self::new(vec![
            "soft_acidic".to_string(),
            "moderate".to_string(),
            "hard_shrimp".to_string(),
            "ro_like".to_string(),
        ])
    }
}

impl ActionFormState {
    pub fn selected_action(&self) -> ActionKind {
        ActionKind::ALL[self.selected_action]
    }

    pub fn selected_action_index(&self) -> usize {
        self.selected_action
    }

    pub fn selected_field_index(&self) -> usize {
        self.selected_field
    }

    pub fn fields(&self) -> Vec<ActionFieldView> {
        match self.selected_action() {
            ActionKind::Feed => vec![ActionFieldView {
                label: "grams".to_string(),
                value: self.feed_grams.clone(),
                hint: "Type a non-negative amount",
            }],
            ActionKind::WaterChangePercent => vec![
                ActionFieldView {
                    label: "percent".to_string(),
                    value: self.water_change_percent.clone(),
                    hint: "0.0 to 100.0",
                },
                ActionFieldView {
                    label: "source".to_string(),
                    value: self
                        .source_water_ids
                        .get(self.water_change_source_index)
                        .cloned()
                        .unwrap_or_default(),
                    hint: "[ / ] cycle source profile",
                },
            ],
            ActionKind::TrimPlants => vec![ActionFieldView {
                label: "fraction".to_string(),
                value: self.trim_fraction.clone(),
                hint: "0.0 to 1.0",
            }],
            ActionKind::SiphonDetritus => vec![ActionFieldView {
                label: "fraction".to_string(),
                value: self.siphon_fraction.clone(),
                hint: "0.0 to 1.0",
            }],
            ActionKind::CleanFilter => vec![ActionFieldView {
                label: "intensity".to_string(),
                value: self.clean_filter_intensity.clone(),
                hint: "0.0 to 1.0",
            }],
            ActionKind::AddShrimp => vec![ActionFieldView {
                label: "count".to_string(),
                value: self.add_shrimp_count.clone(),
                hint: "Whole shrimp count",
            }],
            ActionKind::RemoveShrimp => vec![ActionFieldView {
                label: "count".to_string(),
                value: self.remove_shrimp_count.clone(),
                hint: "Whole shrimp count",
            }],
            ActionKind::ChangePhotoperiod => vec![ActionFieldView {
                label: "hours".to_string(),
                value: self.photoperiod_hours.clone(),
                hint: "0.0 to 24.0",
            }],
            ActionKind::ChangeLightIntensity => vec![ActionFieldView {
                label: "intensity".to_string(),
                value: self.light_intensity.clone(),
                hint: "0.0 to 1.0",
            }],
            ActionKind::ChangeHeaterSetpoint => vec![ActionFieldView {
                label: "setpoint C".to_string(),
                value: self.heater_setpoint_c.clone(),
                hint: "> 0.0 C",
            }],
            ActionKind::ChangeAmbientTemperature => vec![ActionFieldView {
                label: "target C".to_string(),
                value: self.ambient_temp_c.clone(),
                hint: "> 0.0 C",
            }],
            ActionKind::ChangeAeration => vec![
                ActionFieldView {
                    label: "enabled".to_string(),
                    value: if self.aeration_enabled {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    },
                    hint: "Press t to toggle",
                },
                ActionFieldView {
                    label: "intensity".to_string(),
                    value: self.aeration_intensity.clone(),
                    hint: "0.0 to 1.0",
                },
            ],
        }
    }

    pub fn select_next_action(&mut self) {
        self.selected_action = (self.selected_action + 1) % ActionKind::ALL.len();
        self.reset_selection();
    }

    pub fn select_previous_action(&mut self) {
        self.selected_action =
            (self.selected_action + ActionKind::ALL.len() - 1) % ActionKind::ALL.len();
        self.reset_selection();
    }

    pub fn select_next_field(&mut self) {
        let max_fields = self.selected_action().field_count();
        self.selected_field = (self.selected_field + 1) % max_fields;
        self.replace_next_numeric = true;
    }

    pub fn select_previous_field(&mut self) {
        let max_fields = self.selected_action().field_count();
        self.selected_field = (self.selected_field + max_fields - 1) % max_fields;
        self.replace_next_numeric = true;
    }

    pub fn cycle_choice(&mut self, forward: bool) -> bool {
        if self.current_field_kind() != FieldKind::Choice {
            return false;
        }

        let count = self.source_water_ids.len();
        if count == 0 {
            return true;
        }
        if forward {
            self.water_change_source_index = (self.water_change_source_index + 1) % count;
        } else {
            self.water_change_source_index = (self.water_change_source_index + count - 1) % count;
        }
        true
    }

    pub fn toggle_boolean(&mut self) -> bool {
        if self.current_field_kind() != FieldKind::Toggle {
            return false;
        }

        self.aeration_enabled = !self.aeration_enabled;
        true
    }

    pub fn edit_char(&mut self, ch: char) -> bool {
        let replace_next = self.replace_next_numeric;
        self.replace_next_numeric = false;
        let Some((buffer, integer_only)) = self.current_numeric_buffer_mut() else {
            self.replace_next_numeric = replace_next;
            return false;
        };

        if integer_only && !ch.is_ascii_digit() {
            return false;
        }
        if !(integer_only || ch.is_ascii_digit() || ch == '.') {
            return false;
        }
        if ch == '.' && buffer.contains('.') {
            return false;
        }

        if replace_next {
            buffer.clear();
        }
        buffer.push(ch);
        true
    }

    pub fn backspace(&mut self) -> bool {
        let Some((buffer, _)) = self.current_numeric_buffer_mut() else {
            return false;
        };

        buffer.pop();
        let became_empty = buffer.is_empty();
        if became_empty {
            buffer.push('0');
        }
        self.replace_next_numeric = became_empty;
        true
    }

    pub fn submit(&mut self) -> Result<PlayerAction, String> {
        let action = match self.selected_action() {
            ActionKind::Feed => PlayerAction::Feed {
                grams: parse_f64("grams", &self.feed_grams)?,
            },
            ActionKind::WaterChangePercent => PlayerAction::WaterChangePercent {
                percent: parse_f64("percent", &self.water_change_percent)?,
                source_profile_id: self
                    .source_water_ids
                    .get(self.water_change_source_index)
                    .cloned()
                    .unwrap_or_default(),
            },
            ActionKind::TrimPlants => PlayerAction::TrimPlants {
                fraction: parse_f64("fraction", &self.trim_fraction)?,
            },
            ActionKind::SiphonDetritus => PlayerAction::SiphonDetritus {
                fraction: parse_f64("fraction", &self.siphon_fraction)?,
            },
            ActionKind::CleanFilter => PlayerAction::CleanFilter {
                intensity: parse_f64("intensity", &self.clean_filter_intensity)?,
            },
            ActionKind::AddShrimp => PlayerAction::AddShrimp {
                count: parse_u32("count", &self.add_shrimp_count)?,
            },
            ActionKind::RemoveShrimp => PlayerAction::RemoveShrimp {
                count: parse_u32("count", &self.remove_shrimp_count)?,
            },
            ActionKind::ChangePhotoperiod => PlayerAction::ChangePhotoperiod {
                hours: parse_f64("hours", &self.photoperiod_hours)?,
            },
            ActionKind::ChangeLightIntensity => PlayerAction::ChangeLightIntensity {
                intensity_index: parse_f64("intensity_index", &self.light_intensity)?,
            },
            ActionKind::ChangeHeaterSetpoint => PlayerAction::ChangeHeaterSetpoint {
                setpoint_c: parse_f64("setpoint_c", &self.heater_setpoint_c)?,
            },
            ActionKind::ChangeAmbientTemperature => PlayerAction::ChangeAmbientTemperature {
                target_c: parse_f64("target_c", &self.ambient_temp_c)?,
            },
            ActionKind::ChangeAeration => PlayerAction::ChangeAeration {
                enabled: self.aeration_enabled,
                intensity: parse_f64("intensity", &self.aeration_intensity)?,
            },
        };

        action.validate().map_err(|error| error.to_string())?;
        self.replace_next_numeric = true;
        Ok(action)
    }

    fn current_field_kind(&self) -> FieldKind {
        match (self.selected_action(), self.selected_field) {
            (ActionKind::WaterChangePercent, 1) => FieldKind::Choice,
            (ActionKind::ChangeAeration, 0) => FieldKind::Toggle,
            (ActionKind::AddShrimp, 0) | (ActionKind::RemoveShrimp, 0) => {
                FieldKind::Numeric { integer_only: true }
            }
            _ => FieldKind::Numeric {
                integer_only: false,
            },
        }
    }

    fn current_numeric_buffer_mut(&mut self) -> Option<(&mut String, bool)> {
        let integer_only = matches!(
            self.current_field_kind(),
            FieldKind::Numeric { integer_only: true }
        );

        let buffer = match (self.selected_action(), self.selected_field) {
            (ActionKind::Feed, 0) => &mut self.feed_grams,
            (ActionKind::WaterChangePercent, 0) => &mut self.water_change_percent,
            (ActionKind::TrimPlants, 0) => &mut self.trim_fraction,
            (ActionKind::SiphonDetritus, 0) => &mut self.siphon_fraction,
            (ActionKind::CleanFilter, 0) => &mut self.clean_filter_intensity,
            (ActionKind::AddShrimp, 0) => &mut self.add_shrimp_count,
            (ActionKind::RemoveShrimp, 0) => &mut self.remove_shrimp_count,
            (ActionKind::ChangePhotoperiod, 0) => &mut self.photoperiod_hours,
            (ActionKind::ChangeLightIntensity, 0) => &mut self.light_intensity,
            (ActionKind::ChangeHeaterSetpoint, 0) => &mut self.heater_setpoint_c,
            (ActionKind::ChangeAmbientTemperature, 0) => &mut self.ambient_temp_c,
            (ActionKind::ChangeAeration, 1) => &mut self.aeration_intensity,
            _ => return None,
        };

        Some((buffer, integer_only))
    }

    fn reset_selection(&mut self) {
        self.selected_field = 0;
        self.replace_next_numeric = true;
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let layout = Layout::horizontal([Constraint::Length(28), Constraint::Min(30)]).split(area);
    let actions = ActionKind::ALL
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let style = if index == app.action_form.selected_action_index() {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(action.label()).style(style)
        })
        .collect::<Vec<_>>();
    let action_list = List::new(actions).block(
        Block::default()
            .title("Action variants")
            .borders(Borders::ALL),
    );
    frame.render_widget(action_list, layout[0]);

    let field_items = app
        .action_form
        .fields()
        .into_iter()
        .enumerate()
        .map(|(index, field)| {
            let prefix = if index == app.action_form.selected_field_index() {
                ">"
            } else {
                " "
            };
            let style = if index == app.action_form.selected_field_index() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{prefix} {}: ", field.label), style),
                Span::raw(field.value),
                Span::styled(
                    format!("  ({})", field.hint),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect::<Vec<_>>();

    let right = Layout::vertical([Constraint::Min(7), Constraint::Length(5)]).split(layout[1]);
    let fields = List::new(field_items).block(
        Block::default()
            .title(format!(
                "{} form",
                app.action_form.selected_action().label()
            ))
            .borders(Borders::ALL),
    );
    frame.render_widget(fields, right[0]);

    let help = Paragraph::new(vec![
        Line::from("Up/Down change action  Left/Right change field"),
        Line::from("Type digits or '.' to edit numeric fields  Backspace deletes"),
        Line::from("[ and ] cycle source water  t toggles booleans  Enter queues action"),
    ])
    .block(Block::default().title("Controls").borders(Borders::ALL))
    .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(help, right[1]);
}

fn parse_f64(field: &'static str, raw: &str) -> Result<f64, String> {
    raw.parse::<f64>()
        .map_err(|_| format!("{field} must be a number"))
}

fn parse_u32(field: &'static str, raw: &str) -> Result<u32, String> {
    raw.parse::<u32>()
        .map_err(|_| format!("{field} must be a whole number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_water_change_action_with_profile_selection() {
        let mut form = ActionFormState::default();
        form.select_next_action();
        form.select_next_field();
        form.cycle_choice(true);

        let action = form.submit().expect("water change action should parse");
        assert_eq!(
            action,
            PlayerAction::WaterChangePercent {
                percent: 25.0,
                source_profile_id: "hard_shrimp".to_string(),
            }
        );
    }

    #[test]
    fn rejects_invalid_integer_counts() {
        let mut form = ActionFormState::default();
        for _ in 0..5 {
            form.select_next_action();
        }
        form.add_shrimp_count = "1.5".to_string();

        let error = form.submit().expect_err("count should require integers");
        assert!(error.contains("whole number"));
    }
}
