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

use anyhow::Result;
use bin_explorer::analysis::{BinaryReport, FunctionReport};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use regex::{Regex, RegexBuilder};
use resq_tui::{draw_footer, draw_header, draw_popup, Theme};
use std::collections::HashSet;
use std::io;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Targets,
    Functions,
    Disassembly,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            Self::Targets => Self::Functions,
            Self::Functions => Self::Disassembly,
            Self::Disassembly => Self::Targets,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Targets => Self::Disassembly,
            Self::Functions => Self::Targets,
            Self::Disassembly => Self::Functions,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Targets => "TARGETS",
            Self::Functions => "FUNCTIONS",
            Self::Disassembly => "DISASSEMBLY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    FunctionSearch,
    DisassemblySearch,
}

struct App {
    reports: Vec<BinaryReport>,
    target_index: usize,
    function_index: usize,
    disassembly_scroll: u16,
    focus: FocusPane,
    input_mode: InputMode,
    filter: String,
    filter_input: String,
    disasm_query: String,
    disasm_input: String,
    disasm_regex: Option<Regex>,
    disasm_error: Option<String>,
    disasm_match_cursor: usize,
    stats_total: usize,
    stats_processed: usize,
    stats_failed: usize,
    stats_cache_hits: usize,
    issues: Vec<String>,
    show_help: bool,
    theme: Theme,
}

impl App {
    fn new(
        reports: Vec<BinaryReport>,
        stats_total: usize,
        stats_processed: usize,
        stats_failed: usize,
        stats_cache_hits: usize,
        issues: Vec<String>,
    ) -> Self {
        Self {
            reports,
            target_index: 0,
            function_index: 0,
            disassembly_scroll: 0,
            focus: FocusPane::Targets,
            input_mode: InputMode::Normal,
            filter: String::new(),
            filter_input: String::new(),
            disasm_query: String::new(),
            disasm_input: String::new(),
            disasm_regex: None,
            disasm_error: None,
            disasm_match_cursor: 0,
            stats_total,
            stats_processed,
            stats_failed,
            stats_cache_hits,
            issues,
            show_help: false,
            theme: Theme::default(),
        }
    }

    fn selected_report(&self) -> Option<&BinaryReport> {
        self.reports.get(self.target_index)
    }

