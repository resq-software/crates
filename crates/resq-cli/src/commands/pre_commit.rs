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

//! Unified pre-commit hook logic with an optimized ratatui TUI.

use anyhow::{bail, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use resq_tui::{self as tui, Theme};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ── CLI Args ─────────────────────────────────────────────────────────────────

/// Run all pre-commit checks with TUI progress output.
#[derive(clap::Args, Debug)]
pub struct PreCommitArgs {
    /// Project root (defaults to auto-detected)
    #[arg(long, default_value = ".")]
    pub root: PathBuf,

    /// Skip security audit (osv-scanner + npm audit-ci)
    #[arg(long)]
    pub skip_audit: bool,

    /// Skip formatting step
    #[arg(long)]
    pub skip_format: bool,

    /// Skip changeset/versioning prompt
    #[arg(long)]
    pub skip_versioning: bool,

    /// Maximum file size in bytes (default: 10 MiB)
    #[arg(long, default_value_t = 10_485_760)]
    pub max_file_size: u64,

    /// Disable TUI (plain output for CI or piped stderr)
    #[arg(long)]
    pub no_tui: bool,
}

// ── Step tracking ────────────────────────────────────────────────────────────

/// The status of a pre-commit step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepStatus {
    Pending,
    Running,
    Pass,
    Warn,
    Skip,
    Fail,
}

impl StepStatus {
    fn icon(self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Running => "●",
            Self::Pass => "✅",
            Self::Warn => "⚠️ ",
            Self::Skip => "⏭️ ",
            Self::Fail => "❌",
        }
    }

    fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Pending => theme.inactive,
            Self::Running => theme.primary,
            Self::Pass => theme.success,
            Self::Warn => theme.warning,
            Self::Skip => theme.inactive,
            Self::Fail => theme.error,
        }
    }

    fn is_terminal(self) -> bool {
        matches!(self, Self::Pass | Self::Warn | Self::Skip | Self::Fail)
    }
}

/// Visible state for one step in the TUI.
#[derive(Debug, Clone)]
struct StepState {
    name: String,
    emoji: String,
    status: StepStatus,
    detail: Option<String>,
    sub_lines: Vec<String>,
    elapsed: Option<Duration>,
    is_formatter: bool,
    is_versioning: bool,
}

/// Message sent from the worker thread to the TUI render loop.
#[derive(Debug)]
enum StepMsg {
    Started(usize),
    Finished(usize, StepStatus, Option<String>, Vec<String>, Duration),
    Output(usize, String),
    PromptChangeset(usize),
    AllDone,
}

#[derive(Debug)]
enum ChangesetResponse {
    Patch,
    Minor,
    Major,
    None,
}

// ── TUI Application State ────────────────────────────────────────────────────

struct App {
    steps: Vec<StepState>,
    start_time: Instant,
    done: bool,
    aborted: bool,
    spinner_tick: usize,
    scroll_offset: usize,
    theme: Theme,

    // Versioning Prompt State
    prompting_idx: Option<usize>,
    changeset_selector: usize, // 0: none, 1: patch, 2: minor, 3: major
    changeset_message: String,
    entering_message: bool,
}

impl App {
    fn new(steps: Vec<StepState>) -> Self {
        Self {
            steps,
            start_time: Instant::now(),
            done: false,
            aborted: false,
            spinner_tick: 0,
            scroll_offset: 0,
            theme: Theme::default(),
            prompting_idx: None,
            changeset_selector: 0,
            changeset_message: String::new(),
            entering_message: false,
        }
    }

    fn apply(&mut self, msg: StepMsg) {
        match msg {
            StepMsg::Started(i) => {
                if let Some(s) = self.steps.get_mut(i) {
                    s.status = StepStatus::Running;
                }
            }
            StepMsg::Finished(i, status, detail, sub_lines, elapsed) => {
                if let Some(s) = self.steps.get_mut(i) {
                    s.status = status;
                    s.detail = detail;
                    s.sub_lines = sub_lines;
                    s.elapsed = Some(elapsed);
                }
            }
            StepMsg::Output(i, line) => {
                if let Some(s) = self.steps.get_mut(i) {
                    if s.sub_lines.len() > 10 {
                        s.sub_lines.remove(0);
                    }
                    s.sub_lines.push(line);
                }
            }
            StepMsg::PromptChangeset(i) => {
                self.prompting_idx = Some(i);
                if let Some(s) = self.steps.get_mut(i) {
                    s.status = StepStatus::Running;
                }
            }
            StepMsg::AllDone => {
                self.done = true;
            }
        }
    }

