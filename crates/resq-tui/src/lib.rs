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

//! Shared TUI components and themes for `ResQ` developer tools.
//! Inspired by binsider architecture.

pub use crossterm;
pub use ratatui;

pub mod terminal;

use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
    Frame,
};

// ---------------------------------------------------------------------------
// UI Theme
// ---------------------------------------------------------------------------

/// Spinner animation frames for loading indicators.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Standard `ResQ` TUI Theme.
pub struct Theme {
    /// Primary brand color (Cyan)
    pub primary: Color,
    /// Secondary supporting color (Blue)
    pub secondary: Color,
    /// Accent color for PID/Metadata (Magenta)
    pub accent: Color,
    /// Success state (Green)
    pub success: Color,
    /// Warning/Pending state (Yellow)
    pub warning: Color,
    /// Error/Critical state (Red)
    pub error: Color,
    /// Background color
    pub bg: Color,
    /// Foreground text color
    pub fg: Color,
    /// Highlight/Selection color
    pub highlight: Color,
    /// Inactive/Muted color (`DarkGray`)
    pub inactive: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Blue,
            accent: Color::Magenta,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            bg: Color::Black,
            fg: Color::White,
            highlight: Color::Rgb(50, 50, 50),
            inactive: Color::DarkGray,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared Widgets
// ---------------------------------------------------------------------------

/// Renders a standardized header with service metadata and PID.
#[allow(clippy::too_many_arguments)]
pub fn draw_header(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    status: &str,
    status_color: Color,
    pid: Option<i32>,
    url: &str,
    theme: &Theme,
) {
    let pid_info = pid.map_or_else(|| "PID: ?".to_string(), |p| format!("PID: {p}"));

    let header_content = Line::from(vec![
        Span::styled(
            format!(" 🔬 {} ", title.to_uppercase()),
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(status, Style::default().fg(status_color)),
        Span::raw(" │ "),
        Span::styled(pid_info, Style::default().fg(theme.accent)),
        Span::raw(" │ ").fg(theme.inactive),
        Span::styled(
            url,
            Style::default()
                .fg(theme.secondary)
                .add_modifier(Modifier::ITALIC),
        ),
    ]);

    let header = Paragraph::new(header_content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.primary)),
    );

    frame.render_widget(header, area);
}

/// Renders a standardized footer with keyboard shortcuts.
pub fn draw_footer(frame: &mut Frame, area: Rect, keys: &[(&str, &str)], theme: &Theme) {
    let mut spans = Vec::with_capacity(keys.len() * 2);
    for (k, v) in keys {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::default()
                .fg(theme.bg)
                .bg(theme.primary)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {v} "),
            Style::default().fg(theme.fg),
        ));
        spans.push(Span::raw("  "));
    }

    let footer = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.primary)),
    );

    frame.render_widget(footer, area);
}

/// Renders a standardized tab bar.
pub fn draw_tabs(frame: &mut Frame, area: Rect, titles: Vec<&str>, selected: usize) {
    let theme = Theme::default();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected)
        .style(Style::default().fg(theme.primary))
        .highlight_style(Style::default().fg(theme.warning).bold().underlined());

    frame.render_widget(tabs, area);
}

/// Renders a centered popup for help or errors.
#[allow(clippy::too_many_arguments)]
pub fn draw_popup(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: &[Line],
    percent_x: u16,
    percent_y: u16,
    theme: &Theme,
) {
    let popup_area = centered_rect(percent_x, percent_y, area);

    // Clear background
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.bg)),
        popup_area,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.primary))
        .title(format!(" {title} "))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let paragraph = Paragraph::new(lines.to_vec())
        .alignment(Alignment::Left)
        .style(Style::default().fg(theme.fg));

    frame.render_widget(paragraph, inner);
}

/// Helper to create a centered rectangle for popups.
#[must_use]
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
// Common Utilities
// ---------------------------------------------------------------------------

