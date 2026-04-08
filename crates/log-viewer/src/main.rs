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

//! `ResQ` Log-Explorer TUI v2.0
//!
//! Robust real-time log aggregation and analysis dashboard.
//! Optimized for visual clarity and interaction (search, filtering, themes).

#![deny(missing_docs)]

mod parser;
mod sources;

use std::collections::VecDeque;
use std::path::PathBuf;

use clap::Parser;
use resq_tui::crossterm::event::{self, KeyCode, KeyEventKind};
use resq_tui::ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use resq_tui::terminal::{self, TuiApp};
use resq_tui::{self as tui, Theme};
use tokio::sync::mpsc;

use parser::{LogEntry, LogLevel};

const MAX_LOG_LINES: usize = 10_000;
const MAX_INGEST_PER_FRAME: usize = 256;

/// Aggregated log viewer for `ResQ` services.
#[derive(Parser)]
#[command(name = "log-explorer", about = "Aggregated log explorer TUI for ResQ")]
struct Args {
    /// Log source: "docker" or "file"
    #[arg(long, default_value = "docker")]
    source: String,

    /// Path for file source (directory or single file)
    #[arg(long)]
    path: Option<String>,

    /// Filter to a specific service name
    #[arg(long)]
    service: Option<String>,

    /// Minimum log level to display
    #[arg(long)]
    level: Option<String>,
}

/// Input mode for the search bar.
#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    Normal,
    Search,
}

struct App {
    logs: VecDeque<LogEntry>,
    scroll_offset: usize,
    auto_scroll: bool,
    level_filter: Option<LogLevel>,
    service_filter: Option<String>,
    search_query: String,
    input_mode: InputMode,
    search_input: String,
    theme: Theme,
    rx: mpsc::UnboundedReceiver<LogEntry>,
}

impl App {
    fn new(
        level_filter: Option<LogLevel>,
        service_filter: Option<String>,
        rx: mpsc::UnboundedReceiver<LogEntry>,
    ) -> Self {
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            scroll_offset: 0,
            auto_scroll: true,
            level_filter,
            service_filter,
            search_query: String::new(),
            input_mode: InputMode::Normal,
            search_input: String::new(),
            theme: Theme::default(),
            rx,
        }
    }

    fn push_entry(&mut self, entry: LogEntry) {
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
        self.logs.push_back(entry);
    }

    fn filtered_logs(&self) -> Vec<&LogEntry> {
        self.logs
            .iter()
            .filter(|e| {
                if let Some(min_level) = self.level_filter {
                    if e.level < min_level {
                        return false;
                    }
                }
                if let Some(ref svc) = self.service_filter {
                    if !e.service.contains(svc.as_str()) {
                        return false;
                    }
                }
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_ascii_lowercase();
                    if !e.message.to_ascii_lowercase().contains(&q)
                        && !e.service.to_ascii_lowercase().contains(&q)
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    fn cycle_level_filter(&mut self) {
        self.level_filter = match self.level_filter {
            None => Some(LogLevel::Error),
            Some(LogLevel::Error) => Some(LogLevel::Warn),
            Some(LogLevel::Warn) => Some(LogLevel::Info),
            Some(LogLevel::Info) => Some(LogLevel::Debug),
            Some(LogLevel::Debug) => Some(LogLevel::Trace),
            Some(LogLevel::Trace) => None,
        };
    }
}

impl TuiApp for App {
    fn draw(&mut self, frame: &mut resq_tui::ratatui::Frame) {
        // Drain incoming log entries before each render (capped to avoid UI stalls).
        for _ in 0..MAX_INGEST_PER_FRAME {
            match self.rx.try_recv() {
                Ok(entry) => self.push_entry(entry),
                Err(_) => break,
            }
        }
        draw_ui(frame, self);
    }

    fn handle_key(&mut self, key: event::KeyEvent) -> anyhow::Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(true);
        }

        match self.input_mode {
            InputMode::Search => match key.code {
                KeyCode::Enter => {
                    self.search_query = self.search_input.clone();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    self.search_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                }
                _ => {}
            },
            InputMode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('/') => {
                    self.input_mode = InputMode::Search;
                    self.search_input = self.search_query.clone();
                }
                KeyCode::Char('f') => self.cycle_level_filter(),
                KeyCode::Char('c') => {
                    self.logs.clear();
                    self.scroll_offset = 0;
                }
                KeyCode::Char('g') => {
                    self.auto_scroll = true;
                    self.scroll_offset = 0;
                }
                KeyCode::Up => {
                    self.auto_scroll = false;
                    self.scroll_offset += 1;
                }
                KeyCode::Down => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    if self.scroll_offset == 0 {
                        self.auto_scroll = true;
                    }
                }
                KeyCode::PageUp => {
                    self.auto_scroll = false;
                    self.scroll_offset += 20;
                }
                KeyCode::PageDown => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(20);
                    if self.scroll_offset == 0 {
                        self.auto_scroll = true;
                    }
                }
                _ => {}
            },
        }
        Ok(true)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let level_filter = args.level.as_deref().map(LogLevel::from_str_loose);

    let (tx, rx) = mpsc::unbounded_channel::<LogEntry>();

    let project_root = std::env::current_dir()?
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    match args.source.as_str() {
        "docker" => {
            sources::spawn_docker_source(project_root, args.service.clone(), tx)?;
        }
        "file" => {
            let path = args.path.map_or_else(|| PathBuf::from("."), PathBuf::from);
            sources::spawn_file_source(path, tx);
        }
        other => {
            eprintln!("Unknown source: {other}. Use 'docker' or 'file'.");
            std::process::exit(1);
        }
    }

    let mut app = App::new(level_filter, args.service.clone(), rx);
    let mut term = terminal::init()?;
    let result = terminal::run_loop(&mut term, 50, &mut app);
    terminal::restore();
    result
}