    fn filtered_function_indices(&self) -> Vec<usize> {
        let Some(report) = self.selected_report() else {
            return Vec::new();
        };

        report
            .functions
            .iter()
            .enumerate()
            .filter(|(_, function)| {
                if self.filter.is_empty() {
                    true
                } else {
                    function
                        .name
                        .to_ascii_lowercase()
                        .contains(&self.filter.to_ascii_lowercase())
                }
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn selected_function(&self) -> Option<&FunctionReport> {
        let report = self.selected_report()?;
        let indices = self.filtered_function_indices();
        let function_idx = *indices.get(self.function_index)?;
        report.functions.get(function_idx)
    }

    fn disassembly_lines(&self) -> Vec<String> {
        let Some(function) = self.selected_function() else {
            return Vec::new();
        };

        function
            .instructions
            .iter()
            .map(|insn| format!("0x{:x}    {}", insn.address, insn.text))
            .collect()
    }

    fn disasm_match_indices(&self) -> Vec<usize> {
        let Some(regex) = &self.disasm_regex else {
            return Vec::new();
        };

        self.disassembly_lines()
            .iter()
            .enumerate()
            .filter(|(_, line)| regex.is_match(line))
            .map(|(idx, _)| idx)
            .collect()
    }

    fn active_disasm_match_line(&self, matches: &[usize]) -> Option<usize> {
        if matches.is_empty() {
            None
        } else {
            let idx = self
                .disasm_match_cursor
                .min(matches.len().saturating_sub(1));
            Some(matches[idx])
        }
    }

    fn set_disasm_query(&mut self, query: String) {
        self.disasm_query = query;
        self.disasm_match_cursor = 0;
        self.disassembly_scroll = 0;
        self.disasm_error = None;

        if self.disasm_query.is_empty() {
            self.disasm_regex = None;
            return;
        }

        match RegexBuilder::new(&self.disasm_query)
            .case_insensitive(true)
            .build()
        {
            Ok(regex) => {
                self.disasm_regex = Some(regex);
            },
            Err(err) => {
                self.disasm_regex = None;
                self.disasm_error = Some(err.to_string());
            },
        }
    }

    fn jump_disassembly_match(&mut self, next: bool) {
        let matches = self.disasm_match_indices();
        if matches.is_empty() {
            return;
        }

        if next {
            self.disasm_match_cursor = (self.disasm_match_cursor + 1) % matches.len();
        } else if self.disasm_match_cursor == 0 {
            self.disasm_match_cursor = matches.len().saturating_sub(1);
        } else {
            self.disasm_match_cursor = self.disasm_match_cursor.saturating_sub(1);
        }

        let line = matches[self.disasm_match_cursor];
        self.disassembly_scroll = line.saturating_sub(2) as u16;
    }

    fn jump_function_match(&mut self, next: bool) {
        let count = self.filtered_function_indices().len();
        if count == 0 {
            return;
        }

        if next {
            self.function_index = (self.function_index + 1) % count;
        } else if self.function_index == 0 {
            self.function_index = count.saturating_sub(1);
        } else {
            self.function_index = self.function_index.saturating_sub(1);
        }

        self.disassembly_scroll = 0;
    }

    fn clamp_selection(&mut self) {
        if self.reports.is_empty() {
            self.target_index = 0;
            self.function_index = 0;
            return;
        }

        if self.target_index >= self.reports.len() {
            self.target_index = self.reports.len().saturating_sub(1);
        }

        let function_count = self.filtered_function_indices().len();
        if function_count == 0 {
            self.function_index = 0;
        } else if self.function_index >= function_count {
            self.function_index = function_count.saturating_sub(1);
        }

        let disasm_matches = self.disasm_match_indices();
        if self.disasm_match_cursor >= disasm_matches.len() {
            self.disasm_match_cursor = 0;
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            FocusPane::Targets => {
                if !self.reports.is_empty() {
                    self.target_index =
                        (self.target_index + 1).min(self.reports.len().saturating_sub(1));
                    self.function_index = 0;
                    self.disassembly_scroll = 0;
                    self.disasm_match_cursor = 0;
                }
            },
            FocusPane::Functions => {
                let count = self.filtered_function_indices().len();
                if count > 0 {
                    self.function_index = (self.function_index + 1).min(count.saturating_sub(1));
                    self.disassembly_scroll = 0;
                    self.disasm_match_cursor = 0;
                }
            },
            FocusPane::Disassembly => {
                self.disassembly_scroll = self.disassembly_scroll.saturating_add(1);
            },
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            FocusPane::Targets => {
                self.target_index = self.target_index.saturating_sub(1);
                self.function_index = 0;
                self.disassembly_scroll = 0;
                self.disasm_match_cursor = 0;
            },
            FocusPane::Functions => {
                self.function_index = self.function_index.saturating_sub(1);
                self.disassembly_scroll = 0;
                self.disasm_match_cursor = 0;
            },
            FocusPane::Disassembly => {
                self.disassembly_scroll = self.disassembly_scroll.saturating_sub(1);
            },
        }
    }
}

pub(crate) fn run_tui(
    reports: Vec<BinaryReport>,
    stats: crate::RunStats,
    issues: Vec<String>,
) -> Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = ratatui::init();

    let mut app = App::new(
        reports,
        stats.total,
        stats.processed,
        stats.failed,
        stats.cache_hits,
        issues,
    );

    loop {
        app.clamp_selection();
        terminal.draw(|frame| draw_ui(frame, &app))?;

        if event::poll(POLL_INTERVAL)? {
            let event = event::read()?;
            if let Event::Key(key) = event {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.input_mode {
                    InputMode::FunctionSearch => {
                        match key.code {
                            KeyCode::Esc => {
                                app.input_mode = InputMode::Normal;
                                app.filter_input.clear();
                            },
                            KeyCode::Enter => {
                                app.filter = app.filter_input.clone();
                                app.input_mode = InputMode::Normal;
                                app.function_index = 0;
                                app.disassembly_scroll = 0;
                                app.disasm_match_cursor = 0;
                            },
                            KeyCode::Backspace => {
                                app.filter_input.pop();
                            },
                            KeyCode::Char(c) => {
                                app.filter_input.push(c);
                            },
                            _ => {},
                        }
                        continue;
                    },
                    InputMode::DisassemblySearch => {
                        match key.code {
                            KeyCode::Esc => {
                                app.input_mode = InputMode::Normal;
                                app.disasm_input.clear();
                            },
                            KeyCode::Enter => {
                                app.set_disasm_query(app.disasm_input.clone());
                                app.input_mode = InputMode::Normal;
                            },
                            KeyCode::Backspace => {
                                app.disasm_input.pop();
                            },
                            KeyCode::Char(c) => {
                                app.disasm_input.push(c);
                            },
                            _ => {},
                        }
                        continue;
                    },
                    InputMode::Normal => {},
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => app.focus = app.focus.next(),
                    KeyCode::BackTab => app.focus = app.focus.previous(),
                    KeyCode::Left => app.focus = app.focus.previous(),
                    KeyCode::Right => app.focus = app.focus.next(),
                    KeyCode::Char('h') => app.show_help = !app.show_help,
                    KeyCode::Char('/') => {
                        app.input_mode = InputMode::FunctionSearch;
                        app.filter_input = app.filter.clone();
                    },
                    KeyCode::Char('?') => {
                        app.input_mode = InputMode::DisassemblySearch;
                        app.disasm_input = app.disasm_query.clone();
                    },
                    KeyCode::Char('c') => {
                        app.filter.clear();
                        app.function_index = 0;
                        app.disassembly_scroll = 0;
                        app.disasm_match_cursor = 0;
                    },
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::PageDown => {
                        app.disassembly_scroll = app.disassembly_scroll.saturating_add(12);
                    },
                    KeyCode::PageUp => {
                        app.disassembly_scroll = app.disassembly_scroll.saturating_sub(12);
                    },
                    KeyCode::Home => {
                        app.disassembly_scroll = 0;
                    },
                    KeyCode::Char('n') => {
                        if app.focus == FocusPane::Disassembly && app.disasm_regex.is_some() {
                            app.jump_disassembly_match(true);
                        } else {
                            app.jump_function_match(true);
                        }
                    },
                    KeyCode::Char('N') => {
                        if app.focus == FocusPane::Disassembly && app.disasm_regex.is_some() {
                            app.jump_disassembly_match(false);
                        } else {
                            app.jump_function_match(false);
                        }
                    },
                    _ => {},
                }
            }
        }
    }

    ratatui::restore();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw_ui(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if app.show_help {
        draw_popup(
            frame,
            area,
            "Asm-Explorer Help",
            &[
                Line::from("Q / Esc   Quit"),
                Line::from("Tab/Shift  Switch pane focus"),
                Line::from("Up/Down    Navigate focused pane"),
                Line::from("N / Shift+N Jump next/prev match"),
                Line::from("/          Function substring filter"),
                Line::from("?          Disassembly regex search"),
                Line::from("C          Clear function filter"),
                Line::from("H          Toggle help"),
            ],
            60,
            45,
            &app.theme,
        );
        return;
    }

    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(12),
        Constraint::Length(3),
    ])
    .split(area);

    let selected_path = app
        .selected_report()
        .map_or_else(|| "<none>".to_string(), |r| r.path.display().to_string());

    let disasm_tag = if app.disasm_query.is_empty() {
        "<none>".to_string()
    } else {
        app.disasm_query.clone()
    };

    let status = format!(
        "PROCESSED={}/{} FAILED={} CACHE={} | FOCUS={} | FN={} | RX={}",
        app.stats_processed,
        app.stats_total,
        app.stats_failed,
        app.stats_cache_hits,
        app.focus.label(),
        if app.filter.is_empty() {
            "<none>"
        } else {
            app.filter.as_str()
        },
        disasm_tag
    );

    draw_header(
        frame,
        vertical[0],
        "Asm-Explorer",
        &status,
        app.theme.success,
        None,
        &selected_path,
        &app.theme,
    );

    let body = Layout::horizontal([
        Constraint::Percentage(28),
        Constraint::Percentage(32),
        Constraint::Percentage(40),
    ])
    .split(vertical[1]);

    draw_targets(frame, body[0], app);
    draw_functions(frame, body[1], app);
    draw_disassembly(frame, body[2], app);

    let footer_right = match app.input_mode {
        InputMode::Normal => "Normal",
        InputMode::FunctionSearch => "FnSearch",
        InputMode::DisassemblySearch => "RxSearch",
    };
    draw_footer(
        frame,
        vertical[2],
        &[
            ("Q", "Quit"),
            ("Tab", "Focus"),
            ("/", "Fn Filter"),
            ("?", "Regex"),
            ("N", "Jump"),
            ("H", "Help"),
            ("Mode", footer_right),
        ],
        &app.theme,
    );
}

fn pane_block(title: &str, is_focused: bool, theme: &Theme) -> Block<'static> {
    let border_color = if is_focused {
        theme.primary
    } else {
        theme.inactive
    };

    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
}

