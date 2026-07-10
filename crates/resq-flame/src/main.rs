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

//! `ResQ` Flame-Explorer TUI v2.0
//!
//! Robust interactive CPU profiling and flame graph generation tool.
//! Optimized for visual clarity and interaction (service selection, duration tuning, themes).

#![deny(missing_docs)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use inferno::flamegraph::{self, Options as FlamegraphOptions};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{BufReader, BufWriter, Cursor};
use std::path::{Path, PathBuf};

use resq_tui::crossterm::event::{self, KeyCode, KeyEventKind};
use resq_tui::ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};
use resq_tui::{self as tui, terminal, terminal::TuiApp, Theme};

// ─── CLI ───────────────────────────────────────────────────────────────────────

/// Generate interactive SVG flame graphs from CPU profiles.
#[derive(Parser)]
#[command(name = "flame-explorer", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Output SVG path (default: flamegraph.svg)
    #[arg(short, long, global = true, default_value = "flamegraph.svg")]
    output: PathBuf,

    /// Open the SVG in the default browser after generation
    #[arg(long, global = true)]
    open: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch a CPU profile from the coordination-hce server
    Hce {
        /// HCE server URL
        #[arg(short, long, default_value = "http://localhost:5000")]
        url: String,
        /// Profile duration in milliseconds
        #[arg(short, long, default_value_t = 5000)]
        duration: u64,
    },
}

// ─── App State ────────────────────────────────────────────────────────────────

struct App {
    services: Vec<ProfilingTarget>,
    list_state: ListState,
    theme: Theme,
    output_path: PathBuf,
    /// Set when the user presses Enter; holds the index of the selected target.
    selected_target: Option<usize>,
}

struct ProfilingTarget {
    name: String,
    cmd_type: String,
    description: String,
}

impl App {
    fn new(output_path: PathBuf) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            services: vec![
                ProfilingTarget {
                    name: "Coordination HCE".into(),
                    cmd_type: "hce".into(),
                    description: "Node.js/Bun service via HTTP metrics".into(),
                },
                ProfilingTarget {
                    name: "Infrastructure API".into(),
                    cmd_type: "api".into(),
                    description: "Rust backend via pprof".into(),
                },
                ProfilingTarget {
                    name: "Intelligence PDIE".into(),
                    cmd_type: "python".into(),
                    description: "Python AI engine via py-spy".into(),
                },
                ProfilingTarget {
                    name: "Linux Perf".into(),
                    cmd_type: "perf".into(),
                    description: "System-wide profiling via perf record".into(),
                },
            ],
            list_state,
            theme: Theme::adaptive(),
            output_path,
            selected_target: None,
        }
    }
}

// ─── TuiApp impl ─────────────────────────────────────────────────────────────

impl TuiApp for App {
    fn draw(&mut self, frame: &mut Frame) {
        draw_ui(frame, self);
    }

    fn handle_key(&mut self, key: event::KeyEvent) -> anyhow::Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(true);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(false),
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some((i + 1).min(self.services.len() - 1)));
                Ok(true)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some(i.saturating_sub(1)));
                Ok(true)
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    self.selected_target = Some(i);
                }
                Ok(false)
            }
            _ => Ok(true),
        }
    }
}

// ─── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Subcommand mode: scripted, non-interactive profiling.
    if let Some(Commands::Hce { url, duration }) = &cli.command {
        return run_hce_profile(url, *duration, &cli.output, cli.open).await;
    }

    // Interactive TUI mode: pick a target, then dispatch.
    let mut app = App::new(cli.output.clone());
    let mut guard = terminal::init()?;
    terminal::run_loop(&mut guard, 100, &mut app)?;
    drop(guard);

    if let Some(idx) = app.selected_target {
        let target = &app.services[idx];
        match target.cmd_type.as_str() {
            // HCE is the implemented engine. The TUI has no URL/duration inputs
            // yet, so it uses the same defaults as `resq-flame hce`.
            "hce" => {
                run_hce_profile("http://localhost:5000", 5000, &cli.output, cli.open).await?;
            }
            // api (pprof) / python (py-spy) / perf (perf record) are not wired up
            // yet. Bail loudly rather than exiting 0 with no flame graph.
            other => anyhow::bail!(
                "profiling for {} (engine: {}) is not yet implemented — no flame graph was \
                 written to {}. Only the `hce` engine is available today.",
                target.name,
                other,
                cli.output.display()
            ),
        }
    }

    Ok(())
}

