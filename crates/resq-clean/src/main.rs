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

//! `ResQ` Cleanup-Explorer TUI v2.0
//!
//! Robust visual workspace cleaner.
//! Analyzes build artifacts and gitignored files with interactive deletion.

#![deny(missing_docs)]

use anyhow::Result;
use clap::Parser;
use ignore::{gitignore::Gitignore, WalkBuilder};
use std::fs;
use std::path::{Path, PathBuf};

use resq_tui::crossterm::event::{KeyCode, KeyEventKind};
use resq_tui::ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};
use resq_tui::terminal::TuiApp;
use resq_tui::{self as tui, terminal, Theme};

/// Visual workspace cleaner for `ResQ`.
#[derive(Parser, Debug)]
#[command(name = "cleanup-explorer", about = "Visual workspace cleaner for ResQ")]
struct Args {
    /// Preview what would be deleted without removing anything
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

struct App {
    root: PathBuf,
    entries: Vec<Entry>,
    list_state: ListState,
    theme: Theme,
    dry_run: bool,
}

struct Entry {
    path: PathBuf,
    is_dir: bool,
    size: u64,
    selected: bool,
}

impl App {
    fn new(root: PathBuf, dry_run: bool) -> Self {
        Self {
            root,
            entries: Vec::new(),
            list_state: ListState::default(),
            theme: Theme::default(),
            dry_run,
        }
    }

    fn scan(&mut self) {
        let gitignore_path = self.root.join(".gitignore");
        if !gitignore_path.exists() {
            return;
        }
        let (gitignore, _) = Gitignore::new(&gitignore_path);

        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(false)
            .parents(false)
            .build();
        let mut entries = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path().to_path_buf();
            if path == self.root || path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }
            let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
            if gitignore.matched(&path, is_dir).is_ignore() {
                let size = if is_dir {
                    get_dir_size(&path)
                } else {
                    fs::metadata(&path).map_or(0, |m| m.len())
                };
                entries.push(Entry {
                    path,
                    is_dir,
                    size,
                    selected: true,
                });
            }
        }
        entries.sort_by_key(|e| std::cmp::Reverse(e.size));
        self.entries = entries;
        if !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn toggle_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.entries[i].selected = !self.entries[i].selected;
        }
    }

    fn delete_selected(&mut self) {
        if self.dry_run {
            return;
        }
        for entry in self.entries.iter().filter(|e| e.selected) {
            if entry.is_dir {
                let _ = fs::remove_dir_all(&entry.path);
            } else {
                let _ = fs::remove_file(&entry.path);
            }
        }
        self.scan();
    }
}

fn get_dir_size(path: &Path) -> u64 {
    WalkBuilder::new(path)
        .build()
        .filter_map(std::result::Result::ok)
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

impl TuiApp for App {
    fn draw(&mut self, f: &mut Frame) {
        draw_ui(f, self);
    }

    fn handle_key(&mut self, key: resq_tui::crossterm::event::KeyEvent) -> Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(true);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(false),
            KeyCode::Char(' ') => {
                self.toggle_selected();
                Ok(true)
            }
            KeyCode::Enter => {
                self.delete_selected();
                Ok(!self.dry_run)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                if !self.entries.is_empty() {
                    self.list_state
                        .select(Some((i + 1).min(self.entries.len() - 1)));
                }
                Ok(true)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.entries.is_empty() {
                    let i = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some(i.saturating_sub(1)));
                }
                Ok(true)
            }
            _ => Ok(true),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let root = std::env::current_dir()?;
    let mut app = App::new(root, args.dry_run);
    app.scan();

    let mut guard = terminal::init()?;
    terminal::run_loop(&mut guard, 100, &mut app)
}

fn draw_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());
    let total_size: u64 = app
        .entries
        .iter()
        .filter(|e| e.selected)
        .map(|e| e.size)
        .sum();
    tui::draw_header(
        f,
        chunks[0],
        "Cleanup-Explorer",
        &format!("PENDING: {}", tui::format_bytes(total_size)),
        app.theme.warning,
        None,
        &app.root.to_string_lossy(),
        &app.theme,
    );

    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|e| {
            let prefix = if e.selected { " [x] " } else { " [ ] " };
            let icon = if e.is_dir { "📁 " } else { "📄 " };
            let rel_path = e
                .path
                .strip_prefix(&app.root)
                .unwrap_or(&e.path)
                .to_string_lossy();
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(icon, Style::default().fg(app.theme.primary)),
                Span::raw(format!("{rel_path:<40} ")),
                Span::styled(
                    tui::format_bytes(e.size),
                    Style::default().fg(app.theme.success),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(" BUILD ARTIFACTS & IGNORED FILES ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.primary));
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(app.theme.highlight).bold()),
        chunks[1],
        &mut app.list_state,
    );

    tui::draw_footer(
        f,
        chunks[2],
        &[
            ("Q", "Quit"),
            ("Space", "Toggle"),
            (
                "Enter",
                if app.dry_run {
                    "Dry Run Exit"
                } else {
                    "Delete Selected"
                },
            ),
            ("↑↓", "Navigate"),
        ],
        &app.theme,
    );
}