/// Formats bytes into human-readable units.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Formats seconds into human-readable duration.
#[must_use]
pub fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    // -----------------------------------------------------------------------
    // format_bytes
    // -----------------------------------------------------------------------

    #[test]
    fn format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn format_bytes_bytes_range() {
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn format_bytes_kib_range() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1023), "1023.0 KiB");
    }

    #[test]
    fn format_bytes_mib_range() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(1024 * 1024 * 500), "500.0 MiB");
    }

    #[test]
    fn format_bytes_gib_range() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.00 GiB");
    }

    #[test]
    fn format_bytes_boundary_kib() {
        // Exactly at the KiB boundary
        assert_eq!(format_bytes(1024), "1.0 KiB");
    }

    #[test]
    fn format_bytes_boundary_mib() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn format_bytes_boundary_gib() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
    }

    // -----------------------------------------------------------------------
    // format_duration
    // -----------------------------------------------------------------------

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(61), "1m 1s");
        assert_eq!(format_duration(3599), "59m 59s");
    }

    #[test]
    fn format_duration_hours_minutes_seconds() {
        assert_eq!(format_duration(3600), "1h 0m 0s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(86399), "23h 59m 59s");
    }

    #[test]
    fn format_duration_days() {
        assert_eq!(format_duration(86400), "1d 0h 0m");
        assert_eq!(format_duration(90061), "1d 1h 1m");
        assert_eq!(format_duration(172_800), "2d 0h 0m");
    }

    // -----------------------------------------------------------------------
    // centered_rect
    // -----------------------------------------------------------------------

    #[test]
    fn centered_rect_basic() {
        let outer = Rect::new(0, 0, 100, 100);
        let inner = centered_rect(50, 50, outer);
        // The inner rect should be roughly centered and roughly 50% of the outer
        assert!(inner.x > 0, "inner.x should be > 0, got {}", inner.x);
        assert!(inner.y > 0, "inner.y should be > 0, got {}", inner.y);
        assert!(inner.width > 0, "inner.width should be > 0");
        assert!(inner.height > 0, "inner.height should be > 0");
        // Should be contained within the outer rect
        assert!(inner.x + inner.width <= outer.width);
        assert!(inner.y + inner.height <= outer.height);
    }

    #[test]
    fn centered_rect_full_size() {
        let outer = Rect::new(0, 0, 100, 50);
        let inner = centered_rect(100, 100, outer);
        // At 100% it should be the full outer rect
        assert_eq!(inner.width, outer.width);
        assert_eq!(inner.height, outer.height);
    }

    #[test]
    fn centered_rect_small_percent() {
        let outer = Rect::new(0, 0, 200, 200);
        let inner = centered_rect(10, 10, outer);
        // Should be much smaller than outer
        assert!(inner.width < outer.width / 2);
        assert!(inner.height < outer.height / 2);
    }

    #[test]
    fn centered_rect_is_actually_centered() {
        let outer = Rect::new(0, 0, 100, 100);
        let inner = centered_rect(50, 50, outer);
        // Check that margins are roughly equal on both sides
        let left_margin = inner.x;
        let right_margin = outer.width - (inner.x + inner.width);
        let top_margin = inner.y;
        let bottom_margin = outer.height - (inner.y + inner.height);
        // Allow +-1 for rounding
        assert!(
            left_margin.abs_diff(right_margin) <= 1,
            "horizontal centering off: left={left_margin}, right={right_margin}"
        );
        assert!(
            top_margin.abs_diff(bottom_margin) <= 1,
            "vertical centering off: top={top_margin}, bottom={bottom_margin}"
        );
    }

    #[test]
    fn centered_rect_zero_area() {
        let outer = Rect::new(0, 0, 0, 0);
        let inner = centered_rect(50, 50, outer);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    // -----------------------------------------------------------------------
    // Theme::default
    // -----------------------------------------------------------------------

    #[test]
    fn theme_default_colors() {
        let theme = Theme::default();
        assert_eq!(theme.primary, Color::Cyan);
        assert_eq!(theme.secondary, Color::Blue);
        assert_eq!(theme.accent, Color::Magenta);
        assert_eq!(theme.success, Color::Green);
        assert_eq!(theme.warning, Color::Yellow);
        assert_eq!(theme.error, Color::Red);
        assert_eq!(theme.bg, Color::Black);
        assert_eq!(theme.fg, Color::White);
        assert_eq!(theme.highlight, Color::Rgb(50, 50, 50));
        assert_eq!(theme.inactive, Color::DarkGray);
    }

    // -----------------------------------------------------------------------
    // SPINNER_FRAMES
    // -----------------------------------------------------------------------

    #[test]
    #[allow(clippy::const_is_empty)]
    fn spinner_frames_not_empty() {
        assert!(!SPINNER_FRAMES.is_empty());
    }

    #[test]
    fn spinner_frames_all_single_char() {
        for frame in SPINNER_FRAMES {
            assert_eq!(
                frame.chars().count(),
                1,
                "frame '{frame}' is not single char"
            );
        }
    }
}
