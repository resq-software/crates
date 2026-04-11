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

#![allow(clippy::pedantic)]

//! # `ResQ` Performance Monitor TUI v2.0
//!
//! Enhanced real-time performance diagnostics dashboard built with Ratatui.
//! Polls the coordination-hce `/status` endpoint for metrics.
//!
//! Usage:
//!   cargo perf -- [URL] [--refresh-ms MILLISECONDS]
//!   cargo perf -- <http://localhost:5000>
//!   cargo perf -- <http://localhost:5000> --refresh-ms 1000
//!
//! Controls:
//! - `q` / `Esc` / `Ctrl+C` - Quit
//! - `r` - Reset all history
//! - `p` - Pause/Resume updates
//! - `+` / `-` - Increase/Decrease refresh rate
//! - `h` - Toggle help panel

#![deny(missing_docs)]

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use resq_tui::terminal;
use resq_tui::{format_bytes, format_duration};

use resq_tui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use resq_tui::ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table},
    Frame,
};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Metrics Data Structures (matches HCE /status endpoint output)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    uptime: String,
    #[serde(default)]
    uptime_nanoseconds: u64,
    memory: MemoryResponse,
    version: String,
    environment: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
struct MemoryResponse {
    process: ProcessMemory,
    heap: HeapMetrics,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ProcessMemory {
    rss: u64,
    heap_used: u64,
    heap_total: u64,
    external: u64,
    #[serde(default)]
    array_buffers: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct HeapMetrics {
    heap_size: u64,
    heap_capacity: u64,
    extra_memory_size: u64,
    object_count: u64,
    protected_object_count: u64,
    global_object_count: u64,
    protected_global_object_count: u64,
    #[serde(default)]
    object_type_counts: HashMap<String, u64>,
}

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

const DEFAULT_URL: &str = "http://localhost:3000/admin/status";
const DEFAULT_REFRESH_MS: u64 = 500;
const MIN_REFRESH_MS: u64 = 100;
const MAX_REFRESH_MS: u64 = 5000;
const MAX_HISTORY: usize = 120;
const MAX_ERROR_HISTORY: usize = 10;

#[derive(Debug, Clone)]
struct ErrorEntry {
    timestamp: Instant,
    message: String,
}

/// Result of a background fetch operation, sent through a channel.
enum FetchResult {
    /// Successful fetch with parsed status and round-trip latency.
    Ok {
        status: StatusResponse,
        latency_ms: u64,
    },
    /// Fetch failed with an error message and optional latency.
    Err {
        message: String,
        latency_ms: Option<u64>,
    },
}

struct App {
    /// URL being monitored (display only).
    url: String,
    status: Option<StatusResponse>,
    memory_history: VecDeque<u64>,
    latency_history: VecDeque<u64>,
    error_history: VecDeque<ErrorEntry>,
    current_error: Option<String>,
    paused: bool,
    refresh_rate_ms: u64,
    show_help: bool,
    last_latency: Option<u64>,
    success_count: u64,
    error_count: u64,
    /// Receives results from the background polling thread.
    rx: mpsc::Receiver<FetchResult>,
    /// Shared flag to signal the background thread to pause.
    poll_paused: Arc<AtomicBool>,
    /// Shared refresh rate (milliseconds) read by the background thread.
    poll_rate_ms: Arc<AtomicU64>,
}

/// Spawns a background thread that polls the status endpoint and sends
/// results through a channel. The TUI thread never blocks on network I/O.
fn spawn_poller(
    url: String,
    token: Option<String>,
    paused: Arc<AtomicBool>,
    rate_ms: Arc<AtomicU64>,
) -> Result<mpsc::Receiver<FetchResult>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("failed to create HTTP client")?;
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        loop {
            let rate = Duration::from_millis(rate_ms.load(Ordering::Relaxed));
            std::thread::sleep(rate);

            if paused.load(Ordering::Relaxed) {
                continue;
            }

            let start = Instant::now();
            let mut req = client.get(&url);
            if let Some(ref t) = &token {
                req = req.header("authorization", format!("Bearer {t}"));
            }
            let result = match req.send() {
                Ok(resp) => {
                    let latency_ms = start.elapsed().as_millis() as u64;
                    if resp.status().is_success() {
                        match resp.json::<StatusResponse>() {
                            Ok(status) => FetchResult::Ok { status, latency_ms },
                            Err(e) => FetchResult::Err {
                                message: format!("Parse error: {e}"),
                                latency_ms: Some(latency_ms),
                            },
                        }
                    } else {
                        FetchResult::Err {
                            message: format!("HTTP {}", resp.status()),
                            latency_ms: Some(latency_ms),
                        }
                    }
                }
                Err(e) => FetchResult::Err {
                    message: format!("Connection error: {e}"),
                    latency_ms: None,
                },
            };

            // If the TUI has exited, the receiver is dropped and send fails.
            if tx.send(result).is_err() {
                break;
            }
        }
    });

    Ok(rx)
}

