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
use tank_core::{Engine, SimSeed};
use tank_scenarios::{load_named_scenario, seeded_state_with_overrides, ScenarioGeometryOverrides};
use tank_tui::{
    input::{handle_key_event, InputOutcome},
    TuiApp, AUTO_ADVANCE_INTERVAL, FILL_PRESETS, TANK_SIZE_PRESETS,
};

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
    let overrides = ScenarioGeometryOverrides {
        size_scale: selection.size_preset().size_scale,
        fill_ratio: selection.fill_preset().fill_ratio,
    };
    let seed = SimSeed(seed_from_clock());
    let state = seeded_state_with_overrides(seed, selection.scenario_id(), overrides)
        .context("failed to materialize selected scenario")?;
    let scenario_label = format!(
        "{} · {} · fill {}",
        scenario.name,
        selection.size_preset().label,
        selection.fill_preset().label
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
    Launch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartupSelection {
    scenario_index: usize,
    size_index: usize,
    fill_index: usize,
    field: SelectionField,
}

impl Default for StartupSelection {
    fn default() -> Self {
        Self {
            scenario_index: 0,
            size_index: 1,
            fill_index: 1,
            field: SelectionField::Scenario,
        }
    }
}

impl StartupSelection {
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
        self.field = match self.field {
            SelectionField::Scenario => SelectionField::Size,
            SelectionField::Size => SelectionField::Fill,
            SelectionField::Fill => SelectionField::Launch,
            SelectionField::Launch => SelectionField::Scenario,
        };
    }

    fn previous_field(&mut self) {
        self.field = match self.field {
            SelectionField::Scenario => SelectionField::Launch,
            SelectionField::Size => SelectionField::Scenario,
            SelectionField::Fill => SelectionField::Size,
            SelectionField::Launch => SelectionField::Fill,
        };
    }

    fn adjust_left(&mut self) {
        match self.field {
            SelectionField::Scenario => {
                let options = tank_scenarios::default_scenario_ids();
                self.scenario_index = (self.scenario_index + options.len() - 1) % options.len();
            }
            SelectionField::Size => {
                self.size_index =
                    (self.size_index + TANK_SIZE_PRESETS.len() - 1) % TANK_SIZE_PRESETS.len();
            }
            SelectionField::Fill => {
                self.fill_index = (self.fill_index + FILL_PRESETS.len() - 1) % FILL_PRESETS.len();
            }
            SelectionField::Launch => {}
        }
    }

    fn adjust_right(&mut self) {
        match self.field {
            SelectionField::Scenario => {
                self.scenario_index =
                    (self.scenario_index + 1) % tank_scenarios::default_scenario_ids().len();
            }
            SelectionField::Size => {
                self.size_index = (self.size_index + 1) % TANK_SIZE_PRESETS.len();
            }
            SelectionField::Fill => {
                self.fill_index = (self.fill_index + 1) % FILL_PRESETS.len();
            }
            SelectionField::Launch => {}
        }
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
                KeyCode::Left => selection.adjust_left(),
                KeyCode::Right => selection.adjust_right(),
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
    let area = centered_rect(frame.size(), 76, 70);
    let layout = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(10),
        Constraint::Min(6),
        Constraint::Length(3),
    ])
    .split(area);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Tank Sim terminal UI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Choose a scenario, tank size, and fill height before launching."),
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
    frame.render_widget(options, layout[1]);

    let summary = scenario
        .map(|item| {
            vec![
                Line::from(format!("Ambient {:.1} C", item.ambient_temp_c)),
                Line::from(format!(
                    "Base geometry {:.1} x {:.1} x {:.1} cm",
                    item.tank_length_cm, item.tank_width_cm, item.fill_height_cm
                )),
                Line::from(format!("Source water {}", item.source_water_id)),
                Line::from(format!("Plants {:?}", item.plant_ids)),
                Line::from(format!("Substrates {:?}", item.substrate_ids)),
            ]
        })
        .unwrap_or_else(|| vec![Line::from("Scenario details unavailable")]);
    let summary_widget = Paragraph::new(summary)
        .block(
            Block::default()
                .title("Scenario summary")
                .borders(Borders::ALL),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(summary_widget, layout[2]);

    let help = Paragraph::new(
        "Up/Down move between fields, Left/Right change values, Enter advances, q quits.",
    )
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, layout[3]);
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