    fn counts(&self) -> (usize, usize, usize, usize) {
        let mut p = 0;
        let mut f = 0;
        let mut w = 0;
        let mut s = 0;
        for step in &self.steps {
            match step.status {
                StepStatus::Pass => p += 1,
                StepStatus::Fail => f += 1,
                StepStatus::Warn => w += 1,
                StepStatus::Skip => s += 1,
                _ => {}
            }
        }
        (p, f, w, s)
    }
}

// ── TUI Rendering ────────────────────────────────────────────────────────────

fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(5),    // Steps
        Constraint::Length(3), // Footer
    ])
    .split(size);

    draw_header(frame, chunks[0], app);
    draw_steps(frame, chunks[1], app);
    draw_footer(frame, chunks[2], app);

    if app.prompting_idx.is_some() {
        draw_prompt(frame, size, app);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let (pass, fail, _warn, _skip) = app.counts();
    let status_text = if app.done {
        if fail > 0 {
            format!("FAILED ({fail} ERROR)")
        } else {
            format!("PASSED ({pass} OK)")
        }
    } else if app.prompting_idx.is_some() {
        "WAITING FOR INPUT".to_string()
    } else {
        let done = app.steps.iter().filter(|s| s.status.is_terminal()).count();
        format!("RUNNING ({done}/{})", app.steps.len())
    };

    let status_color = if app.done {
        if fail > 0 {
            app.theme.error
        } else {
            app.theme.success
        }
    } else {
        app.theme.warning
    };

    tui::draw_header(
        frame,
        area,
        "Pre-commit",
        &status_text,
        status_color,
        None,
        &format!("{}s", app.start_time.elapsed().as_secs()),
        &app.theme,
    );
}

fn draw_steps(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.inactive))
        .title(" Verification Pipeline ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    let mut saw_formatter = false;
    let mut saw_versioning = false;

    for step in &app.steps {
        if step.is_formatter && !saw_formatter {
            saw_formatter = true;
            lines.push(Line::from(
                "  ── Formatters ──────────────────────────────────────────".fg(app.theme.inactive),
            ));
        }
        if step.is_versioning && !saw_versioning {
            saw_versioning = true;
            lines.push(Line::from(
                "  ── Versioning ──────────────────────────────────────────".fg(app.theme.inactive),
            ));
        }

        let spinner = tui::SPINNER_FRAMES[app.spinner_tick % tui::SPINNER_FRAMES.len()];
        let icon = if step.status == StepStatus::Running {
            spinner
        } else {
            step.status.icon()
        };

        let mut spans = vec![
            Span::raw("  "),
            Span::styled(
                format!("{icon} "),
                Style::default().fg(step.status.color(&app.theme)),
            ),
            Span::styled(format!("{} ", step.emoji), Style::default()),
            Span::styled(
                step.name.clone(),
                Style::default().fg(if step.status == StepStatus::Pending {
                    app.theme.inactive
                } else {
                    app.theme.fg
                }),
            ),
        ];

        if let Some(ref d) = step.detail {
            spans.push(Span::raw(format!(" {d}")).fg(app.theme.inactive));
        }

        lines.push(Line::from(spans));

        if (step.status == StepStatus::Running
            || step.status == StepStatus::Fail
            || step.status == StepStatus::Warn)
            && !step.sub_lines.is_empty()
        {
            for sub in step.sub_lines.iter().rev().take(3).rev() {
                lines.push(Line::from(
                    format!("       └─ {sub}").fg(app.theme.inactive),
                ));
            }
        }
    }

    frame.render_widget(
        Paragraph::new(lines).scroll((app.scroll_offset as u16, 0)),
        inner,
    );
}