impl App {
    fn new(url: String, token: Option<String>, refresh_rate_ms: u64) -> Result<Self> {
        let poll_paused = Arc::new(AtomicBool::new(false));
        let poll_rate_ms = Arc::new(AtomicU64::new(refresh_rate_ms));
        let rx = spawn_poller(
            url.clone(),
            token,
            Arc::clone(&poll_paused),
            Arc::clone(&poll_rate_ms),
        )?;
        Ok(Self {
            url,
            status: None,
            memory_history: VecDeque::with_capacity(MAX_HISTORY),
            latency_history: VecDeque::with_capacity(MAX_HISTORY),
            error_history: VecDeque::new(),
            current_error: None,
            paused: false,
            refresh_rate_ms,
            show_help: false,
            last_latency: None,
            success_count: 0,
            error_count: 0,
            rx,
            poll_paused,
            poll_rate_ms,
        })
    }

    /// Drains all pending results from the background poller (non-blocking).
    fn drain_updates(&mut self) {
        while let Ok(result) = self.rx.try_recv() {
            match result {
                FetchResult::Ok {
                    status,
                    latency_ms,
                } => {
                    self.last_latency = Some(latency_ms);
                    self.latency_history.push_back(latency_ms);
                    if self.latency_history.len() > MAX_HISTORY {
                        self.latency_history.pop_front();
                    }
                    self.memory_history
                        .push_back(status.memory.process.heap_used);
                    if self.memory_history.len() > MAX_HISTORY {
                        self.memory_history.pop_front();
                    }
                    self.status = Some(status);
                    self.current_error = None;
                    self.success_count += 1;
                }
                FetchResult::Err {
                    message,
                    latency_ms,
                } => {
                    if let Some(lat) = latency_ms {
                        self.last_latency = Some(lat);
                        self.latency_history.push_back(lat);
                        if self.latency_history.len() > MAX_HISTORY {
                            self.latency_history.pop_front();
                        }
                    }
                    self.record_error(message);
                }
            }
        }
    }

    fn record_error(&mut self, message: String) {
        self.error_count += 1;
        self.current_error = Some(message.clone());
        self.error_history.push_back(ErrorEntry {
            timestamp: Instant::now(),
            message,
        });
        if self.error_history.len() > MAX_ERROR_HISTORY {
            self.error_history.pop_front();
        }
    }

    fn reset(&mut self) {
        self.memory_history.clear();
        self.latency_history.clear();
        self.error_history.clear();
        self.current_error = None;
        self.success_count = 0;
        self.error_count = 0;
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.poll_paused.store(self.paused, Ordering::Relaxed);
    }

    fn increase_refresh_rate(&mut self) {
        if self.refresh_rate_ms > MIN_REFRESH_MS {
            self.refresh_rate_ms = (self.refresh_rate_ms - 100).max(MIN_REFRESH_MS);
            self.poll_rate_ms
                .store(self.refresh_rate_ms, Ordering::Relaxed);
        }
    }

