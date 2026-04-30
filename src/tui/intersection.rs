//! Simple junction status: one labeled line per approach (direction of travel).

use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use traffic_lights::direction::Direction;
use traffic_lights::signal::Signal;

use super::App;

pub fn diagram(app: &App) -> Paragraph<'static> {
    let mode = if app.auto_cycle { " auto " } else { " manual " };
    let lines = vec![
        traffic_line(
            "North Bound Traffic Light",
            app.junction.signal(Direction::South),
        ),
        traffic_line(
            "South Bound Traffic Light",
            app.junction.signal(Direction::North),
        ),
        traffic_line(
            "East Bound Traffic Light",
            app.junction.signal(Direction::West),
        ),
        traffic_line(
            "West Bound Traffic Light",
            app.junction.signal(Direction::East),
        ),
        Line::from(""),
        junction_state_line(app),
    ];

    Paragraph::new(lines)
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Line::from(vec![
                    Span::styled(" - ", Style::default().fg(Color::Cyan)),
                    Span::styled("Junction", Style::default().fg(Color::White).bold()),
                    Span::styled(mode, Style::default().fg(Color::Yellow)),
                    Span::styled(" - ", Style::default().fg(Color::Cyan)),
                ]))
                .title_alignment(Alignment::Center),
        )
}

fn traffic_line(label: &'static str, sig: Signal) -> Line<'static> {
    let status = signal_words(sig);
    Line::from(vec![
        Span::styled(
            format!("{label}: "),
            Style::default().fg(Color::Rgb(200, 200, 210)),
        ),
        styled_status(sig, status),
    ])
}

fn junction_state_line(app: &App) -> Line<'static> {
    let (text, alert) = if app.junction.is_all_off() {
        ("All signals OFF (fault / shutdown)", true)
    } else if app.junction.alert_raised() {
        ("Alert raised", true)
    } else if app.junction.ped_crossing_active() {
        ("Pedestrian crossing active", false)
    } else if app.junction.is_ped_waiting() {
        ("Pedestrian waiting", false)
    } else {
        ("—", false)
    };

    let style = if alert {
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else if text == "—" {
        Style::default().fg(Color::Rgb(120, 120, 130))
    } else {
        Style::default().fg(Color::LightCyan)
    };

    Line::from(vec![
        Span::styled("Junction state: ", Style::default().fg(Color::Rgb(200, 200, 210))),
        Span::styled(text.to_string(), style),
    ])
}

fn signal_words(sig: Signal) -> &'static str {
    match sig {
        Signal::Red => "RED",
        Signal::RedAmber => "RED + AMBER",
        Signal::Green => "GREEN",
        Signal::Amber => "AMBER",
        Signal::Off => "OFF",
    }
}

fn styled_status(sig: Signal, words: &'static str) -> Span<'static> {
    if sig == Signal::Off {
        return Span::styled(
            words.to_string(),
            Style::default()
                .fg(Color::Rgb(180, 180, 190))
                .add_modifier(Modifier::DIM),
        );
    }
    let style = match sig {
        Signal::Red => Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(200, 35, 42))
            .add_modifier(Modifier::BOLD),
        Signal::RedAmber => Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(220, 175, 40))
            .add_modifier(Modifier::BOLD),
        Signal::Green => Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(48, 185, 78))
            .add_modifier(Modifier::BOLD),
        Signal::Amber => Style::default()
            .fg(Color::Black)
            .bg(Color::Rgb(220, 175, 40))
            .add_modifier(Modifier::BOLD),
        Signal::Off => Style::default().fg(Color::Gray),
    };
    Span::styled(words.to_string(), style)
}