/// Fetch a V8 CPU profile from the coordination-hce server, fold it into
/// stack-collapse format, and render an interactive SVG flame graph.
///
/// HCE samples for `duration_ms` before responding, so the window is only
/// meaningful while the service is under load — generate traffic first.
async fn run_hce_profile(
    url: &str,
    duration_ms: u64,
    output: &Path,
    open_after: bool,
) -> anyhow::Result<()> {
    let endpoint = format!("{}/admin/cpu-profile", url.trim_end_matches('/'));
    println!("🔥 Profiling HCE at {endpoint} for {duration_ms}ms (drive load now)…");

    // The server blocks for ~duration_ms while sampling, so the client timeout
    // must comfortably exceed the profiling window plus transfer time.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(duration_ms + 30_000))
        .build()
        .context("failed to build HTTP client")?;

    let mut req = client
        .post(&endpoint)
        .json(&serde_json::json!({ "durationMs": duration_ms, "intervalUs": 100 }));

    // HCE's authGuard accepts either a Bearer JWT or an `x-api-key`; admin routes
    // do not enforce a role. Prefer a token, fall back to an API key.
    match (std::env::var("RESQ_TOKEN"), std::env::var("RESQ_API_KEY")) {
        (Ok(token), _) if !token.is_empty() => req = req.bearer_auth(token),
        (_, Ok(key)) if !key.is_empty() => req = req.header("x-api-key", key),
        _ => anyhow::bail!(
            "no HCE credentials — set RESQ_TOKEN (a Bearer JWT) or RESQ_API_KEY (an x-api-key)"
        ),
    }

    let resp = req
        .send()
        .await
        .context("request to HCE /admin/cpu-profile failed")?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .context("failed to read HCE response body")?;
    if !status.is_success() {
        anyhow::bail!("HCE returned {status}: {}", body.trim());
    }

    let profile: CpuProfile =
        serde_json::from_str(&body).context("failed to parse .cpuprofile JSON from HCE")?;
    let folded = cpuprofile_to_folded(&profile);
    if folded.trim().is_empty() {
        anyhow::bail!(
            "profile contained no samples — HCE was idle during the {duration_ms}ms window; \
             drive traffic (e.g. the perf suite) while profiling and retry"
        );
    }

    generate_svg(
        &folded,
        output,
        Some("ResQ HCE — CPU Flame Graph"),
        false,
        0.1,
    )
    .map_err(|e| anyhow::anyhow!("flame graph generation failed: {e}"))?;
    println!("✅ Flame graph written to {}", output.display());

    if open_after {
        if let Err(e) = open::that(output) {
            eprintln!("warning: could not open {}: {e}", output.display());
        }
    }

    Ok(())
}

fn draw_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());
    tui::draw_header(
        f,
        chunks[0],
        "Flame-Explorer",
        "READY TO PROFILE",
        app.theme.success,
        None,
        &app.output_path.to_string_lossy(),
        &app.theme,
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .services
        .iter()
        .map(|s| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" 🔥 {:<20} ", s.name),
                    Style::default().fg(app.theme.primary).bold(),
                ),
                Span::styled(&s.cmd_type, Style::default().fg(app.theme.secondary).dim()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" PROFILING TARGETS ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(app.theme.primary)),
        )
        .highlight_style(Style::default().bg(app.theme.highlight).bold());
    f.render_stateful_widget(list, body[0], &mut app.list_state);

    if let Some(i) = app.list_state.selected() {
        let s = &app.services[i];
        let detail = vec![
            Line::from(vec![
                Span::styled("TARGET: ", Style::default().bold()),
                Span::raw(&s.name),
            ]),
            Line::from(vec![
                Span::styled("ENGINE: ", Style::default().bold()),
                Span::raw(&s.cmd_type),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "DESCRIPTION:",
                Style::default().bold().fg(app.theme.secondary),
            )]),
            Line::from(s.description.as_str()),
        ];
        f.render_widget(
            Paragraph::new(detail).block(
                Block::default()
                    .title(" TARGET DETAILS ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(app.theme.accent)),
            ),
            body[1],
        );
    }

    tui::draw_footer(
        f,
        chunks[2],
        &[
            ("Q", "Quit"),
            ("Enter", "Profile Target"),
            ("↑↓", "Navigate"),
        ],
        &app.theme,
    );
}

