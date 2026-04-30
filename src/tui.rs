//! Ratatui + crossterm front-end for the junction demo.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction as LayoutDir, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::DefaultTerminal;

use traffic_lights::clock::SystemClock;
use traffic_lights::direction::Direction;
use traffic_lights::junction::Junction;
mod intersection;

const TICK_MS: u64 = 100;

/// Enters alternate screen, runs the interactive UI until the user quits.
pub fn run() -> io::Result<()> {
    ratatui::run(run_app)
}

fn run_app(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App::new();
    while !app.should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(LayoutDir::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(3),
                    Constraint::Length(2),
                ])
                .split(area);

            frame.render_widget(intersection::diagram(&app), chunks[0]);
            frame.render_widget(status_bar(&app), chunks[1]);
            frame.render_widget(help_bar(), chunks[2]);
        })?;

        app.junction.tick();
        app.maybe_auto_start_phase();

        if event::poll(Duration::from_millis(TICK_MS))? {
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()?
                    && key.kind == KeyEventKind::Press
                {
                    app.on_key(key.code);
                }
            }
        }
    }
    Ok(())
}

pub(super) struct App {
    junction: Junction<SystemClock>,
    next_green_is_ns: bool,
    auto_cycle: bool,
    status: String,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            junction: Junction::with_clock(SystemClock),
            next_green_is_ns: true,
            auto_cycle: true,
            status: "Ready. Press ? for keys.".into(),
            should_quit: false,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    fn maybe_auto_start_phase(&mut self) {
        if !self.auto_cycle {
            return;
        }
        if !self.junction.is_all_red() || self.junction.is_all_off() || self.junction.ped_crossing_active() {
            return;
        }

        if self.junction.is_ped_waiting() {
            if self.junction.begin_pedestrian_crossing().is_ok() {
                self.set_status("Pedestrian crossing started (auto).");
            }
            return;
        }

        let started = if self.next_green_is_ns {
            self.junction.set_competing_traffic(Direction::East, true);
            self.junction.try_advance_ns().is_ok()
        } else {
            self.junction.set_competing_traffic(Direction::North, true);
            self.junction.try_advance_ew().is_ok()
        };
        if started {
            self.next_green_is_ns = !self.next_green_is_ns;
        }
    }

    fn on_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
                self.set_status("Goodbye.");
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.junction.request_pedestrian_crossing();
                self.set_status("Pedestrian request registered.");
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                match self.junction.begin_pedestrian_crossing() {
                    Ok(()) => self.set_status("Pedestrian crossing started."),
                    Err(e) => self.set_status(format!("Cannot begin crossing: {e}")),
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.junction.end_pedestrian_crossing();
                self.set_status("Pedestrian crossing ended (demo).");
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.auto_cycle = !self.auto_cycle;
                self.set_status(if self.auto_cycle {
                    "Auto cycle ON."
                } else {
                    "Auto cycle OFF — use n / e."
                });
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                match self.junction.try_advance_ns() {
                    Ok(()) => self.set_status("Advanced NS pair."),
                    Err(e) => self.set_status(format!("NS: {e}")),
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                match self.junction.try_advance_ew() {
                    Ok(()) => self.set_status("Advanced EW pair."),
                    Err(e) => self.set_status(format!("EW: {e}")),
                }
            }
            KeyCode::Char('1') => {
                self.junction.report_sensor_fault(Direction::East);
                self.set_status("Sensor fault East (30s green, alert).");
            }
            KeyCode::Char('2') => {
                self.junction.report_light_fault(Direction::North);
                self.set_status("Light fault North — shutdown.");
            }
            KeyCode::Char('?') => {
                self.set_status(
                    "q quit · p request · c begin · d end · a auto · n NS · e EW · 1 sensor · 2 light fault",
                );
            }
            _ => {}
        }
    }
}

fn status_bar(app: &App) -> Paragraph<'_> {
    Paragraph::new(app.status.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Status "),
    )
}

fn help_bar() -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(
            "p",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " request  ".into(),
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " cross  ".into(),
        Span::styled(
            "d",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " end  ".into(),
        Span::styled(
            "n",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        "/".into(),
        Span::styled(
            "e",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " advance  ".into(),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " auto  ".into(),
        Span::styled(
            "1",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        "/".into(),
        Span::styled(
            "2",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " fault  ".into(),
        Span::styled(
            "?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " help  ".into(),
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        " quit".into(),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::Rgb(140, 140, 150)))
}
