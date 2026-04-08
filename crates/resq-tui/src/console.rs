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

//! TTY-gated console message formatters for non-TUI CLI output.
//!
//! Mirrors the gh-aw `pkg/console/console.go` pattern:
//! - 14+ named message formatters (success, error, warning, info, …)
//! - All styling gated through [`crate::detect::should_style`]
//! - Output routing: diagnostics → stderr, structured data → stdout
//! - In non-TTY / accessible mode, emoji prefixes still appear but ANSI
//!   color codes are stripped.

use crate::detect;
use crate::theme;

// ---------------------------------------------------------------------------
// ANSI helpers (inline, no external deps)
// ---------------------------------------------------------------------------

/// Applies foreground ANSI color to a string if styling is enabled.
fn style_fg(text: &str, color: ratatui::style::Color) -> String {
    if !detect::should_style() {
        return text.to_string();
    }
    let code = color_to_ansi_fg(color);
    format!("{code}{text}\x1b[0m")
}

/// Applies bold + foreground color if styling is enabled.
fn style_bold(text: &str, color: ratatui::style::Color) -> String {
    if !detect::should_style() {
        return text.to_string();
    }
    let code = color_to_ansi_fg(color);
    format!("\x1b[1m{code}{text}\x1b[0m")
}

/// Applies dim style if styling is enabled.
fn style_dim(text: &str) -> String {
    if !detect::should_style() {
        return text.to_string();
    }
    format!("\x1b[2m{text}\x1b[0m")
}

/// Converts a ratatui Color to an ANSI foreground escape sequence.
fn color_to_ansi_fg(color: ratatui::style::Color) -> String {
    use ratatui::style::Color;
    match color {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
        Color::Black => "\x1b[30m".into(),
        Color::Red => "\x1b[31m".into(),
        Color::Green => "\x1b[32m".into(),
        Color::Yellow => "\x1b[33m".into(),
        Color::Blue => "\x1b[34m".into(),
        Color::Magenta => "\x1b[35m".into(),
        Color::Cyan => "\x1b[36m".into(),
        Color::White => "\x1b[37m".into(),
        Color::DarkGray => "\x1b[90m".into(),
        Color::LightRed => "\x1b[91m".into(),
        Color::LightGreen => "\x1b[92m".into(),
        Color::LightYellow => "\x1b[93m".into(),
        Color::LightBlue => "\x1b[94m".into(),
        Color::LightMagenta => "\x1b[95m".into(),
        Color::LightCyan => "\x1b[96m".into(),
        Color::Gray => "\x1b[37m".into(),
        Color::Indexed(n) => format!("\x1b[38;5;{n}m"),
        Color::Reset => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Format functions — return styled strings
// ---------------------------------------------------------------------------

/// Formats a success message: `✅ <message>`
pub fn format_success(message: &str) -> String {
    format!("✅ {}", style_fg(message, theme::COLOR_SUCCESS.resolve()))
}

/// Formats an error message: `❌ <message>`
pub fn format_error(message: &str) -> String {
    format!("❌ {}", style_bold(message, theme::COLOR_ERROR.resolve()))
}

/// Formats a warning message: `⚠️  <message>`
pub fn format_warning(message: &str) -> String {
    format!("⚠️  {}", style_fg(message, theme::COLOR_WARNING.resolve()))
}

/// Formats an info message: `ℹ️  <message>`
pub fn format_info(message: &str) -> String {
    format!("ℹ️  {}", style_fg(message, theme::COLOR_PRIMARY.resolve()))
}

/// Formats a command reference: `▶ <command>`
pub fn format_command(command: &str) -> String {
    format!("▶ {}", style_bold(command, theme::COLOR_SECONDARY.resolve()))
}

/// Formats a progress/in-flight message: `⏳ <message>`
pub fn format_progress(message: &str) -> String {
    format!("⏳ {}", style_fg(message, theme::COLOR_WARNING.resolve()))
}

/// Formats a prompt message: `? <message>`
pub fn format_prompt(message: &str) -> String {
    format!("? {}", style_bold(message, theme::COLOR_PRIMARY.resolve()))
}

/// Formats a verbose/debug message (dim).
pub fn format_verbose(message: &str) -> String {
    style_dim(message)
}

/// Formats a list item: `  • <message>`
pub fn format_list_item(message: &str) -> String {
    format!("  • {message}")
}

/// Formats a section header with a rule line.
pub fn format_section_header(header: &str) -> String {
    let bar = "━".repeat(74usize.saturating_sub(header.len() + 1));
    format!(
        "\n━━━ {} {}",
        style_bold(header, theme::COLOR_PRIMARY.resolve()),
        style_dim(&bar)
    )
}

/// Formats a count/metric message: `📊 <message>`
pub fn format_count(message: &str) -> String {
    format!("📊 {}", style_fg(message, theme::COLOR_ACCENT.resolve()))
}

/// Formats a location/path message: `📁 <message>`
pub fn format_location(message: &str) -> String {
    format!("📁 {}", style_fg(message, theme::COLOR_SECONDARY.resolve()))
}

/// Formats a list header.
pub fn format_list_header(header: &str) -> String {
    style_bold(header, theme::COLOR_FG.resolve())
}

/// Formats a search/scan message: `🔍 <message>`
pub fn format_search(message: &str) -> String {
    format!("🔍 {}", style_fg(message, theme::COLOR_PRIMARY.resolve()))
}

// ---------------------------------------------------------------------------
// Convenience output functions (write to stderr)
// ---------------------------------------------------------------------------

/// Prints a success message to stderr.
pub fn success(message: &str) {
    eprintln!("{}", format_success(message));
}

/// Prints an error message to stderr.
pub fn error(message: &str) {
    eprintln!("{}", format_error(message));
}

/// Prints a warning message to stderr.
pub fn warning(message: &str) {
    eprintln!("{}", format_warning(message));
}

/// Prints an info message to stderr.
pub fn info(message: &str) {
    eprintln!("{}", format_info(message));
}

/// Prints a progress message to stderr.
pub fn progress(message: &str) {
    eprintln!("{}", format_progress(message));
}

/// Prints a verbose/debug message to stderr.
pub fn verbose(message: &str) {
    eprintln!("{}", format_verbose(message));
}

/// Prints a section header to stderr.
pub fn section(header: &str) {
    eprintln!("{}", format_section_header(header));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_success_contains_emoji() {
        let s = format_success("all good");
        assert!(s.contains('✅'));
        assert!(s.contains("all good"));
    }

    #[test]
    fn format_error_contains_emoji() {
        let s = format_error("it broke");
        assert!(s.contains('❌'));
        assert!(s.contains("it broke"));
    }

    #[test]
    fn format_list_item_is_indented() {
        let s = format_list_item("entry");
        assert!(s.starts_with("  •"));
    }

    #[test]
    fn format_section_header_has_rule() {
        let s = format_section_header("Test");
        assert!(s.contains('━'));
        assert!(s.contains("Test"));
    }

    #[test]
    fn color_to_ansi_fg_basic_colors() {
        use ratatui::style::Color;
        assert_eq!(color_to_ansi_fg(Color::Red), "\x1b[31m");
        assert_eq!(color_to_ansi_fg(Color::Green), "\x1b[32m");
        assert!(color_to_ansi_fg(Color::Rgb(255, 0, 0)).contains("255"));
    }
}