    fn decrease_refresh_rate(&mut self) {
        if self.refresh_rate_ms < MAX_REFRESH_MS {
            self.refresh_rate_ms = (self.refresh_rate_ms + 100).min(MAX_REFRESH_MS);
            self.poll_rate_ms
                .store(self.refresh_rate_ms, Ordering::Relaxed);
        }
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    fn uptime_seconds(&self) -> u64 {
        if let Some(ref status) = self.status {
            status.uptime_nanoseconds / 1_000_000_000
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// TUI Rendering
// ---------------------------------------------------------------------------

// format_bytes and format_duration are imported from resq_tui

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if app.show_help {
        draw_help_overlay(frame, area);
        return;
    }

    // Main layout: header, content, footer
    let main_chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(frame, app, main_chunks[0]);

    if let Some(ref status) = app.status {
        draw_content(frame, app, status, main_chunks[1]);
    } else {
        draw_waiting(frame, app, main_chunks[1]);
    }

    draw_footer(frame, app, main_chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let status = if app.paused {
        Span::styled(" ⏸ PAUSED ", Style::default().fg(Color::Yellow).bold())
    } else if let Some(ref s) = app.status {
        Span::styled(
            format!(" ✓ {} │ {}", s.uptime, s.environment),
            Style::default().fg(Color::Green),
        )
    } else if let Some(ref err) = app.current_error {
        Span::styled(
            format!(" ⚠ {}", err.chars().take(35).collect::<String>()),
            Style::default().fg(Color::Red),
        )
    } else {
        Span::styled(" ⏳ Connecting...", Style::default().fg(Color::Yellow))
    };

    let latency_info = if let Some(lat) = app.last_latency {
        let color = if lat < 50 {
            Color::Green
        } else if lat < 200 {
            Color::Yellow
        } else {
            Color::Red
        };
        Span::styled(format!(" {lat}ms"), Style::default().fg(color))
    } else {
        Span::styled(" --ms", Style::default().fg(Color::DarkGray))
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " 🔬 ResQ Performance Monitor ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│"),
        status,
        Span::raw(" │"),
        latency_info,
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(header, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let stats = format!(
        "✓ {} │ ✗ {} │ {}ms refresh",
        app.success_count, app.error_count, app.refresh_rate_ms
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("q", Style::default().fg(Color::White).bold()),
        Span::styled(" quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("p", Style::default().fg(Color::White).bold()),
        Span::styled(" pause  ", Style::default().fg(Color::DarkGray)),
        Span::styled("r", Style::default().fg(Color::White).bold()),
        Span::styled(" reset  ", Style::default().fg(Color::DarkGray)),
        Span::styled("+/-", Style::default().fg(Color::White).bold()),
        Span::styled(" speed  ", Style::default().fg(Color::DarkGray)),
        Span::styled("h", Style::default().fg(Color::White).bold()),
        Span::styled(" help  ", Style::default().fg(Color::DarkGray)),
        Span::raw("│ "),
        Span::styled(stats, Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(footer, area);
}

fn draw_waiting(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Waiting for HCE service...",
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Polling: {}", app.url),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!("Refresh rate: {}ms", app.refresh_rate_ms),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let Some(ref err) = app.current_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Last error:",
            Style::default().fg(Color::Red).bold(),
        )));
        lines.push(Line::from(Span::styled(
            err.as_str(),
            Style::default().fg(Color::Red),
        )));
    }

    if !app.error_history.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error history ({} total):", app.error_count),
            Style::default().fg(Color::DarkGray),
        )));
        for entry in app.error_history.iter().rev().take(3) {
            let ago = entry.timestamp.elapsed().as_secs();
            lines.push(Line::from(Span::styled(
                format!("  {}s ago: {}", ago, entry.message),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Start HCE: cd services/coordination-hce && bun dev",
        Style::default().fg(Color::DarkGray),
    )));

    let msg = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Status "))
        .alignment(Alignment::Left);

    frame.render_widget(msg, area);
}

fn draw_content(frame: &mut Frame, app: &App, status: &StatusResponse, area: Rect) {
    // Split into three columns: memory, latency, heap objects
    let h_chunks = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(area);

    draw_memory(frame, app, status, h_chunks[0]);
    draw_latency(frame, app, h_chunks[1]);
    draw_heap_objects(frame, status, h_chunks[2]);
}

fn draw_memory(frame: &mut Frame, app: &App, status: &StatusResponse, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title(" Memory ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2), // Gauge
        Constraint::Length(4), // Stats
        Constraint::Min(0),    // Sparkline
    ])
    .split(inner);

    let mem = &status.memory.process;

    // Heap gauge
    let heap_ratio = if mem.heap_total > 0 {
        mem.heap_used as f64 / mem.heap_total as f64
    } else {
        0.0
    };
    let gauge_color = if heap_ratio < 0.5 {
        Color::Green
    } else if heap_ratio < 0.8 {
        Color::Yellow
    } else {
        Color::Red
    };

    let gauge = Gauge::default()
        .block(Block::default().title(Span::styled("Heap", Style::default().bold())))
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .label(format!(
            "{} / {} ({:.0}%)",
            format_bytes(mem.heap_used),
            format_bytes(mem.heap_total),
            heap_ratio * 100.0
        ))
        .ratio(heap_ratio.clamp(0.0, 1.0))
        .use_unicode(true);

    frame.render_widget(gauge, chunks[0]);

    // Stats
    let stats = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("RSS: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_bytes(mem.rss), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("External: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_bytes(mem.external),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_duration(app.uptime_seconds()),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("Version: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&status.version, Style::default().fg(Color::Cyan)),
        ]),
    ]);
    frame.render_widget(stats, chunks[1]);

    // Sparkline
    let data: Vec<u64> = app.memory_history.iter().copied().collect();
    let max_val = data.iter().copied().max().unwrap_or(1);
    let sparkline = Sparkline::default()
        .block(Block::default().title(Span::styled(
            format!("History (max: {})", format_bytes(max_val)),
            Style::default().fg(Color::DarkGray),
        )))
        .data(&data)
        .style(Style::default().fg(Color::Cyan))
        .bar_set(symbols::bar::NINE_LEVELS);

    frame.render_widget(sparkline, chunks[2]);
}

