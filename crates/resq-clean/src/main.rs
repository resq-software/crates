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
use ignore::{
    gitignore::{Gitignore, GitignoreBuilder},
    WalkBuilder,
};
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
    /// Result of the most recent delete pass, shown in the header. `Some(true)`
    /// on the error variant so the header can switch to the warning colour.
    status: Option<(String, bool)>,
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
            theme: Theme::adaptive(),
            dry_run,
            status: None,
        }
    }

    fn scan(&mut self) {
        self.entries = collect_ignored(&self.root);
        if self.entries.is_empty() {
            self.list_state.select(None);
        } else {
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
        let selected: Vec<&Entry> = self.entries.iter().filter(|e| e.selected).collect();
        let count = selected.len();
        let errors = delete_entries(&selected);
        self.status = Some(if errors.is_empty() {
            (format!("Deleted {count} item(s)"), false)
        } else {
            // Surface (do not swallow) every failed removal; show the first inline.
            let (path, err) = &errors[0];
            (
                format!(
                    "{} of {count} failed — {}: {err}{}",
                    errors.len(),
                    path.display(),
                    if errors.len() > 1 { " (…)" } else { "" }
                ),
                true,
            )
        });
        self.scan();
    }
}

/// File names that must never be deleted, even when gitignored. Matches `.env`
/// and any `.env.*` variant (`.env.local`, `.env.production`, …) — see
/// resq-clean/CLAUDE.md: "Never delete `.env` files even if they are gitignored."
fn is_protected(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name == ".env" || name.starts_with(".env."),
        None => false,
    }
}