fn draw_prompt(frame: &mut Frame, area: Rect, app: &App) {
    let prompt_area = tui::centered_rect(60, 40, area);
    frame.render_widget(ratatui::widgets::Clear, prompt_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(app.theme.primary))
        .title(" Version Bump Intent ")
        .bg(Color::Black);

    let inner = block.inner(prompt_area);
    frame.render_widget(block, prompt_area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new("Does this change require a version bump?").bold(),
        chunks[0],
    );

    let options = [" None ", " Patch ", " Minor ", " Major "];
    let spans: Vec<Span> = options
        .iter()
        .enumerate()
        .map(|(i, &opt)| {
            if i == app.changeset_selector {
                Span::styled(
                    opt,
                    Style::default()
                        .bg(app.theme.primary)
                        .fg(app.theme.bg)
                        .bold(),
                )
            } else {
                Span::raw(opt).fg(app.theme.fg)
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);

    if app.changeset_selector > 0 {
        let msg_style = if app.entering_message {
            app.theme.primary
        } else {
            app.theme.inactive
        };
        let msg_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(msg_style))
            .title(" Change Summary ");
        let msg = if app.changeset_message.is_empty() && !app.entering_message {
            "Press 'm' to add a summary..."
        } else {
            &app.changeset_message
        };
        frame.render_widget(Paragraph::new(msg).block(msg_block), chunks[3]);
    }

    let help = if app.entering_message {
        "Enter: confirm, Esc: cancel"
    } else {
        "←/→ Select, 'm' Message, Enter: Commit, Esc: Abort"
    };
    frame.render_widget(Paragraph::new(help).fg(app.theme.inactive), chunks[4]);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let keys = if app.done {
        vec![("Enter", "Exit"), ("q", "Quit")]
    } else if app.prompting_idx.is_some() {
        vec![("Enter", "Confirm"), ("Esc", "Abort")]
    } else {
        vec![("q", "Abort"), ("↑/↓", "Scroll")]
    };
    tui::draw_footer(frame, area, &keys, &app.theme);
}

// ── Logic Helpers ────────────────────────────────────────────────────────────

fn staged_files(exts: &[&str]) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .output();
    let Ok(output) = output else { return vec![] };
    if !output.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|f| exts.iter().any(|ext| f.ends_with(ext)))
        .filter(|f| !f.contains("/vendor/"))
        .map(String::from)
        .collect()
}

fn restage(files: &[String]) {
    if !files.is_empty() {
        let _ = Command::new("git").arg("add").args(files).status();
    }
}

#[allow(dead_code)]
fn has_cmd(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn self_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("resq"))
}

struct StepResult {
    status: StepStatus,
    detail: Option<String>,
    sub_lines: Vec<String>,
}

// ── Step Implementations ─────────────────────────────────────────────────────

fn step_copyright(root: &Path) -> StepResult {
    let exe = self_exe();
    let ok = Command::new(&exe)
        .arg("copyright")
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return StepResult {
            status: StepStatus::Fail,
            detail: Some("write failed".into()),
            sub_lines: vec![],
        };
    }
    let _ = Command::new("git")
        .args(["add", "-u"])
        .current_dir(root)
        .status();
    let ok = Command::new(&exe)
        .args(["copyright", "--check"])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    StepResult {
        status: if ok {
            StepStatus::Pass
        } else {
            StepStatus::Fail
        },
        detail: if ok {
            None
        } else {
            Some("headers missing".into())
        },
        sub_lines: vec![],
    }
}

fn step_large_files(max_size: u64) -> StepResult {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACM"])
        .output();
    let Ok(output) = output else {
        return StepResult {
            status: StepStatus::Skip,
            detail: None,
            sub_lines: vec![],
        };
    };
    let files = String::from_utf8_lossy(&output.stdout);
    let mut large = Vec::new();
    for f in files.lines() {
        if let Ok(m) = std::fs::metadata(f) {
            if m.len() > max_size {
                large.push(format!("{f} ({:.1} MiB)", m.len() as f64 / 1048576.0));
            }
        }
    }
    if large.is_empty() {
        StepResult {
            status: StepStatus::Pass,
            detail: None,
            sub_lines: vec![],
        }
    } else {
        StepResult {
            status: StepStatus::Fail,
            detail: Some(format!("{} files too large", large.len())),
            sub_lines: large,
        }
    }
}

fn step_debug_statements() -> StepResult {
    let files = staged_files(&[".rs", ".ts", ".tsx", ".js", ".jsx", ".py"]);
    let mut warnings = Vec::new();
    for file in &files {
        let out = Command::new("git")
            .args(["diff", "--cached", "--", file])
            .output();
        let Ok(out) = out else { continue };
        let diff = String::from_utf8_lossy(&out.stdout);
        let patterns = if file.ends_with(".py") {
            vec!["print(", "breakpoint(", "import pdb"]
        } else {
            vec!["console.log", "dbg!", "debugger;"]
        };
        for line in diff
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        {
            if patterns.iter().any(|p| line.contains(p)) {
                warnings.push(file.clone());
                break;
            }
        }
    }
    if warnings.is_empty() {
        StepResult {
            status: StepStatus::Pass,
            detail: None,
            sub_lines: vec![],
        }
    } else {
        StepResult {
            status: StepStatus::Warn,
            detail: Some(format!("{} debug stmts", warnings.len())),
            sub_lines: warnings,
        }
    }
}

