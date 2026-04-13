/*
 * Copyright 2026 ResQ
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! `ResQ` Health-Explorer TUI v2.1 - Optimized & Non-blocking

#![allow(clippy::pedantic)]
#![deny(missing_docs)]

mod integration;
mod services;

use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use clap::Parser;
use resq_tui::crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
};
use resq_tui::ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};
use resq_tui::{self as tui, terminal, Theme};
use services::{HealthStatus, ServiceHealth, ServiceRegistry};

/// `ResQ` Health Check Dashboard
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run a single health check and exit (for CI/scripts)
    #[arg(short, long)]
    check: bool,

    /// Poll interval in seconds
    #[arg(short, long, default_value = "5")]
    interval: u64,

    /// Run integration tests (path to script or directory)
    #[arg(short, long)]
    test: Option<String>,
}

enum AppEvent {
    Tick,
    Key(event::KeyEvent),
}

struct App {
    services: Vec<ServiceHealth>,
    last_tick: Instant,
    table_state: TableState,
    theme: Theme,
    show_details: bool,
    show_help: bool,
    is_updating: bool,
    start_time: Instant,
}

impl App {
    fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            services: Vec::new(),
            last_tick: Instant::now(),
            table_state,
            theme: Theme::default(),
            show_details: false,
            show_help: false,
            is_updating: false,
            start_time: Instant::now(),
        }
    }

    fn next(&mut self) {
        if self.services.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.services.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.services.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.services.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn selected_service(&self) -> Option<&ServiceHealth> {
        self.table_state
            .selected()
            .and_then(|i| self.services.get(i))
    }

    fn summary(&self) -> (usize, usize) {
        let healthy = self
            .services
            .iter()
            .filter(|s| s.status == HealthStatus::Healthy)
            .count();
        (healthy, self.services.len())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let poll_interval = Duration::from_secs(args.interval);

    if args.check {
        let mut registry = ServiceRegistry::new()?;
        registry.check_all().await;
        let (healthy, total) = registry.summary();
        for service in registry.services() {
            println!(
                "[{}] {} - {:?} ({}ms)",
                if service.status == HealthStatus::Healthy {
                    "✓"
                } else {
                    "✗"
                },
                service.name,
                service.status,
                service.latency_ms
            );
        }
        println!("\n{healthy}/{total} services healthy");
        std::process::exit(i32::from(healthy != total));
    }

    let mut guard = terminal::init()?;
    execute!(io::stdout(), EnableMouseCapture)?;

    let (event_tx, mut event_rx) = mpsc::channel(32);
    let (reg_tx, mut reg_rx) = mpsc::channel(1);

    // Event Loop task — polls terminal events and forwards to the main loop.
    let etx = event_tx.clone();
    tokio::spawn(async move {
        loop {
            match event::poll(Duration::from_millis(50)) {
                Ok(true) => {
                    if let Ok(Event::Key(key)) = event::read() {
                        let _ = etx.send(AppEvent::Key(key)).await;
                    }
                }
                Ok(false) => {}
                Err(_) => break, // Terminal disconnected
            }
            let _ = etx.send(AppEvent::Tick).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    // Background Updater task — polls service health endpoints.
    tokio::spawn(async move {
        let mut registry = match ServiceRegistry::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to initialize service registry: {e}");
                return;
            }
        };
        loop {
            registry.check_all().await;
            let _ = reg_tx.send(registry.services().to_vec()).await;
            tokio::time::sleep(poll_interval).await;
        }
    });

    let mut app = App::new();
    let mut result: anyhow::Result<()> = Ok(());

    loop {
        if let Err(e) = guard.draw(|f| draw_ui(f, &mut app)) {
            result = Err(e.into());
            break;
        }

        tokio::select! {
            Some(services) = reg_rx.recv() => {
                app.services = services;
                app.is_updating = false;
            }
            Some(event) = event_rx.recv() => {
                match event {
                    AppEvent::Key(key) => {
                        if key.kind != KeyEventKind::Press { continue; }
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('h') => app.show_help = !app.show_help,
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Enter => app.show_details = !app.show_details,
                            _ => {}
                        }
                    }
                    AppEvent::Tick => {
                        app.last_tick = Instant::now();
                    }
                }
            }
        }
    }

    // Cleanup is unconditional — runs even if the loop errored
    let _ = execute!(io::stdout(), DisableMouseCapture);
    drop(guard);
    result
}

fn draw_ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if app.show_help {
        tui::draw_popup(
            f,
            area,
            "HELP",
            &[
                Line::from("Q / Esc - Quit"),
                Line::from("↑↓ / JK - Navigate"),
                Line::from("Enter   - Toggle Details"),
                Line::from("H       - Close Help"),
            ],
            50,
            30,
            &app.theme,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let (healthy, total) = app.summary();
    let status_text = if total == 0 {
        "INITIALIZING...".to_string()
    } else {
        format!("{healthy}/{total} SERVICES HEALTHY")
    };
    let status_color = if total > 0 && healthy == total {
        app.theme.success
    } else if healthy > 0 {
        app.theme.warning
    } else {
        app.theme.error
    };

    tui::draw_header(
        f,
        chunks[0],
        "Health",
        &status_text,
        status_color,
        None,
        &format!("Up: {}s", app.start_time.elapsed().as_secs()),
        &app.theme,
    );

    if app.show_details {
        draw_details(f, app, chunks[1]);
    } else {
        draw_services(f, app, chunks[1]);
    }

    tui::draw_footer(
        f,
        chunks[2],
        &[("Q", "Quit"), ("↑↓", "Nav"), ("Enter", "Details")],
        &app.theme,
    );
}

fn draw_services(f: &mut Frame, app: &mut App, area: Rect) {
    let rows = app.services.iter().map(|s| {
        let status_style = match s.status {
            HealthStatus::Healthy => Style::default().fg(app.theme.success),
            HealthStatus::Degraded => Style::default().fg(app.theme.warning),
            HealthStatus::Unhealthy => Style::default().fg(app.theme.error),
            _ => Style::default().fg(app.theme.inactive),
        };

        Row::new(vec![
            Cell::from(s.name.as_str()).bold(),
            Cell::from(format!("{:?}", s.status)).style(status_style),
            Cell::from(format!("{}ms", s.latency_ms)).fg(app.theme.warning),
            Cell::from(s.error.as_deref().unwrap_or("-"))
                .fg(app.theme.inactive)
                .italic(),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["SERVICE", "STATUS", "LATENCY", "MESSAGE"])
            .fg(app.theme.primary)
            .bold()
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.inactive)),
    )
    .row_highlight_style(
        Style::default()
            .bg(app.theme.highlight)
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_details(f: &mut Frame, app: &mut App, area: Rect) {
    let service = match app.selected_service() {
        Some(s) => s,
        None => return,
    };
    let block = Block::default()
        .title(format!(" {} ", service.name.to_uppercase()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.primary));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::raw("URL:     ").bold(),
            Span::raw(&service.url).fg(app.theme.secondary),
        ]),
        Line::from(vec![
            Span::raw("STATUS:  ").bold(),
            Span::raw(format!("{:?}", service.status)).fg(app.theme.success),
        ]),
        Line::from(vec![
            Span::raw("LATENCY: ").bold(),
            Span::raw(format!("{}ms", service.latency_ms)),
        ]),
        Line::from(""),
        Line::from("DIAGNOSTICS:".bold().fg(app.theme.error)),
        Line::from(
            service
                .error
                .as_deref()
                .unwrap_or("All systems operational."),
        ),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}