fn draw_latency(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Response Time ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).split(inner);

    // Statistics
    let latencies: Vec<u64> = app.latency_history.iter().copied().collect();
    let (avg, min, max, p95) = if latencies.is_empty() {
        (0, 0, 0, 0)
    } else {
        let sum: u64 = latencies.iter().sum();
        let avg = sum / latencies.len() as u64;
        let min = *latencies.iter().min().unwrap();
        let max = *latencies.iter().max().unwrap();

        let mut sorted = latencies.clone();
        sorted.sort_unstable();
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        let p95 = sorted
            .get(p95_idx.min(sorted.len() - 1))
            .copied()
            .unwrap_or(0);

        (avg, min, max, p95)
    };

    let stats = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Current: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}ms", app.last_latency.unwrap_or(0)),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Average: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{avg}ms"), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Min: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{min}ms"), Style::default().fg(Color::Green)),
            Span::raw("  "),
            Span::styled("Max: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{max}ms"), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("P95: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{p95}ms"), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Success: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{:.1}%",
                    if app.success_count + app.error_count > 0 {
                        (app.success_count as f64 / (app.success_count + app.error_count) as f64)
                            * 100.0
                    } else {
                        0.0
                    }
                ),
                Style::default().fg(Color::Green),
            ),
        ]),
    ]);
    frame.render_widget(stats, chunks[0]);

    // Sparkline
    let sparkline = Sparkline::default()
        .block(Block::default().title(Span::styled(
            format!("Latency (max: {max}ms)"),
            Style::default().fg(Color::DarkGray),
        )))
        .data(&latencies)
        .style(Style::default().fg(Color::Yellow))
        .bar_set(symbols::bar::NINE_LEVELS);

    frame.render_widget(sparkline, chunks[1]);
}