fn draw_ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Logs
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    let status_text = format!(
        "{} LOGS | FILTER: {} | MODE: {}",
        app.logs.len(),
        app.level_filter.map_or("ALL", |l| l.as_str()),
        if app.auto_scroll { "FOLLOW" } else { "SCROLL" }
    );

    tui::draw_header(
        f,
        chunks[0],
        "Log-Explorer",
        &status_text,
        if app.auto_scroll {
            app.theme.success
        } else {
            app.theme.warning
        },
        None,
        &app.search_query,
        &app.theme,
    );

    draw_logs(f, chunks[1], app);

    if app.input_mode == InputMode::Search {
        tui::draw_popup(
            f,
            f.area(),
            "SEARCH",
            &[Line::from(vec![
                Span::styled("> ", Style::default().fg(app.theme.primary).bold()),
                Span::raw(&app.search_input),
                Span::styled("▌", Style::default().fg(app.theme.primary)),
            ])],
            60,
            20,
            &app.theme,
        );
    }

    tui::draw_footer(
        f,
        chunks[2],
        &[
            ("Q", "Quit"),
            ("/", "Search"),
            ("F", "Filter"),
            ("C", "Clear"),
            ("G", "Follow"),
            ("↑↓", "Scroll"),
        ],
        &app.theme,
    );
}

fn draw_logs(f: &mut Frame, area: Rect, app: &App) {
    let filtered = app.filtered_logs();
    let visible_height = area.height.saturating_sub(2) as usize;

    let total = filtered.len();
    let start = if app.auto_scroll {
        total.saturating_sub(visible_height)
    } else {
        total.saturating_sub(visible_height + app.scroll_offset)
    };
    let end = (start + visible_height).min(total);

    let lines: Vec<Line> = filtered[start..end]
        .iter()
        .map(|entry| {
            let level_color = match entry.level {
                LogLevel::Error => app.theme.error,
                LogLevel::Warn => app.theme.warning,
                LogLevel::Info => app.theme.success,
                _ => app.theme.secondary,
            };

            let ts_str = entry.timestamp.map_or_else(
                || "--------".to_string(),
                |t| t.format("%H:%M:%S").to_string(),
            );

            let svc_color = service_color(&entry.service);

            Line::from(vec![
                Span::styled(
                    format!("{ts_str} "),
                    Style::default().fg(app.theme.primary).dim(),
                ),
                Span::styled(
                    format!("{:5} ", entry.level.as_str()),
                    Style::default().fg(level_color).bold(),
                ),
                Span::styled(
                    format!("{:>18} ", truncate_svc(&entry.service, 18)),
                    Style::default().fg(svc_color),
                ),
                Span::styled(entry.message.clone(), Style::default().fg(app.theme.fg)),
            ])
        })
        .collect();

    let log_block = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" REAL-TIME SERVICE STREAM ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.primary)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(log_block, area);
}

fn service_color(name: &str) -> Color {
    let colors = [
        Color::Cyan,
        Color::Magenta,
        Color::Blue,
        Color::Yellow,
        Color::Green,
        Color::LightRed,
        Color::LightCyan,
        Color::LightMagenta,
    ];
    let hash: usize = name.bytes().map(|b| b as usize).sum();
    colors[hash % colors.len()]
}

fn truncate_svc(name: &str, max: usize) -> String {
    if name.len() <= max {
        name.to_string()
    } else {
        format!("{}…", &name[..max - 1])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_color_is_deterministic() {
        let c1 = service_color("coordination-hce");
        let c2 = service_color("coordination-hce");
        assert_eq!(c1, c2);
    }

    #[test]
    fn truncate_short_name_unchanged() {
        assert_eq!(truncate_svc("api", 10), "api");
    }

    fn make_entry(service: &str, level: LogLevel, msg: &str) -> LogEntry {
        LogEntry {
            timestamp: None,
            service: service.to_string(),
            level,
            message: msg.to_string(),
            raw: msg.to_string(),
        }
    }

    #[test]
    fn filtered_logs_level_filter() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let mut app = App::new(Some(LogLevel::Error), None, rx);
        app.push_entry(make_entry("api", LogLevel::Info, "info msg"));
        app.push_entry(make_entry("api", LogLevel::Error, "error msg"));
        let filtered = app.filtered_logs();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "error msg");
    }
}