fn draw_targets(frame: &mut Frame, area: Rect, app: &App) {
    let items = app
        .reports
        .iter()
        .map(|report| {
            let name = report
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>");
            ListItem::new(format!(
                "{name} [{} {}]",
                report.format, report.architecture
            ))
        })
        .collect::<Vec<_>>();

    let mut state = ListState::default().with_selected(Some(app.target_index));
    let list = List::new(items)
        .block(pane_block(
            "Targets",
            app.focus == FocusPane::Targets,
            &app.theme,
        ))
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_functions(frame: &mut Frame, area: Rect, app: &App) {
    let Some(report) = app.selected_report() else {
        let empty = Paragraph::new("No target selected").block(pane_block(
            "Functions",
            app.focus == FocusPane::Functions,
            &app.theme,
        ));
        frame.render_widget(empty, area);
        return;
    };

    let filtered_indices = app.filtered_function_indices();
    let items = filtered_indices
        .iter()
        .filter_map(|idx| report.functions.get(*idx))
        .map(|f| {
            ListItem::new(format!(
                "{} [0x{:x}] insn={} size={}",
                f.name,
                f.address,
                f.instructions.len(),
                f.size
            ))
        })
        .collect::<Vec<_>>();

    let selected = if items.is_empty() {
        None
    } else {
        Some(app.function_index.min(items.len().saturating_sub(1)))
    };

    let mut state = ListState::default().with_selected(selected);
    let list = List::new(items)
        .block(pane_block(
            "Functions",
            app.focus == FocusPane::Functions,
            &app.theme,
        ))
        .highlight_style(
            Style::default()
                .bg(app.theme.highlight)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_disassembly(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::vertical([Constraint::Length(7), Constraint::Min(6)]).split(area);

    let matches = app.disasm_match_indices();
    let active_match = app.active_disasm_match_line(&matches);

    let summary_lines = if let Some(report) = app.selected_report() {
        vec![
            Line::from(format!("file: {}", report.path.display())),
            Line::from(format!(
                "format={} arch={} endian={} entry=0x{:x}",
                report.format, report.architecture, report.endianness, report.entry
            )),
            Line::from(format!(
                "sections={} symbols={} functions={} warnings={}",
                report.sections.len(),
                report.symbols.len(),
                report.functions.len(),
                report.warnings.len()
            )),
            Line::from(match &report.disassembly_coverage {
                Some(c) => format!(
                    "coverage: with_insn={} capstone={} objdump={} missing={}",
                    c.functions_with_instructions,
                    c.capstone_functions,
                    c.objdump_functions,
                    c.missing_functions
                ),
                None => "coverage: <none>".to_string(),
            }),
            Line::from(format!(
                "regex={} matches={}",
                if app.disasm_query.is_empty() {
                    "<none>"
                } else {
                    app.disasm_query.as_str()
                },
                matches.len()
            )),
            Line::from(match &app.disasm_error {
                Some(err) => format!("regex error: {err}"),
                None => "regex error: <none>".to_string(),
            }),
            Line::from(format!("issues: {}", app.issues.len())),
            Line::from(match app.issues.first() {
                Some(issue) => format!("latest issue: {issue}"),
                None => "latest issue: <none>".to_string(),
            }),
        ]
    } else {
        vec![Line::from("No report available")]
    };

    let summary = Paragraph::new(summary_lines)
        .block(pane_block("Summary", false, &app.theme))
        .wrap(Wrap { trim: false });
    frame.render_widget(summary, chunks[0]);

    let disasm_lines = if let Some(function) = app.selected_function() {
        if function.instructions.is_empty() {
            vec![Line::from("No disassembly available for this function.")]
        } else {
            let match_set = matches.iter().copied().collect::<HashSet<_>>();
            function
                .instructions
                .iter()
                .enumerate()
                .map(|(idx, insn)| {
                    let text = format!("0x{:x}    {}", insn.address, insn.text);
                    if Some(idx) == active_match {
                        Line::styled(
                            text,
                            Style::default()
                                .fg(app.theme.bg)
                                .bg(app.theme.accent)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else if match_set.contains(&idx) {
                        Line::styled(text, Style::default().fg(app.theme.warning))
                    } else {
                        Line::from(text)
                    }
                })
                .collect::<Vec<_>>()
        }
    } else {
        vec![Line::from("Select a function to view disassembly.")]
    };

    let disasm = Paragraph::new(disasm_lines)
        .block(pane_block(
            "Disassembly",
            app.focus == FocusPane::Disassembly,
            &app.theme,
        ))
        .scroll((app.disassembly_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(disasm, chunks[1]);

    match app.input_mode {
        InputMode::FunctionSearch => {
            draw_popup(
                frame,
                frame.area(),
                "Function Filter",
                &[
                    Line::from("Type function substring and press Enter."),
                    Line::from(format!("> {}", app.filter_input)),
                ],
                55,
                20,
                &app.theme,
            );
        },
        InputMode::DisassemblySearch => {
            draw_popup(
                frame,
                frame.area(),
                "Disassembly Regex",
                &[
                    Line::from("Type regex and press Enter. Case-insensitive."),
                    Line::from(format!("> {}", app.disasm_input)),
                    Line::from(match &app.disasm_error {
                        Some(err) => format!("Last error: {err}"),
                        None => "Last error: <none>".to_string(),
                    }),
                ],
                65,
                25,
                &app.theme,
            );
        },
        InputMode::Normal => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bin_explorer::analysis::{FunctionReport, Instruction, SectionInfo, SymbolInfo};
    use std::path::PathBuf;

    fn sample_report() -> BinaryReport {
        BinaryReport {
            path: PathBuf::from("sample.bin"),
            format: "Elf".to_string(),
            architecture: "X86_64".to_string(),
            endianness: "Little".to_string(),
            entry: 0x1000,
            size_bytes: 1234,
            sections: vec![SectionInfo {
                name: ".text".to_string(),
                address: 0x1000,
                size: 16,
                kind: "Text".to_string(),
            }],
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                address: 0x1000,
                size: 16,
                kind: "Text".to_string(),
                is_global: true,
            }],
            functions: vec![FunctionReport {
                name: "main".to_string(),
                address: 0x1000,
                size: 16,
                instructions: vec![
                    Instruction {
                        address: 0x1000,
                        text: "push %rbp".to_string(),
                    },
                    Instruction {
                        address: 0x1001,
                        text: "mov %rsp,%rbp".to_string(),
                    },
                    Instruction {
                        address: 0x1004,
                        text: "pop %rbp".to_string(),
                    },
                ],
            }],
            disassembly_backend: Some("capstone".to_string()),
            disassembly_attempts: vec!["capstone: ok".to_string()],
            disassembly_coverage: Some(bin_explorer::analysis::DisassemblyCoverage {
                total_functions: 1,
                functions_with_instructions: 1,
                capstone_functions: 1,
                objdump_functions: 0,
                missing_functions: 0,
            }),
            function_backend_coverage: vec![bin_explorer::analysis::FunctionBackendCoverage {
                name: "main".to_string(),
                backend: "capstone".to_string(),
                instruction_count: 3,
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn regex_query_tracks_matches_and_jump() {
        let mut app = App::new(vec![sample_report()], 1, 1, 0, 0, Vec::new());
        app.set_disasm_query("rbp".to_string());
        let matches = app.disasm_match_indices();
        assert_eq!(matches, vec![0, 1, 2]);

        app.focus = FocusPane::Disassembly;
        app.jump_disassembly_match(true);
        let line = app.active_disasm_match_line(&app.disasm_match_indices());
        assert_eq!(line, Some(1));
    }

    #[test]
    fn invalid_regex_sets_error() {
        let mut app = App::new(vec![sample_report()], 1, 1, 0, 0, Vec::new());
        app.set_disasm_query("(".to_string());
        assert!(app.disasm_regex.is_none());
        assert!(app.disasm_error.is_some());
    }
}