// Render folded stack samples into an interactive SVG flame graph via inferno.
fn generate_svg(
    folded: &str,
    output: &std::path::Path,
    title: Option<&str>,
    reverse: bool,
    min_width: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut opts = FlamegraphOptions::default();
    opts.title = title.unwrap_or("ResQ Flame Graph").to_string();
    opts.min_width = min_width;
    opts.direction = if reverse {
        flamegraph::Direction::Inverted
    } else {
        flamegraph::Direction::Straight
    };
    let reader = BufReader::new(Cursor::new(folded.as_bytes()));
    let writer = BufWriter::new(fs::File::create(output)?);
    flamegraph::from_reader(&mut opts, reader, writer).map_err(std::convert::Into::into)
}

// ─── CPU Profile Types ─────────────────────────────────────────────────────────

/// Error types for profiling operations.
#[derive(Debug)]
#[allow(dead_code)]
enum AppError {
    /// A general error.
    General(String),
    /// An error originating from perf subsystem.
    Perf(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::General(msg) => write!(f, "{msg}"),
            Self::Perf(msg) => write!(f, "Perf error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

/// A single call frame in a CPU profile node.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CallFrame {
    /// The name of the function.
    function_name: String,
    /// Source URL (may be empty or absent on synthetic/native frames).
    #[serde(default)]
    url: String,
    /// Line number in the source file. V8 emits `-1` for root/native/synthetic
    /// frames, so this is signed; the folder never reads it.
    #[serde(default)]
    line_number: i64,
}

/// A node in the CPU profile tree.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CpuNode {
    /// Unique identifier for this node.
    id: u64,
    /// The call frame data for this node (`callFrame` in the V8 JSON).
    call_frame: CallFrame,
    /// Child node IDs. Omitted (not `[]`) on leaf nodes, hence `default`.
    #[serde(default)]
    children: Vec<u64>,
}

/// A parsed V8/Chrome CPU profile.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CpuProfile {
    /// All nodes in the profile tree.
    #[serde(default)]
    nodes: Vec<CpuNode>,
    /// Leaf node id sampled at each tick.
    #[serde(default)]
    samples: Vec<u64>,
    /// Time deltas between samples in microseconds (`timeDeltas`; unused).
    #[serde(default)]
    time_deltas: Vec<i64>,
}