fn draw_heap_objects(frame: &mut Frame, status: &StatusResponse, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(" Heap Objects ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let heap = &status.memory.heap;

    // Summary + top types
    let chunks = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).split(inner);

    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Objects: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", heap.object_count),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("  "),
            Span::styled("Protected: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", heap.protected_object_count),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Global: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", heap.global_object_count),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Heap Size: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_bytes(heap.heap_size),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Capacity: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_bytes(heap.heap_capacity),
                Style::default().fg(Color::White),
            ),
        ]),
    ]);
    frame.render_widget(summary, chunks[0]);

    // Top types table
    let mut types: Vec<_> = heap.object_type_counts.iter().collect();
    types.sort_by(|a, b| b.1.cmp(a.1));

    let header = Row::new(vec!["Type", "Count", "%"])
        .style(Style::default().fg(Color::White).bold())
        .bottom_margin(1);

    let rows: Vec<Row> = types
        .into_iter()
        .take(12)
        .map(|(name, count)| {
            let pct = if heap.object_count > 0 {
                (*count as f64 / heap.object_count as f64) * 100.0
            } else {
                0.0
            };
            Row::new(vec![
                Span::styled(
                    name.chars().take(18).collect::<String>(),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!("{count}"), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{pct:.1}%"), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(55),
        Constraint::Percentage(25),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, chunks[1]);
}

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    // Create centered popup
    let popup_area = centered_rect(60, 50, area);

    // Clear background
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Help ")
        .style(Style::default().bg(Color::Black));

    frame.render_widget(block, popup_area);

    let inner = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Help ")
        .inner(popup_area);

    let help_items = vec![
        ListItem::new(Line::from(vec![
            Span::styled("q", Style::default().fg(Color::Yellow).bold()),
            Span::raw(" / "),
            Span::styled("Esc", Style::default().fg(Color::Yellow).bold()),
            Span::raw(" / "),
            Span::styled("Ctrl+C", Style::default().fg(Color::Yellow).bold()),
            Span::styled(" - Quit application", Style::default().fg(Color::White)),
        ])),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Yellow).bold()),
            Span::styled(" - Pause/Resume updates", Style::default().fg(Color::White)),
        ])),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("r", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                " - Reset all history and stats",
                Style::default().fg(Color::White),
            ),
        ])),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("+", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                " - Increase refresh rate (faster)",
                Style::default().fg(Color::White),
            ),
        ])),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("-", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                " - Decrease refresh rate (slower)",
                Style::default().fg(Color::White),
            ),
        ])),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("h", Style::default().fg(Color::Yellow).bold()),
            Span::styled(
                " - Toggle this help panel",
                Style::default().fg(Color::White),
            ),
        ])),
        ListItem::new(""),
        ListItem::new(""),
        ListItem::new(Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("h", Style::default().fg(Color::Yellow).bold()),
            Span::styled(" to close", Style::default().fg(Color::DarkGray)),
        ])),
    ];

    let list = List::new(help_items)
        .block(Block::default())
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

// ---------------------------------------------------------------------------
// Main Entry Point
// ---------------------------------------------------------------------------

/// CLI arguments for the performance monitor.
#[derive(Parser)]
#[command(name = "resq-perf")]
#[command(
    version,
    about = "Real-time performance monitoring TUI for ResQ services"
)]
struct CliArgs {
    /// Service URL to monitor
    #[arg(default_value = DEFAULT_URL)]
    url: String,

    /// Refresh rate in milliseconds
    #[arg(long, default_value_t = DEFAULT_REFRESH_MS)]
    refresh_ms: u64,

    /// Bearer token for authentication (also reads `RESQ_TOKEN` env var)
    #[arg(short, long, env = "RESQ_TOKEN")]
    token: Option<String>,
}

impl App {
    /// Handle a key event. Returns `false` to signal exit.
    fn handle_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('r') => self.reset(),
                KeyCode::Char('p') => self.toggle_pause(),
                KeyCode::Char('+' | '=') => self.increase_refresh_rate(),
                KeyCode::Char('-' | '_') => self.decrease_refresh_rate(),
                KeyCode::Char('h') => self.toggle_help(),
                _ => {}
            }
        }
        Ok(true)
    }
}

