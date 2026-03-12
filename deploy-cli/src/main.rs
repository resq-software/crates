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

//! `ResQ` Deploy-Explorer TUI v2.0
//!
//! Robust interactive deployment manager for `ResQ` infrastructure.
//! Optimized for visual clarity and interaction (stateful actions, logs, themes).

#![deny(missing_docs)]

#[allow(unreachable_pub)]
mod docker;
#[allow(unreachable_pub)]
mod k8s;

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use resq_tui::{self as tui, Theme};
use tokio::sync::mpsc;

const SERVICES: &[&str] = &[
    "infrastructure-api",
    "coordination-hce",
    "intelligence-pdie",
    "web-dashboard",
];

const DOCKER_ACTIONS: &[&str] = &["status", "build", "up", "down", "restart", "logs"];
const K8S_ACTIONS: &[&str] = &["status", "deploy", "destroy", "logs"];

/// Interactive deployment manager for `ResQ` infrastructure.
#[derive(Parser)]
#[command(name = "deploy-explorer", about = "Deployment explorer TUI for ResQ")]
struct Args {
    /// Run a single action non-interactively
    #[arg(long)]
    action: Option<String>,

    /// Target environment: dev, staging, prod
    #[arg(long, default_value = "dev")]
    env: String,

    /// Target a specific service
    #[arg(long)]
    service: Option<String>,

    /// Use Kubernetes instead of Docker Compose
    #[arg(long)]
    k8s: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Services,
    Actions,
}

struct App {
    env: String,
    use_k8s: bool,
    focus: Focus,
    service_state: ListState,
    action_state: ListState,
    containers: Vec<docker::ContainerStatus>,
    output_lines: VecDeque<String>,
    project_root: PathBuf,
    theme: Theme,
}

impl App {
    fn new(env: String, use_k8s: bool, project_root: PathBuf) -> Self {
        let mut service_state = ListState::default();
        service_state.select(Some(0));
        let mut action_state = ListState::default();
        action_state.select(Some(0));

        Self {
            env,
            use_k8s,
            focus: Focus::Services,
            service_state,
            action_state,
            containers: Vec::new(),
            output_lines: VecDeque::with_capacity(500),
            project_root,
            theme: Theme::default(),
        }
    }

    const fn actions(&self) -> &[&str] {
        if self.use_k8s {
            K8S_ACTIONS
        } else {
            DOCKER_ACTIONS
        }
    }

    fn selected_service(&self) -> Option<&str> {
        self.service_state.selected().map(|i| SERVICES[i])
    }

    fn selected_action(&self) -> Option<&str> {
        self.action_state.selected().map(|i| self.actions()[i])
    }

    fn cycle_env(&mut self) {
        self.env = match self.env.as_str() {
            "dev" => "staging".into(),
            "staging" => "prod".into(),
            _ => "dev".into(),
        };
    }

    fn refresh_status(&mut self) {
        if self.use_k8s {
            self.containers = k8s::get_status(&self.env);
        } else {
            self.containers = docker::get_status(&self.project_root, &self.env);
        }
    }

    fn push_output(&mut self, line: String) {
        if self.output_lines.len() >= 500 {
            self.output_lines.pop_front();
        }
        self.output_lines.push_back(line);
    }