fn step_secrets() -> StepResult {
    let exe = self_exe();
    let ok = Command::new(&exe)
        .args(["secrets", "--staged"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    StepResult {
        status: if ok {
            StepStatus::Pass
        } else {
            StepStatus::Fail
        },
        detail: if ok {
            None
        } else {
            Some("secrets detected".into())
        },
        sub_lines: vec![],
    }
}

fn step_audit(root: &Path, tx: &mpsc::Sender<StepMsg>, idx: usize) -> StepResult {
    let exe = self_exe();
    let mut args = vec!["audit", "--skip-react"];
    if staged_files(&["package.json", "bun.lockb", "bun.lock"]).is_empty() {
        args.push("--skip-npm");
    }
    let child = Command::new(&exe)
        .args(&args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(child) = child else {
        return StepResult {
            status: StepStatus::Fail,
            detail: Some("spawn failed".into()),
            sub_lines: vec![],
        };
    };
    let output = child.wait_with_output();
    let Ok(output) = output else {
        return StepResult {
            status: StepStatus::Fail,
            detail: Some("exec failed".into()),
            sub_lines: vec![],
        };
    };
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let interesting: Vec<String> = combined
        .lines()
        .filter(|l| l.contains("✅") || l.contains("❌") || l.contains("Vulnerabilities"))
        .map(|l| l.trim().to_string())
        .take(5)
        .collect();
    for line in &interesting {
        let _ = tx.send(StepMsg::Output(idx, line.clone()));
    }
    StepResult {
        status: if output.status.success() {
            StepStatus::Pass
        } else {
            StepStatus::Fail
        },
        detail: None,
        sub_lines: interesting,
    }
}

// Per-language format steps delegate their implementations to
// `commands::format`, so both `resq pre-commit` and `resq format` stay in
// sync. This wrapper captures the staged file list, calls the shared
// function, and restages any rewrites.
fn run_format_step<F>(root: &Path, exts: &[&str], formatter: F) -> StepResult
where
    F: FnOnce(&Path, &[String], bool) -> anyhow::Result<crate::commands::format::FormatOutcome>,
{
    let files = staged_files(exts);
    if files.is_empty() {
        return StepResult {
            status: StepStatus::Skip,
            detail: Some("no files".into()),
            sub_lines: vec![],
        };
    }
    match formatter(root, &files, false) {
        Ok(crate::commands::format::FormatOutcome::Clean)
        | Ok(crate::commands::format::FormatOutcome::Formatted) => {
            restage(&files);
            StepResult {
                status: StepStatus::Pass,
                detail: None,
                sub_lines: vec![],
            }
        }
        Ok(crate::commands::format::FormatOutcome::Skipped(reason)) => StepResult {
            status: StepStatus::Skip,
            detail: Some(reason),
            sub_lines: vec![],
        },
        Ok(crate::commands::format::FormatOutcome::Failed(stderr)) => {
            if !stderr.trim().is_empty() {
                eprintln!("{stderr}");
            }
            StepResult {
                status: StepStatus::Fail,
                detail: None,
                sub_lines: vec![],
            }
        }
        Err(e) => StepResult {
            status: StepStatus::Fail,
            detail: Some(e.to_string()),
            sub_lines: vec![],
        },
    }
}

fn step_format_rust(root: &Path) -> StepResult {
    run_format_step(root, &[".rs"], crate::commands::format::format_rust)
}

fn step_format_ts(root: &Path) -> StepResult {
    run_format_step(
        root,
        &[".ts", ".tsx", ".js", ".jsx", ".json", ".css"],
        crate::commands::format::format_ts,
    )
}

fn step_format_python(root: &Path) -> StepResult {
    run_format_step(root, &[".py"], crate::commands::format::format_python)
}

fn step_format_cpp(root: &Path) -> StepResult {
    run_format_step(
        root,
        &[".cpp", ".cc", ".h", ".hpp"],
        crate::commands::format::format_cpp,
    )
}

fn step_format_csharp(root: &Path) -> StepResult {
    run_format_step(root, &[".cs"], crate::commands::format::format_csharp)
}

// ── Worker & Main ────────────────────────────────────────────────────────────

fn build_step_list(skip_audit: bool, skip_format: bool, skip_versioning: bool) -> Vec<StepState> {
    let mut steps = vec![
        StepState {
            name: "Copyright headers".into(),
            emoji: "📝".into(),
            status: StepStatus::Pending,
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: false,
        },
        StepState {
            name: "Large file check".into(),
            emoji: "📦".into(),
            status: StepStatus::Pending,
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: false,
        },
        StepState {
            name: "Debug statements".into(),
            emoji: "🐛".into(),
            status: StepStatus::Pending,
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: false,
        },
        StepState {
            name: "Secrets scan".into(),
            emoji: "🔐".into(),
            status: StepStatus::Pending,
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: false,
        },
        StepState {
            name: "Security audit".into(),
            emoji: "🔒".into(),
            status: if skip_audit {
                StepStatus::Skip
            } else {
                StepStatus::Pending
            },
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: false,
        },
    ];
    if !skip_format {
        steps.extend(vec![
            StepState {
                name: "Format Rust".into(),
                emoji: "🦀".into(),
                status: StepStatus::Pending,
                detail: None,
                sub_lines: vec![],
                elapsed: None,
                is_formatter: true,
                is_versioning: false,
            },
            StepState {
                name: "Format TS/JS".into(),
                emoji: "🎨".into(),
                status: StepStatus::Pending,
                detail: None,
                sub_lines: vec![],
                elapsed: None,
                is_formatter: true,
                is_versioning: false,
            },
            StepState {
                name: "Format Python".into(),
                emoji: "🐍".into(),
                status: StepStatus::Pending,
                detail: None,
                sub_lines: vec![],
                elapsed: None,
                is_formatter: true,
                is_versioning: false,
            },
            StepState {
                name: "Format C++".into(),
                emoji: "⚙️ ".into(),
                status: StepStatus::Pending,
                detail: None,
                sub_lines: vec![],
                elapsed: None,
                is_formatter: true,
                is_versioning: false,
            },
            StepState {
                name: "Format C#".into(),
                emoji: "🔷".into(),
                status: StepStatus::Pending,
                detail: None,
                sub_lines: vec![],
                elapsed: None,
                is_formatter: true,
                is_versioning: false,
            },
        ]);
    }
    if !skip_versioning {
        steps.push(StepState {
            name: "Versioning / Changeset".into(),
            emoji: "🏷️".into(),
            status: StepStatus::Pending,
            detail: None,
            sub_lines: vec![],
            elapsed: None,
            is_formatter: false,
            is_versioning: true,
        });
    }
    steps
}

fn run_worker(
    tx: mpsc::Sender<StepMsg>,
    res_rx: mpsc::Receiver<ChangesetResponse>,
    root: PathBuf,
    skip_audit: bool,
    skip_format: bool,
    skip_versioning: bool,
    max_file_size: u64,
) {
    let mut idx = 0;
    macro_rules! run {
        ($step_fn:expr) => {{
            let _ = tx.send(StepMsg::Started(idx));
            let start = Instant::now();
            let res = $step_fn;
            let _ = tx.send(StepMsg::Finished(
                idx,
                res.status,
                res.detail,
                res.sub_lines,
                start.elapsed(),
            ));
            idx += 1;
        }};
    }

    run!(step_copyright(&root));
    run!(step_large_files(max_file_size));
    run!(step_debug_statements());
    run!(step_secrets());
    if skip_audit {
        idx += 1;
    } else {
        run!(step_audit(&root, &tx, idx));
    }

    if !skip_format {
        run!(step_format_rust(&root));
        run!(step_format_ts(&root));
        run!(step_format_python(&root));
        run!(step_format_cpp(&root));
        run!(step_format_csharp(&root));
    }

    if !skip_versioning {
        let _ = tx.send(StepMsg::PromptChangeset(idx));
        if let Ok(resp) = res_rx.recv() {
            let (status, detail) = match resp {
                ChangesetResponse::None => (StepStatus::Skip, Some("no bump".into())),
                _ => (StepStatus::Pass, Some("bump recorded".into())),
            };
            let _ = tx.send(StepMsg::Finished(
                idx,
                status,
                detail,
                vec![],
                Duration::ZERO,
            ));
        }
    }
    let _ = tx.send(StepMsg::AllDone);
}

/// Main entry point for the pre-commit command.
///
/// Runs verification checks and formatters with a TUI progress dashboard.
pub async fn run(args: PreCommitArgs) -> Result<()> {
    let root = if args.root == Path::new(".") {
        crate::utils::find_project_root()
    } else {
        args.root.clone()
    };
    if args.no_tui
        || !crossterm::tty::IsTty::is_tty(&io::stderr())
        || std::env::var_os("GIT_INDEX_FILE").is_some()
    {
        return run_plain(&root, args.skip_audit, args.skip_format, args.max_file_size);
    }

    let steps = build_step_list(args.skip_audit, args.skip_format, args.skip_versioning);
    let mut app = App::new(steps);
    let (tx, rx) = mpsc::channel();
    let (res_tx, res_rx) = mpsc::channel();
    let worker_root = root.clone();

    std::thread::spawn(move || {
        run_worker(
            tx,
            res_rx,
            worker_root,
            args.skip_audit,
            args.skip_format,
            args.skip_versioning,
            args.max_file_size,
        );
    });

    enable_raw_mode()?;
    io::stderr().execute(EnterAlternateScreen)?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stderr()))?;
    let mut done_at: Option<Instant> = None;

    loop {
        while let Ok(msg) = rx.try_recv() {
            app.apply(msg);
        }
        if app.done && done_at.is_none() {
            done_at = Some(Instant::now());
        }
        if let Some(t) = done_at {
            if t.elapsed() > Duration::from_millis(500)
                && !app.aborted
                && app.prompting_idx.is_none()
            {
                break;
            }
        }
        app.spinner_tick = app.spinner_tick.wrapping_add(1);
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.prompting_idx.is_some() {
                    if app.entering_message {
                        match key.code {
                            KeyCode::Char(c) => app.changeset_message.push(c),
                            KeyCode::Backspace => {
                                app.changeset_message.pop();
                            }
                            KeyCode::Enter => app.entering_message = false,
                            KeyCode::Esc => {
                                app.entering_message = false;
                                app.changeset_message.clear();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Left => {
                                app.changeset_selector = app.changeset_selector.saturating_sub(1);
                            }
                            KeyCode::Right if app.changeset_selector < 3 => {
                                app.changeset_selector += 1;
                            }
                            KeyCode::Char('m') if app.changeset_selector > 0 => {
                                app.entering_message = true;
                            }
                            KeyCode::Enter => {
                                let resp = match app.changeset_selector {
                                    1 => ChangesetResponse::Patch,
                                    2 => ChangesetResponse::Minor,
                                    3 => ChangesetResponse::Major,
                                    _ => ChangesetResponse::None,
                                };
                                if app.changeset_selector > 0 {
                                    let bump = match resp {
                                        ChangesetResponse::Patch => "patch",
                                        ChangesetResponse::Minor => "minor",
                                        _ => "major",
                                    };
                                    let _ = Command::new(self_exe())
                                        .args([
                                            "version",
                                            "add",
                                            "--bump",
                                            bump,
                                            "--message",
                                            &app.changeset_message,
                                        ])
                                        .current_dir(&root)
                                        .status();
                                }
                                let _ = res_tx.send(resp);
                            }
                            KeyCode::Esc => {
                                app.aborted = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.aborted = true;
                        break;
                    }
                    KeyCode::Enter if app.done => break,
                    KeyCode::Up => app.scroll_offset = app.scroll_offset.saturating_sub(1),
                    KeyCode::Down => app.scroll_offset = app.scroll_offset.saturating_add(1),
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    io::stderr().execute(LeaveAlternateScreen)?;
    if app.aborted {
        bail!("pre-commit aborted");
    }
    let (_, fail, _, _) = app.counts();
    if fail > 0 {
        bail!("pre-commit checks failed");
    }
    Ok(())
}

fn run_plain(root: &Path, skip_audit: bool, skip_format: bool, max_file_size: u64) -> Result<()> {
    eprintln!("🔍 ResQ Pre-commit Checks\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let mut fail = false;
    macro_rules! run {
        ($name:expr, $fn:expr) => {{
            eprint!("  {}...", $name);
            let res = $fn;
            eprintln!("\r  {} {}", res.status.icon(), $name);
            if res.status == StepStatus::Fail {
                fail = true;
            }
        }};
    }
    run!("Copyright", step_copyright(root));
    run!("Large Files", step_large_files(max_file_size));
    run!("Debug Stmts", step_debug_statements());
    run!("Secrets", step_secrets());
    if !skip_audit {
        run!("Audit", step_audit(root, &mpsc::channel().0, 0));
    }
    if !skip_format {
        run!("Format Rust", step_format_rust(root));
        run!("Format TS", step_format_ts(root));
        run!("Format Python", step_format_python(root));
        run!("Format C++", step_format_cpp(root));
        run!("Format C#", step_format_csharp(root));
    }
    if fail {
        bail!("checks failed");
    }
    Ok(())
}
