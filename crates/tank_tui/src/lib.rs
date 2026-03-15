pub mod input;
pub mod screens;

use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Context;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Tabs},
};
use screens::actions::ActionFormState;
use tank_core::{Engine, PlayerAction, SaveFile, SimulationEngine, TankSnapshot};

pub const AUTO_ADVANCE_INTERVAL: Duration = Duration::from_millis(150);
const STATUS_TTL: Duration = Duration::from_secs(4);
const HISTORY_CAPACITY: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Overview,
    Chemistry,
    Biology,
    Actions,
    Log,
}

impl Screen {
    pub const ALL: [Self; 5] = [
        Self::Overview,
        Self::Chemistry,
        Self::Biology,
        Self::Actions,
        Self::Log,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "1 Overview",
            Self::Chemistry => "2 Chemistry",
            Self::Biology => "3 Biology",
            Self::Actions => "4 Actions",
            Self::Log => "5 Log",
        }
    }

    pub fn next(self) -> Self {
        let index = (self.index() + 1) % Self::ALL.len();
        Self::ALL[index]
    }

    pub fn from_digit(digit: char) -> Option<Self> {
        match digit {
            '1' => Some(Self::Overview),
            '2' => Some(Self::Chemistry),
            '3' => Some(Self::Biology),
            '4' => Some(Self::Actions),
            '5' => Some(Self::Log),
            _ => None,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Chemistry => 1,
            Self::Biology => 2,
            Self::Actions => 3,
            Self::Log => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TankSizePreset {
    pub label: &'static str,
    pub size_scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillPreset {
    pub label: &'static str,
    pub fill_ratio: f64,
}

pub const TANK_SIZE_PRESETS: [TankSizePreset; 3] = [
    TankSizePreset {
        label: "Compact",
        size_scale: 0.8,
    },
    TankSizePreset {
        label: "Baseline",
        size_scale: 1.0,
    },
    TankSizePreset {
        label: "Large",
        size_scale: 1.25,
    },
];

pub const FILL_PRESETS: [FillPreset; 3] = [
    FillPreset {
        label: "75%",
        fill_ratio: 0.75,
    },
    FillPreset {
        label: "90%",
        fill_ratio: 0.9,
    },
    FillPreset {
        label: "100%",
        fill_ratio: 1.0,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct TuiApp {
    pub active_screen: Screen,
    pub engine: Engine,
    pub snapshot: TankSnapshot,
    pub auto_advance: bool,
    pub status_message: Option<StatusMessage>,
    pub action_form: ActionFormState,
    pub save_file_path: PathBuf,
    pub scenario_label: String,
    snapshot_history: VecDeque<TankSnapshot>,
}

impl TuiApp {
    pub fn new(engine: Engine, save_file_path: PathBuf, scenario_label: impl Into<String>) -> Self {
        let snapshot = engine.snapshot();
        let mut snapshot_history = VecDeque::with_capacity(HISTORY_CAPACITY);
        snapshot_history.push_back(snapshot.clone());

        Self {
            active_screen: Screen::Overview,
            engine,
            snapshot,
            auto_advance: false,
            status_message: None,
            action_form: ActionFormState::default(),
            save_file_path,
            scenario_label: scenario_label.into(),
            snapshot_history,
        }
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.active_screen = screen;
    }

    pub fn next_screen(&mut self) {
        self.active_screen = self.active_screen.next();
    }

    pub fn toggle_auto_advance(&mut self) {
        self.auto_advance = !self.auto_advance;
        let state = if self.auto_advance {
            "enabled"
        } else {
            "paused"
        };
        self.set_status(StatusLevel::Info, format!("Auto-advance {state}"));
    }

    pub fn apply_action(&mut self, action: PlayerAction) {
        match self.engine.apply_action(action.clone()) {
            Ok(()) => {
                self.refresh_snapshot();
                self.set_status(StatusLevel::Info, format!("Queued action: {action:?}"));
            }
            Err(error) => {
                self.set_status(StatusLevel::Error, format!("Action rejected: {error}"));
            }
        }
    }

    pub fn step_hours(&mut self, hours: u32) {
        match self.engine.step_hours(hours) {
            Ok(()) => {
                self.refresh_snapshot();
                let label = match hours {
                    1 => "Stepped 1 hour".to_string(),
                    24 => "Stepped 1 day".to_string(),
                    168 => "Stepped 1 week".to_string(),
                    _ => format!("Stepped {hours} hours"),
                };
                self.set_status(StatusLevel::Info, label);
            }
            Err(error) => {
                self.set_status(StatusLevel::Error, format!("Step failed: {error}"));
            }
        }
    }

    pub fn save_to_disk(&mut self) -> anyhow::Result<()> {
        let save = SaveFile::from_engine(&self.engine);
        let json = save
            .to_json_pretty()
            .context("failed to serialize save file")?;
        fs::write(&self.save_file_path, json)
            .with_context(|| format!("failed to write {}", self.save_file_path.display()))?;
        self.set_status(
            StatusLevel::Info,
            format!("Saved to {}", self.save_file_path.display()),
        );
        Ok(())
    }

    pub fn load_from_disk(&mut self) -> anyhow::Result<()> {
        let raw = fs::read_to_string(&self.save_file_path)
            .with_context(|| format!("failed to read {}", self.save_file_path.display()))?;
        let save = SaveFile::from_json(&raw).context("failed to parse save file")?;
        self.engine = save.into_engine();
        self.snapshot_history.clear();
        self.refresh_snapshot();
        self.set_status(
            StatusLevel::Info,
            format!("Loaded {}", self.save_file_path.display()),
        );
        Ok(())
    }

    pub fn clear_expired_status(&mut self) {
        if self
            .status_message
            .as_ref()
            .is_some_and(|message| Instant::now() >= message.expires_at)
        {
            self.status_message = None;
        }
    }

    pub fn history_values<F>(&self, metric: F) -> Vec<f64>
    where
        F: Fn(&TankSnapshot) -> f64,
    {
        self.snapshot_history.iter().map(metric).collect()
    }

    pub fn active_warning_lines(&self) -> Vec<String> {
        self.snapshot
            .recent_events
            .iter()
            .rev()
            .filter(|event| !matches!(event.severity, tank_core::EventSeverity::Info))
            .take(4)
            .map(|event| {
                format!(
                    "Day {} {:02}:00 {:?}: {}",
                    event.day, event.hour, event.kind, event.summary
                )
            })
            .collect()
    }

    pub fn show_status(&mut self, level: StatusLevel, text: impl Into<String>) {
        self.set_status(level, text);
    }

    pub fn render(&self, frame: &mut Frame<'_>) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(frame.size());

        let titles = Screen::ALL
            .iter()
            .map(|screen| Line::from(screen.title()))
            .collect::<Vec<_>>();
        let tabs = Tabs::new(titles)
            .select(self.active_screen.index())
            .block(
                Block::default()
                    .title(format!(" Tank Sim · {} ", self.scenario_label))
                    .borders(Borders::ALL),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, chunks[0]);

        screens::render_screen(self.active_screen, frame, chunks[1], self);

        let footer = Paragraph::new(self.status_line())
            .block(Block::default().borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(footer, chunks[2]);
    }

    fn refresh_snapshot(&mut self) {
        self.snapshot = self.engine.snapshot();
        if self.snapshot_history.len() == HISTORY_CAPACITY {
            self.snapshot_history.pop_front();
        }
        self.snapshot_history.push_back(self.snapshot.clone());
    }

    fn set_status(&mut self, level: StatusLevel, text: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            level,
            expires_at: Instant::now() + STATUS_TTL,
        });
    }

    fn status_line(&self) -> Line<'static> {
        let mut spans = vec![
            Span::styled("Tab/1-5 switch", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("h", Style::default().fg(Color::Cyan)),
            Span::raw(" hour  "),
            Span::styled("d", Style::default().fg(Color::Cyan)),
            Span::raw(" day  "),
            Span::styled("w", Style::default().fg(Color::Cyan)),
            Span::raw(" week  "),
            Span::styled("space", Style::default().fg(Color::Cyan)),
            Span::raw(" auto  "),
            Span::styled("s/l", Style::default().fg(Color::Cyan)),
            Span::raw(" save/load  "),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::raw(" quit"),
        ];

        if let Some(message) = &self.status_message {
            spans.push(Span::raw("  |  "));
            let color = match message.level {
                StatusLevel::Info => Color::Green,
                StatusLevel::Warning => Color::Yellow,
                StatusLevel::Error => Color::Red,
            };
            spans.push(Span::styled(
                message.text.clone(),
                Style::default().fg(color),
            ));
        }

        Line::from(spans)
    }
}
