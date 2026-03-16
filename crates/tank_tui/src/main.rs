use std::{
    io::{self, Stdout},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use tank_core::{Engine, SimSeed, TankGeometry};
use tank_scenarios::{
    load_named_scenario, seeded_state_with_full_overrides, startup_defaults_for_scenario,
    startup_source_water_ids, StartupHeaterPreset, StartupLightPreset, StartupOverrides,
    StartupPlantSelection, StartupSubstratePreset,
};
use tank_tui::{
    input::{handle_key_event, InputOutcome},
    TuiApp, AUTO_ADVANCE_INTERVAL, FILL_PRESETS, TANK_SIZE_PRESETS,
};

const STARTUP_SHRIMP_COUNTS: [u32; 5] = [0, 5, 10, 15, 20];

fn main() -> anyhow::Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;

    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let Some(selection) = select_startup_config(terminal)? else {
        return Ok(());
    };

    let scenario =
        load_named_scenario(selection.scenario_id()).context("failed to load scenario")?;
    let seed = SimSeed(seed_from_clock());
    let state = seeded_state_with_full_overrides(
        seed,
        selection.scenario_id(),
        selection.startup_overrides(),
    )
    .context("failed to materialize selected scenario")?;
    let scenario_label = format!(
        "{} · {} · fill {} · {} shrimp",
        scenario.name,
        selection.size_preset().label,
        selection.fill_preset().label,
        selection.initial_shrimp_count
    );
    let save_file_path = default_save_path(selection.scenario_id());
    let engine = Engine::from_parts(state, vec![]);
    let mut app = TuiApp::new(engine, save_file_path, scenario_label);

    loop {
        app.clear_expired_status();
        terminal
            .draw(|frame| app.render(frame))
            .context("failed to render TUI frame")?;

        if event::poll(AUTO_ADVANCE_INTERVAL).context("failed to poll for events")? {
            if let Event::Key(key) = event::read().context("failed to read terminal event")? {
                if key.kind == KeyEventKind::Press
                    && handle_key_event(&mut app, key) == InputOutcome::Quit
                {
                    break;
                }
            }
        } else if app.auto_advance {
            app.step_hours(1);
        }
    }

    Ok(())
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn default_save_path(scenario_id: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(format!("{scenario_id}-save.json"))
}

fn seed_from_clock() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionField {
    Scenario,
    Size,
    Fill,
    SourceWater,
    Substrate,
    Plants,
    Filter,
    Light,
    Heater,
    Aeration,
    ShrimpCount,
    Launch,
}

impl SelectionField {
    const ALL: [Self; 12] = [
        Self::Scenario,
        Self::Size,
        Self::Fill,
        Self::SourceWater,
        Self::Substrate,
        Self::Plants,
        Self::Filter,
        Self::Light,
        Self::Heater,
        Self::Aeration,
        Self::ShrimpCount,
        Self::Launch,
    ];

    fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StartupSelection {
    scenario_index: usize,
    size_index: usize,
    fill_index: usize,
    source_water_id: String,
    substrate_preset: StartupSubstratePreset,
    plant_selection: StartupPlantSelection,
    filter_enabled: bool,
    light_preset: StartupLightPreset,
    heater_preset: StartupHeaterPreset,
    aeration_enabled: bool,
    initial_shrimp_count: u32,
    field: SelectionField,
}

impl Default for StartupSelection {
    fn default() -> Self {
        let mut selection = Self {
            scenario_index: 0,
            size_index: 1,
            fill_index: 1,
            source_water_id: String::new(),
            substrate_preset: StartupSubstratePreset::InertSand,
            plant_selection: StartupPlantSelection::FastStemOnly,
            filter_enabled: true,
            light_preset: StartupLightPreset::Hours8,
            heater_preset: StartupHeaterPreset::Off,
            aeration_enabled: false,
            initial_shrimp_count: 0,
            field: SelectionField::Scenario,
        };
        selection
            .reset_template_fields()
            .expect("default startup scenario should load");
        selection
    }
}

fn cycle_choice<T: Copy + PartialEq>(current: &mut T, options: &[T], forward: bool) {
    let index = options
        .iter()
        .position(|option| *option == *current)
        .unwrap_or(0);
    let next_index = if forward {
        (index + 1) % options.len()
    } else {
        (index + options.len() - 1) % options.len()
    };
    *current = options[next_index];
}

fn on_off(enabled: bool) -> &'static str {
    if enabled {
        "On"
    } else {
        "Off"
    }
}

fn source_water_label(id: &str) -> String {
    id.replace('_', " ")
}

impl StartupSelection {
    fn reset_template_fields(&mut self) -> anyhow::Result<()> {
        let defaults = startup_defaults_for_scenario(self.scenario_id()).with_context(|| {
            format!("failed to load startup defaults for {}", self.scenario_id())
        })?;

        self.source_water_id = defaults
            .source_water_profile_id
            .unwrap_or_else(|| "moderate".to_string());
        self.substrate_preset = defaults
            .substrate_preset
            .unwrap_or(StartupSubstratePreset::InertSand);
        self.plant_selection = defaults
            .plant_selection
            .unwrap_or(StartupPlantSelection::FastStemOnly);
        self.filter_enabled = defaults.filter_enabled.unwrap_or(true);
        self.light_preset = defaults.light_preset.unwrap_or(StartupLightPreset::Hours8);
        self.heater_preset = defaults.heater_preset.unwrap_or(StartupHeaterPreset::Off);
        self.aeration_enabled = defaults.aeration_enabled.unwrap_or(false);
        self.initial_shrimp_count = defaults.initial_adult_shrimp_count.unwrap_or(0);
        Ok(())
    }

    fn set_scenario_index(&mut self, scenario_index: usize) -> anyhow::Result<()> {
        self.scenario_index = scenario_index;
        self.reset_template_fields()
    }

    fn startup_overrides(&self) -> StartupOverrides {
        StartupOverrides {
            geometry: tank_scenarios::ScenarioGeometryOverrides {
                size_scale: self.size_preset().size_scale,
                fill_ratio: self.fill_preset().fill_ratio,
            },
            source_water_profile_id: Some(self.source_water_id.clone()),
            substrate_preset: Some(self.substrate_preset),
            plant_selection: Some(self.plant_selection),
            filter_enabled: Some(self.filter_enabled),
            light_preset: Some(self.light_preset),
            heater_preset: Some(self.heater_preset),
            aeration_enabled: Some(self.aeration_enabled),
            initial_adult_shrimp_count: Some(self.initial_shrimp_count),
        }
    }

    fn effective_geometry(
        &self,
        tank_length_cm: f64,
        tank_width_cm: f64,
        tank_height_cm: f64,
        fill_height_cm: f64,
    ) -> TankGeometry {
        let size_scale = self.size_preset().size_scale;
        let length_cm = tank_length_cm * size_scale;
        let width_cm = tank_width_cm * size_scale;
        let height_cm = tank_height_cm * size_scale;
        let fill_height_cm =
            (fill_height_cm * size_scale * self.fill_preset().fill_ratio).min(height_cm);
        TankGeometry {
            length_cm,
            width_cm,
            height_cm,
            fill_height_cm,
            glass_thickness_mm: 5.0,
            open_top: true,
            lid_exchange_factor: 0.25,
        }
    }

    fn scenario_id(&self) -> &'static str {
        tank_scenarios::default_scenario_ids()[self.scenario_index]
    }

    fn size_preset(&self) -> tank_tui::TankSizePreset {
        TANK_SIZE_PRESETS[self.size_index]
    }

    fn fill_preset(&self) -> tank_tui::FillPreset {
        FILL_PRESETS[self.fill_index]
    }

    fn next_field(&mut self) {
        self.field = self.field.next();
    }

    fn previous_field(&mut self) {
        self.field = self.field.previous();
    }

    fn adjust_left(&mut self) -> anyhow::Result<()> {
        match self.field {
            SelectionField::Scenario => {
                let options = tank_scenarios::default_scenario_ids();
                let next_index = (self.scenario_index + options.len() - 1) % options.len();
                self.set_scenario_index(next_index)?;
            }
            SelectionField::Size => {
                self.size_index =
                    (self.size_index + TANK_SIZE_PRESETS.len() - 1) % TANK_SIZE_PRESETS.len();
            }
            SelectionField::Fill => {
                self.fill_index = (self.fill_index + FILL_PRESETS.len() - 1) % FILL_PRESETS.len();
            }
            SelectionField::SourceWater => {
                let options = startup_source_water_ids();
                let current_index = options
                    .iter()
                    .position(|id| *id == self.source_water_id)
                    .unwrap_or(0);
                let next_index = (current_index + options.len() - 1) % options.len();
                self.source_water_id = options[next_index].to_string();
            }
            SelectionField::Substrate => {
                cycle_choice(
                    &mut self.substrate_preset,
                    &StartupSubstratePreset::ALL,
                    false,
                );
            }
            SelectionField::Plants => {
                cycle_choice(
                    &mut self.plant_selection,
                    &StartupPlantSelection::ALL,
                    false,
                );
            }
            SelectionField::Filter => self.filter_enabled = !self.filter_enabled,
            SelectionField::Light => {
                cycle_choice(&mut self.light_preset, &StartupLightPreset::ALL, false);
            }
            SelectionField::Heater => {
                cycle_choice(&mut self.heater_preset, &StartupHeaterPreset::ALL, false);
            }
            SelectionField::Aeration => self.aeration_enabled = !self.aeration_enabled,
            SelectionField::ShrimpCount => {
                cycle_choice(
                    &mut self.initial_shrimp_count,
                    &STARTUP_SHRIMP_COUNTS,
                    false,
                );
            }
            SelectionField::Launch => {}
        }
        Ok(())
    }

    fn adjust_right(&mut self) -> anyhow::Result<()> {
        match self.field {
            SelectionField::Scenario => {
                let next_index =
                    (self.scenario_index + 1) % tank_scenarios::default_scenario_ids().len();
                self.set_scenario_index(next_index)?;
            }
            SelectionField::Size => {
                self.size_index = (self.size_index + 1) % TANK_SIZE_PRESETS.len();
            }
            SelectionField::Fill => {
                self.fill_index = (self.fill_index + 1) % FILL_PRESETS.len();
            }
            SelectionField::SourceWater => {
                let options = startup_source_water_ids();
                let current_index = options
                    .iter()
                    .position(|id| *id == self.source_water_id)
                    .unwrap_or(0);
                let next_index = (current_index + 1) % options.len();
                self.source_water_id = options[next_index].to_string();
            }
            SelectionField::Substrate => {
                cycle_choice(
                    &mut self.substrate_preset,
                    &StartupSubstratePreset::ALL,
                    true,
                );
            }
            SelectionField::Plants => {
                cycle_choice(&mut self.plant_selection, &StartupPlantSelection::ALL, true);
            }
            SelectionField::Filter => self.filter_enabled = !self.filter_enabled,
            SelectionField::Light => {
                cycle_choice(&mut self.light_preset, &StartupLightPreset::ALL, true);
            }
            SelectionField::Heater => {
                cycle_choice(&mut self.heater_preset, &StartupHeaterPreset::ALL, true);
            }
            SelectionField::Aeration => self.aeration_enabled = !self.aeration_enabled,
            SelectionField::ShrimpCount => {
                cycle_choice(&mut self.initial_shrimp_count, &STARTUP_SHRIMP_COUNTS, true);
            }
            SelectionField::Launch => {}
        }
        Ok(())
    }
}