    fn move_selection(&mut self, delta: i32) {
        match self.focus {
            Focus::Services => {
                let cur = self.service_state.selected().unwrap_or(0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let next = (cur as i32 + delta).rem_euclid(SERVICES.len() as i32) as usize;
                self.service_state.select(Some(next));
            },
            Focus::Actions => {
                let cur = self.action_state.selected().unwrap_or(0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let next = (cur as i32 + delta).rem_euclid(self.actions().len() as i32) as usize;
                self.action_state.select(Some(next));
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let project_root = std::env::current_dir()?
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    if let Some(action) = &args.action {
        return run_non_interactive(
            &project_root,
            &args.env,
            action,
            args.service.as_deref(),
            args.k8s,
        );
    }

    let mut app = App::new(args.env, args.k8s, project_root);
    app.refresh_status();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = ratatui::init();

    loop {
        while let Ok(line) = rx.try_recv() {
            app.push_output(line);
        }
        terminal.draw(|f| draw_ui(f, &mut app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => {
                        app.focus = if app.focus == Focus::Services {
                            Focus::Actions
                        } else {
                            Focus::Services
                        }
                    },
                    KeyCode::Char('e') => {
                        app.cycle_env();
                        app.refresh_status();
                    },
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Enter => {
                        if let (Some(action), Some(service)) =
                            (app.selected_action(), app.selected_service())
                        {
                            let action = action.to_string();
                            let service = service.to_string();
                            let env = app.env.clone();
                            let root = app.project_root.clone();
                            let tx_clone = tx.clone();
                            app.push_output(format!(
                                "=== EXECUTING {} ON {} [{}] ===",
                                action.to_uppercase(),
                                service.to_uppercase(),
                                env.to_uppercase()
                            ));
                            if app.use_k8s {
                                let svc =
                                    if ["deploy", "destroy", "status"].contains(&action.as_str()) {
                                        None
                                    } else {
                                        Some(service.as_str())
                                    };
                                let _ = k8s::run_action(&root, &env, &action, svc, tx_clone);
                            } else {
                                let svc = if action == "down" {
                                    None
                                } else {
                                    Some(service.as_str())
                                };
                                let _ = docker::run_action(&root, &env, &action, svc, tx_clone);
                            }
                        }
                    },
                    _ => {},
                }
            }
        }
    }

    ratatui::restore();
    disable_raw_mode()?;
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
    let env_color = match app.env.as_str() {
        "prod" => app.theme.error,
        "staging" => app.theme.warning,
        _ => app.theme.success,
    };
    tui::draw_header(
        f,
        chunks[0],
        "Deploy-Explorer",
        &format!("ENV: {}", app.env.to_uppercase()),
        env_color,
        None,
        if app.use_k8s {
            "Kubernetes"
        } else {
            "Docker Compose"
        },
        &app.theme,
    );

    let body = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(55),
        ])
        .split(chunks[1]);
    draw_services(f, body[0], app);
    draw_actions(f, body[1], app);
    draw_output(f, body[2], app);
    tui::draw_footer(
        f,
        chunks[2],
        &[
            ("Q", "Quit"),
            ("Tab", "Focus"),
            ("E", "Env"),
            ("↑↓", "Select"),
            ("Enter", "Run"),
        ],
        &app.theme,
    );
}

fn draw_services(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = SERVICES
        .iter()
        .map(|svc| {
            let status = app
                .containers
                .iter()
                .find(|c| c.service == *svc)
                .map_or_else(
                    || ("unknown".into(), app.theme.secondary),
                    |c| {
                        let color = match c.state.as_str() {
                            "running" => app.theme.success,
                            "exited" => app.theme.error,
                            _ => app.theme.warning,
                        };
                        (c.state.clone(), color)
                    },
                );
            ListItem::new(Line::from(vec![
                Span::styled(format!("{svc:<18} "), Style::default().fg(app.theme.fg)),
                Span::styled(format!("[{}]", status.0), Style::default().fg(status.1)),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(" SERVICES ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if app.focus == Focus::Services {
            app.theme.primary
        } else {
            app.theme.secondary
        }));
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(app.theme.highlight).bold())
            .highlight_symbol("▸ "),
        area,
        &mut app.service_state,
    );
}

fn draw_actions(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .actions()
        .iter()
        .map(|a| ListItem::new(format!("  {a}")))
        .collect();
    let block = Block::default()
        .title(" ACTIONS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if app.focus == Focus::Actions {
            app.theme.primary
        } else {
            app.theme.secondary
        }));
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(app.theme.highlight).bold())
            .highlight_symbol("▸ "),
        area,
        &mut app.action_state,
    );
}

fn draw_output(f: &mut Frame, area: Rect, app: &App) {
    let visible_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app
        .output_lines
        .iter()
        .rev()
        .take(visible_height)
        .rev()
        .map(|l| {
            let color = if l.contains("Error") || l.contains("error") {
                app.theme.error
            } else if l.starts_with("===") {
                app.theme.primary
            } else {
                app.theme.fg
            };
            Line::styled(l.clone(), Style::default().fg(color))
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" EXECUTION LOG ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(app.theme.secondary)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn run_non_interactive(
    project_root: &Path,
    env: &str,
    action: &str,
    service: Option<&str>,
    use_k8s: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    if use_k8s {
        k8s::run_action(project_root, env, action, service, tx)?;
    } else {
        docker::run_action(project_root, env, action, service, tx)?;
    }
    let rt_rx = std::sync::Mutex::new(rx);
    loop {
        #[allow(clippy::expect_used)]
        let mut guard = rt_rx.lock().expect("Lock should not be poisoned");
        match guard.try_recv() {
            Ok(line) => {
                println!("{line}");
                if line.starts_with("--- Process") {
                    break;
                }
            },
            Err(mpsc::error::TryRecvError::Empty) => {
                drop(guard);
                std::thread::sleep(Duration::from_millis(50));
            },
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    Ok(())
}