/// Convert a [`CpuProfile`] into folded-stack format suitable for flame graph generation.
///
/// Each unique stack trace is emitted as `frame1;frame2;...;frameN count`.
/// The `(root)` pseudo-frame is excluded and empty function names become `(anonymous)`.
#[allow(dead_code)]
fn cpuprofile_to_folded(profile: &CpuProfile) -> String {
    if profile.samples.is_empty() {
        return String::new();
    }

    // Build lookup: id -> &CpuNode
    let node_map: HashMap<u64, &CpuNode> = profile.nodes.iter().map(|n| (n.id, n)).collect();

    // Build parent map: child_id -> parent_id
    let mut parent_map: HashMap<u64, u64> = HashMap::new();
    for node in &profile.nodes {
        for &child in &node.children {
            parent_map.insert(child, node.id);
        }
    }

    // Collect stack paths for each sample
    let mut stack_counts: HashMap<String, u64> = HashMap::new();
    for &sample_id in &profile.samples {
        let mut frames: Vec<String> = Vec::new();
        let mut current = sample_id;
        loop {
            if let Some(node) = node_map.get(&current) {
                let name = if node.call_frame.function_name.is_empty() {
                    "(anonymous)".to_string()
                } else {
                    node.call_frame.function_name.clone()
                };
                if name != "(root)" {
                    frames.push(name);
                }
            }
            match parent_map.get(&current) {
                Some(&pid) => current = pid,
                None => break,
            }
        }
        frames.reverse();
        if !frames.is_empty() {
            let key = frames.join(";");
            *stack_counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut lines: Vec<String> = stack_counts
        .into_iter()
        .map(|(stack, count)| format!("{stack} {count}"))
        .collect();
    lines.sort();
    lines.join("\n")
}

/// Parse bpftrace histogram output into folded-stack format.
///
/// Each input line should be `frame1, frame2: count`. Commas are converted
/// to semicolons, producing `frame1;frame2 count`.
#[allow(dead_code)]
fn parse_bpftrace_output(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    for line in input.lines() {
        if let Some((stack, count_str)) = line.rsplit_once(':') {
            let folded = stack.trim().replace(", ", ";");
            let count = count_str.trim();
            lines.push(format!("{folded} {count}"));
        }
    }
    if lines.is_empty() {
        return Err("no valid stack traces found".into());
    }
    Ok(lines.join("\n"))
}

/// Parse pre-folded stack lines (`stack count`) into a map.
///
/// Lines with unparseable counts default to `0`.
#[allow(dead_code)]
fn parse_folded_stacks(input: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((stack, count_str)) = line.rsplit_once(' ') {
            let count = count_str.parse::<u64>().unwrap_or(0);
            map.insert(stack.to_string(), count);
        }
    }
    map
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn make_profile(nodes: Vec<CpuNode>, samples: Vec<u64>) -> CpuProfile {
        CpuProfile {
            nodes,
            samples,
            time_deltas: vec![],
        }
    }

    fn make_node(id: u64, name: &str, children: Vec<u64>) -> CpuNode {
        CpuNode {
            id,
            call_frame: CallFrame {
                function_name: name.to_string(),
                url: String::new(),
                line_number: 0,
            },
            children,
        }
    }

    #[test]
    fn cpuprofile_to_folded_simple_stack() {
        // (root) -> main -> process
        let profile = make_profile(
            vec![
                make_node(1, "(root)", vec![2]),
                make_node(2, "main", vec![3]),
                make_node(3, "process", vec![]),
            ],
            vec![3], // one sample at "process"
        );

        let folded = cpuprofile_to_folded(&profile);
        assert_eq!(folded, "main;process 1");
    }

    #[test]
    fn cpuprofile_to_folded_anonymous_function() {
        let profile = make_profile(
            vec![make_node(1, "(root)", vec![2]), make_node(2, "", vec![])],
            vec![2],
        );

        let folded = cpuprofile_to_folded(&profile);
        assert_eq!(folded, "(anonymous) 1");
    }

    #[test]
    fn cpuprofile_to_folded_multiple_samples() {
        let profile = make_profile(
            vec![
                make_node(1, "(root)", vec![2]),
                make_node(2, "main", vec![]),
            ],
            vec![2, 2, 2],
        );

        let folded = cpuprofile_to_folded(&profile);
        assert_eq!(folded, "main 3");
    }

    #[test]
    fn cpuprofile_to_folded_empty_samples() {
        let profile = make_profile(vec![make_node(1, "(root)", vec![])], vec![]);

        let folded = cpuprofile_to_folded(&profile);
        assert!(folded.is_empty());
    }

    #[test]
    fn parse_bpftrace_output_valid() {
        let input = "main, process: 42\nother, handler: 10";
        let result = parse_bpftrace_output(input).expect("Should parse valid output");
        assert!(result.contains("main;process 42"));
        assert!(result.contains("other;handler 10"));
    }

    #[test]
    fn parse_bpftrace_output_empty() {
        let result = parse_bpftrace_output("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_bpftrace_output_no_stacks() {
        let result = parse_bpftrace_output("no colons here\njust text");
        assert!(result.is_err());
    }

    #[test]
    fn parse_folded_stacks_valid() {
        let input = "main;foo 10\nbar;baz 5";
        let result = parse_folded_stacks(input);
        assert_eq!(result.get("main;foo"), Some(&10));
        assert_eq!(result.get("bar;baz"), Some(&5));
    }

    #[test]
    fn parse_folded_stacks_empty_input() {
        let result = parse_folded_stacks("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_folded_stacks_invalid_count() {
        let input = "main;foo abc";
        let result = parse_folded_stacks(input);
        // invalid count parses to 0
        assert_eq!(result.get("main;foo"), Some(&0));
    }

    #[test]
    fn app_error_display() {
        let err = AppError::General("test error".into());
        assert_eq!(format!("{err}"), "test error");

        let err = AppError::Perf("perf failed".into());
        assert_eq!(format!("{err}"), "Perf error: perf failed");
    }
}