fn select_startup_config(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> anyhow::Result<Option<StartupSelection>> {
    let mut selection = StartupSelection::default();

    loop {
        terminal.draw(|frame| render_startup(frame, &selection))?;

        if let Event::Key(key) = event::read().context("failed to read selection input")? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') => return Ok(None),
                KeyCode::Up => selection.previous_field(),
                KeyCode::Down => selection.next_field(),
                KeyCode::Left => selection.adjust_left()?,
                KeyCode::Right => selection.adjust_right()?,
                KeyCode::Enter if selection.field == SelectionField::Launch => {
                    return Ok(Some(selection));
                }
                KeyCode::Enter => selection.next_field(),
                _ => {}
            }
        }
    }
}

fn render_startup(frame: &mut Frame<'_>, selection: &StartupSelection) {
    let area = centered_rect(frame.size(), 92, 88);
    let layout = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(16),
        Constraint::Length(3),
    ])
    .split(area);
    let body = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(layout[1]);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Tank Sim terminal UI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(
            "Choose a scenario template, then override water, planting, hardware, and starting stock.",
        ),
    ])
    .block(Block::default().title("Startup").borders(Borders::ALL));
    frame.render_widget(header, layout[0]);

    let scenario = load_named_scenario(selection.scenario_id()).ok();
    let fields = vec![
        startup_item(
            selection.field == SelectionField::Scenario,
            "Scenario",
            scenario
                .as_ref()
                .map(|item| format!("{} ({})", item.name, item.id))
                .unwrap_or_else(|| selection.scenario_id().to_string()),
        ),
        startup_item(
            selection.field == SelectionField::Size,
            "Tank size",
            selection.size_preset().label.to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Fill,
            "Fill height",
            selection.fill_preset().label.to_string(),
        ),
        startup_item(
            selection.field == SelectionField::SourceWater,
            "Source water",
            selection.source_water_id.clone(),
        ),
        startup_item(
            selection.field == SelectionField::Substrate,
            "Substrate",
            selection.substrate_preset.label().to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Plants,
            "Plants",
            selection.plant_selection.label().to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Filter,
            "Filter",
            on_off(selection.filter_enabled).to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Light,
            "Light",
            selection.light_preset.label().to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Heater,
            "Heater",
            selection.heater_preset.label().to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Aeration,
            "Aeration",
            on_off(selection.aeration_enabled).to_string(),
        ),
        startup_item(
            selection.field == SelectionField::ShrimpCount,
            "Initial shrimp",
            selection.initial_shrimp_count.to_string(),
        ),
        startup_item(
            selection.field == SelectionField::Launch,
            "Launch",
            "Press Enter to start".to_string(),
        ),
    ];
    let options = List::new(fields).block(
        Block::default()
            .title("Configuration")
            .borders(Borders::ALL),
    );
    frame.render_widget(options, body[0]);

    let summary = scenario
        .map(|item| {
            let geometry = selection.effective_geometry(
                item.tank_length_cm,
                item.tank_width_cm,
                item.tank_height_cm,
                item.fill_height_cm,
            );
            vec![
                Line::from(format!("Scenario template {} ({})", item.name, item.id)),
                Line::from(format!("Ambient {:.1} C", item.ambient_temp_c)),
                Line::from(format!(
                    "Geometry {:.1} x {:.1} x {:.1} cm · fill {:.1} cm",
                    geometry.length_cm,
                    geometry.width_cm,
                    geometry.height_cm,
                    geometry.fill_height_cm
                )),
                Line::from(format!("Water volume {:.1} L", geometry.water_volume_l())),
                Line::from(format!(
                    "Source water {} ({})",
                    source_water_label(&selection.source_water_id),
                    selection.source_water_id
                )),
                Line::from(format!("Substrate {}", selection.substrate_preset.label())),
                Line::from(format!("Plants {}", selection.plant_selection.label())),
                Line::from(format!("Filter {}", on_off(selection.filter_enabled))),
                Line::from(format!(
                    "Light photoperiod {}",
                    selection.light_preset.label()
                )),
                Line::from(format!("Heater {}", selection.heater_preset.label())),
                Line::from(format!("Aeration {}", on_off(selection.aeration_enabled))),
                Line::from(format!(
                    "Initial shrimp {} adults",
                    selection.initial_shrimp_count
                )),
                Line::from("Changing Scenario resets all override fields to that template."),
            ]
        })
        .unwrap_or_else(|| vec![Line::from("Scenario details unavailable")]);
    let summary_widget = Paragraph::new(summary)
        .block(
            Block::default()
                .title("Launch summary")
                .borders(Borders::ALL),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(summary_widget, body[1]);

    let help = Paragraph::new(
        "Up/Down move between fields, Left/Right change values, Enter advances, q quits.",
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, layout[2]);
}

fn startup_item(selected: bool, label: &str, value: String) -> ListItem<'static> {
    let style = if selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    ListItem::new(Line::from(vec![
        Span::styled(format!("{label}: "), style),
        Span::raw(value),
    ]))
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - height_percent) / 2),
        Constraint::Percentage(height_percent),
        Constraint::Percentage((100 - height_percent) / 2),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - width_percent) / 2),
        Constraint::Percentage(width_percent),
        Constraint::Percentage((100 - width_percent) / 2),
    ])
    .split(vertical[1]);
    horizontal[1]
}