/// Build a gitignore matcher spanning every `.gitignore` in the tree, shallowest
/// first, so deeper and negated rules (`!keep`) are added later and take
/// precedence — the root-only matcher this replaces silently dropped them.
fn build_gitignore(root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    let mut gitignores: Vec<PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(false)
        .parents(false)
        .build()
        .flatten()
        .filter(|e| {
            e.file_name() == ".gitignore" && !e.path().components().any(|c| c.as_os_str() == ".git")
        })
        .map(|e| e.path().to_path_buf())
        .collect();
    gitignores.sort_by_key(|p| p.components().count());
    for gi in gitignores {
        // A malformed nested .gitignore is skipped, matching git's own leniency.
        let _ = builder.add(gi);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// Collect the gitignored files/dirs under `root` that are candidates for
/// deletion. Pure over the filesystem (no `self`), so it is unit-testable:
/// `.env*` and `.git` are always excluded, nested negations are honored.
fn collect_ignored(root: &Path) -> Vec<Entry> {
    let gitignore = build_gitignore(root);
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(false)
        .parents(false)
        .build();
    let mut entries = Vec::new();

    for entry in walker.flatten() {
        let path = entry.path().to_path_buf();
        if path == root || path.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        if is_protected(&path) {
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
    entries
}

/// Remove the given entries, returning every failure instead of discarding it.
fn delete_entries(entries: &[&Entry]) -> Vec<(PathBuf, std::io::Error)> {
    let mut errors = Vec::new();
    for entry in entries {
        let result = if entry.is_dir {
            fs::remove_dir_all(&entry.path)
        } else {
            fs::remove_file(&entry.path)
        };
        if let Err(e) = result {
            errors.push((entry.path.clone(), e));
        }
    }
    errors
}

fn get_dir_size(path: &Path) -> u64 {
    WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(false)
        .build()
        .filter_map(std::result::Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(std::fs::Metadata::is_file)
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
                // Dry-run: nothing to delete, exit as before. Real run: delete,
                // then stay open so the rescan and any surfaced errors are visible
                // (the user quits with q/Esc). Previously this exited immediately,
                // making the post-delete rescan dead work.
                if self.dry_run {
                    return Ok(false);
                }
                self.delete_selected();
                Ok(true)
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
    let (subtitle, header_color) = match &app.status {
        Some((msg, is_error)) => (
            msg.clone(),
            if *is_error {
                app.theme.error
            } else {
                app.theme.success
            },
        ),
        None => (
            format!("PENDING: {}", tui::format_bytes(total_size)),
            app.theme.warning,
        ),
    };
    tui::draw_header(
        f,
        chunks[0],
        "Cleanup-Explorer",
        &subtitle,
        header_color,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;

    /// Build a temp dir tree from (relative path, contents) pairs. A path ending
    /// in `/` is created as a directory. Returns the tempdir (kept alive by caller).
    fn make_tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel, contents) in files {
            let path = dir.path().join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir");
            }
            if rel.ends_with('/') {
                fs::create_dir_all(&path).expect("mkdir");
            } else {
                fs::write(&path, contents).expect("write");
            }
        }
        dir
    }

    fn collected_names(root: &Path) -> HashSet<String> {
        collect_ignored(root)
            .into_iter()
            .map(|e| {
                e.path
                    .strip_prefix(root)
                    .unwrap_or(&e.path)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn env_file_is_never_collected_even_when_gitignored() {
        let dir = make_tree(&[
            (".gitignore", ".env\nbuild/\n"),
            (".env", "SECRET=1"),
            ("build/", ""),
        ]);
        let names = collected_names(dir.path());
        assert!(
            !names.contains(".env"),
            "`.env` must never be a deletion candidate; got {names:?}"
        );
        assert!(
            names.contains("build"),
            "gitignored artifacts must still be collected; got {names:?}"
        );
    }

    #[test]
    fn dotenv_variants_are_protected() {
        let dir = make_tree(&[
            (".gitignore", ".env*\n"),
            (".env", "A=1"),
            (".env.local", "B=2"),
            (".env.production", "C=3"),
        ]);
        let names = collected_names(dir.path());
        assert!(
            names.is_empty(),
            "no `.env*` file may be collected; got {names:?}"
        );
    }

    #[test]
    fn nested_negation_preserves_whitelisted_file() {
        // Root ignores every *.log; a nested rule re-includes keep.log.
        let dir = make_tree(&[
            (".gitignore", "*.log\n"),
            ("sub/.gitignore", "!keep.log\n"),
            ("a.log", "x"),
            ("sub/keep.log", "y"),
        ]);
        let names = collected_names(dir.path());
        assert!(
            names.contains("a.log"),
            "root-ignored a.log should be collected; got {names:?}"
        );
        assert!(
            !names.contains("sub/keep.log"),
            "nested `!keep.log` negation must preserve the file; got {names:?}"
        );
    }

    #[test]
    fn git_internal_files_are_skipped() {
        let dir = make_tree(&[
            (".gitignore", "*.log\n"),
            (".git/config", "[core]"),
            (".git/x.log", "z"),
            ("a.log", "x"),
        ]);
        let names = collected_names(dir.path());
        assert!(
            !names.iter().any(|n| n.starts_with(".git")),
            "`.git` contents must be skipped; got {names:?}"
        );
        assert!(names.contains("a.log"));
    }

    #[test]
    fn no_gitignore_yields_no_candidates() {
        let dir = make_tree(&[("src/main.rs", "fn main() {}"), ("a.log", "x")]);
        assert!(collected_names(dir.path()).is_empty());
    }

    #[test]
    fn dry_run_deletes_nothing() {
        let dir = make_tree(&[(".gitignore", "build/\n"), ("build/artifact.bin", "data")]);
        let mut app = App::new(dir.path().to_path_buf(), true);
        app.scan();
        assert!(
            !app.entries.is_empty(),
            "should have found the gitignored build/ dir"
        );
        app.delete_selected();
        assert!(
            dir.path().join("build/artifact.bin").exists(),
            "dry-run must not delete anything"
        );
    }

    #[test]
    fn real_delete_removes_selected_and_reports_success() {
        let dir = make_tree(&[
            (".gitignore", "build/\n.env\n"),
            ("build/artifact.bin", "data"),
            (".env", "SECRET=1"),
        ]);
        let mut app = App::new(dir.path().to_path_buf(), false);
        app.scan();
        app.delete_selected();
        assert!(
            !dir.path().join("build").exists(),
            "selected gitignored dir should be deleted"
        );
        assert!(
            dir.path().join(".env").exists(),
            "protected .env must survive a real delete"
        );
        assert!(
            matches!(app.status, Some((_, false))),
            "successful delete should report a non-error status"
        );
    }

    #[test]
    fn delete_errors_are_surfaced_not_swallowed() {
        // An entry pointing at a path that no longer exists yields an io error
        // that must be reported rather than discarded by `let _ =`.
        let missing = Entry {
            path: PathBuf::from("/nonexistent/resq-clean/definitely/not/here"),
            is_dir: false,
            size: 0,
            selected: true,
        };
        let errors = delete_entries(&[&missing]);
        assert_eq!(
            errors.len(),
            1,
            "a failed removal must be returned, not swallowed"
        );
        assert_eq!(errors[0].0, missing.path);
    }
}
