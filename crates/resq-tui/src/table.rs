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

//! Styled table renderer for non-TUI CLI output.
//!
//! Mirrors gh-aw's `RenderTable` with zebra-striped rows and
//! adaptive colors. Falls back to plain aligned text when styling
//! is disabled.

use crate::detect;
use crate::theme;

/// Column alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Left-aligned (default).
    Left,
    /// Right-aligned.
    Right,
}

/// A column definition.
pub struct Column {
    /// Column header text.
    pub header: String,
    /// Alignment.
    pub align: Align,
    /// Minimum width (0 = auto).
    pub min_width: usize,
}

impl Column {
    /// Creates a left-aligned column.
    pub fn new(header: &str) -> Self {
        Self {
            header: header.to_string(),
            align: Align::Left,
            min_width: 0,
        }
    }

    /// Creates a right-aligned column.
    pub fn right(header: &str) -> Self {
        Self {
            header: header.to_string(),
            align: Align::Right,
            min_width: 0,
        }
    }

    /// Sets minimum column width.
    pub fn width(mut self, w: usize) -> Self {
        self.min_width = w;
        self
    }
}

/// Renders a table to stderr with optional zebra striping and header styling.
///
/// Each row is a `Vec<String>`. Columns define headers and alignment.
///
/// # Example
/// ```no_run
/// use resq_tui::table::{Column, render_table};
///
/// let columns = vec![
///     Column::new("Name"),
///     Column::right("Size"),
///     Column::new("Status"),
/// ];
/// let rows = vec![
///     vec!["api".into(), "12 MB".into(), "healthy".into()],
///     vec!["worker".into(), "8 MB".into(), "degraded".into()],
/// ];
/// render_table(&columns, &rows);
/// ```
pub fn render_table(columns: &[Column], rows: &[Vec<String>]) {
    if columns.is_empty() {
        return;
    }

    // Compute column widths
    let mut widths: Vec<usize> = columns
        .iter()
        .map(|c| c.header.len().max(c.min_width))
        .collect();

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let styled = detect::should_style();

    // Header
    let header_line: String = columns
        .iter()
        .enumerate()
        .map(|(i, col)| pad(&col.header, widths[i], col.align))
        .collect::<Vec<_>>()
        .join("  ");

    if styled {
        let color = theme::COLOR_PRIMARY.resolve();
        let code = color_to_fg(color);
        eprintln!("  {code}\x1b[1m{header_line}\x1b[0m");
    } else {
        eprintln!("  {header_line}");
    }

    // Separator
    let sep: String = widths
        .iter()
        .map(|w| "─".repeat(*w))
        .collect::<Vec<_>>()
        .join("──");
    if styled {
        let dim = "\x1b[2m";
        eprintln!("  {dim}{sep}\x1b[0m");
    } else {
        eprintln!("  {sep}");
    }

    // Rows with zebra striping
    for (row_idx, row) in rows.iter().enumerate() {
        let line: String = columns
            .iter()
            .enumerate()
            .map(|(i, col)| {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                pad(cell, widths[i], col.align)
            })
            .collect::<Vec<_>>()
            .join("  ");

        if styled && row_idx % 2 == 1 {
            // Zebra stripe: dim background row
            eprintln!("  \x1b[2m{line}\x1b[0m");
        } else {
            eprintln!("  {line}");
        }
    }
}

/// Pads a string to the given width with the given alignment.
fn pad(text: &str, width: usize, align: Align) -> String {
    match align {
        Align::Left => format!("{:<width$}", text, width = width),
        Align::Right => format!("{:>width$}", text, width = width),
    }
}

/// ANSI foreground helper.
fn color_to_fg(color: ratatui::style::Color) -> String {
    use ratatui::style::Color;
    match color {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
        Color::Reset => String::new(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_left() {
        assert_eq!(pad("hi", 5, Align::Left), "hi   ");
    }

    #[test]
    fn pad_right() {
        assert_eq!(pad("hi", 5, Align::Right), "   hi");
    }

    #[test]
    fn empty_columns_no_panic() {
        render_table(&[], &[]);
    }

    #[test]
    fn column_builder() {
        let c = Column::new("Name").width(10);
        assert_eq!(c.header, "Name");
        assert_eq!(c.min_width, 10);
        assert_eq!(c.align, Align::Left);
    }

    #[test]
    fn column_right() {
        let c = Column::right("Size");
        assert_eq!(c.align, Align::Right);
    }
}