fn main() -> Result<()> {
    let args = CliArgs::parse();
    let refresh_ms = args.refresh_ms.clamp(MIN_REFRESH_MS, MAX_REFRESH_MS);

    let mut terminal = terminal::init()?;
    let mut app = App::new(args.url, args.token, refresh_ms)?;

    // Event loop: network polling runs on a background thread.
    // The main thread only handles rendering and keyboard input.
    let poll_timeout = Duration::from_millis(50);
    let result: Result<()> = (|| {
        loop {
            // 1. Drain any results from the background poller (non-blocking).
            app.drain_updates();

            // 2. Render the current state (pure, no I/O).
            terminal.draw(|f| draw(f, &app))?;

            // 3. Handle keyboard input.
            if event::poll(poll_timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        break;
                    }
                    if !app.handle_key(key)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    })();

    terminal::restore();
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── format_bytes ─────────────────────────────────────────────────────────

    #[test]
    fn format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn format_bytes_small() {
        assert_eq!(format_bytes(500), "500 B");
    }

    #[test]
    fn format_bytes_kilobytes() {
        assert_eq!(format_bytes(1536), "1.5 KiB");
    }

    #[test]
    fn format_bytes_megabytes() {
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MiB");
    }

    #[test]
    fn format_bytes_gigabytes() {
        let gb = (2.5 * 1024.0 * 1024.0 * 1024.0) as u64;
        let result = format_bytes(gb);
        assert!(result.contains("GiB"), "Expected GiB, got {result}");
    }

    // ── format_duration ──────────────────────────────────────────────────────

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(30), "30s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(90), "1m 30s");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(3661), "1h 1m 1s");
    }

    #[test]
    fn format_duration_days() {
        assert_eq!(format_duration(90061), "1d 1h 1m");
    }

    // ── App state machine ────────────────────────────────────────────────────

    #[test]
    fn app_new_defaults() {
        let app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        assert!(!app.paused);
        assert_eq!(app.success_count, 0);
        assert_eq!(app.error_count, 0);
        assert!(app.memory_history.is_empty());
        assert!(app.latency_history.is_empty());
        assert!(app.status.is_none());
    }

    #[test]
    fn app_toggle_pause() {
        let mut app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        assert!(!app.paused);
        app.toggle_pause();
        assert!(app.paused);
        app.toggle_pause();
        assert!(!app.paused);
    }

    #[test]
    fn app_increase_refresh_rate_clamps_at_min() {
        let mut app = App::new("http://localhost:3000".into(), None, MIN_REFRESH_MS).unwrap();
        app.increase_refresh_rate();
        assert_eq!(app.refresh_rate_ms, MIN_REFRESH_MS);
    }

    #[test]
    fn app_decrease_refresh_rate_clamps_at_max() {
        let mut app = App::new("http://localhost:3000".into(), None, MAX_REFRESH_MS).unwrap();
        app.decrease_refresh_rate();
        assert_eq!(app.refresh_rate_ms, MAX_REFRESH_MS);
    }

    #[test]
    fn app_increase_refresh_rate_decreases_ms() {
        let mut app = App::new("http://localhost:3000".into(), None, 500).unwrap();
        app.increase_refresh_rate();
        assert_eq!(app.refresh_rate_ms, 400);
    }

    #[test]
    fn app_decrease_refresh_rate_increases_ms() {
        let mut app = App::new("http://localhost:3000".into(), None, 500).unwrap();
        app.decrease_refresh_rate();
        assert_eq!(app.refresh_rate_ms, 600);
    }

    #[test]
    fn app_reset_clears_state() {
        let mut app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        app.record_error("test error".into());
        app.success_count = 5;
        app.memory_history.push_back(1234);
        app.latency_history.push_back(100);

        app.reset();

        assert!(app.memory_history.is_empty());
        assert!(app.latency_history.is_empty());
        assert!(app.error_history.is_empty());
        assert!(app.current_error.is_none());
        assert_eq!(app.success_count, 0);
        assert_eq!(app.error_count, 0);
    }

    #[test]
    fn app_record_error_increments_count() {
        let mut app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        app.record_error("err 1".into());
        app.record_error("err 2".into());
        assert_eq!(app.error_count, 2);
        assert_eq!(app.error_history.len(), 2);
        assert_eq!(app.current_error, Some("err 2".into()));
    }

    #[test]
    fn app_record_error_caps_history() {
        let mut app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        for i in 0..MAX_ERROR_HISTORY + 5 {
            app.record_error(format!("err {i}"));
        }
        assert_eq!(app.error_history.len(), MAX_ERROR_HISTORY);
    }

    #[test]
    fn app_uptime_seconds_without_status() {
        let app = App::new("http://localhost:3000".into(), None, DEFAULT_REFRESH_MS).unwrap();
        assert_eq!(app.uptime_seconds(), 0);
    }

    #[test]
    fn app_refresh_rate() {
        let app = App::new("http://localhost:3000".into(), None, 500).unwrap();
        assert_eq!(app.refresh_rate_ms, 500);
    }

    #[test]
    fn parse_full_status_response() {
        let json = r#"{
            "uptime": "1d 2h 30m 15s",
            "uptimeNanoseconds": 95415000000000,
            "memory": {
                "process": {
                    "rss": 245694464,
                    "heapUsed": 67108864,
                    "heapTotal": 134217728,
                    "external": 8388608,
                    "arrayBuffers": 1024
                },
                "heap": {
                    "heapSize": 67108864,
                    "heapCapacity": 134217728,
                    "extraMemorySize": 0,
                    "objectCount": 12345,
                    "protectedObjectCount": 100,
                    "globalObjectCount": 50,
                    "protectedGlobalObjectCount": 5,
                    "objectTypeCounts": { "Object": 5000, "Array": 3000 }
                }
            },
            "version": "2.1.0",
            "environment": "production"
        }"#;
        let status: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(status.uptime, "1d 2h 30m 15s");
        assert_eq!(status.uptime_nanoseconds, 95_415_000_000_000);
        assert_eq!(status.memory.process.rss, 245_694_464);
        assert_eq!(status.memory.process.heap_used, 67_108_864);
        assert_eq!(status.memory.heap.object_count, 12345);
        assert_eq!(status.version, "2.1.0");
        assert_eq!(status.environment, "production");
    }

    #[test]
    fn parse_minimal_status_response() {
        // Missing optional fields should use defaults
        let json = r#"{
            "uptime": "0s",
            "memory": {
                "process": { "rss": 0, "heapUsed": 0, "heapTotal": 0, "external": 0 },
                "heap": {
                    "heapSize": 0, "heapCapacity": 0, "extraMemorySize": 0,
                    "objectCount": 0, "protectedObjectCount": 0,
                    "globalObjectCount": 0, "protectedGlobalObjectCount": 0
                }
            },
            "version": "0.0.0",
            "environment": "test"
        }"#;
        let status: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(status.uptime_nanoseconds, 0); // default
        assert_eq!(status.memory.process.array_buffers, 0); // default
    }

    #[test]
    fn parse_status_with_extra_fields() {
        // Unknown fields should be silently ignored
        let json = r#"{
            "uptime": "1s",
            "uptimeNanoseconds": 1000000000,
            "memory": {
                "process": { "rss": 100, "heapUsed": 50, "heapTotal": 200, "external": 10 },
                "heap": {
                    "heapSize": 50, "heapCapacity": 200, "extraMemorySize": 0,
                    "objectCount": 1, "protectedObjectCount": 0,
                    "globalObjectCount": 0, "protectedGlobalObjectCount": 0
                }
            },
            "version": "1.0.0",
            "environment": "dev",
            "unknownField": "should be ignored",
            "anotherExtra": 42
        }"#;
        let status: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(status.version, "1.0.0");
    }
}
