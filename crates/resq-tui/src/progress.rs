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

//! Non-TUI progress bar for CLI output.
//!
//! Renders a styled progress bar to stderr using adaptive colors.
//! Falls back to a plain ASCII bar in accessible/non-TTY mode.
//!
//! Mirrors gh-aw's `pkg/console/progress.go` pattern with adaptive colors
//! instead of hardcoded hex strings.

use std::io::{self, Write};

use crate::detect;
use crate::theme;

/// Configuration for a progress bar.
pub struct ProgressBar {
    width: usize,
    message: String,
}

impl ProgressBar {
    /// Creates a new progress bar with the given width and message.
    pub fn new(message: &str, width: usize) -> Self {
        Self {
            width,
            message: message.to_string(),
        }
    }

    /// Renders the progress bar at the given fraction (0.0 – 1.0) to stderr.
    ///
    /// In TTY mode with color support, uses adaptive gradient colors.
    /// In non-TTY or accessible mode, renders a plain ASCII bar.
    pub fn render(&self, fraction: f64) {
        let fraction = fraction.clamp(0.0, 1.0);
        let filled = (self.width as f64 * fraction).round() as usize;
        let empty = self.width - filled;

        if detect::should_style() && detect::is_tty_stderr() {
            let fill_color = theme::COLOR_PROGRESS_START.resolve();
            let empty_color = theme::COLOR_PROGRESS_EMPTY.resolve();

            let fill_ansi = color_to_fg(fill_color);
            let empty_ansi = color_to_fg(empty_color);

            eprint!(
                "\r{} {}{}{}{}{}  {:.0}%  ",
                self.message,
                fill_ansi,
                "█".repeat(filled),
                empty_ansi,
                "░".repeat(empty),
                "\x1b[0m",
                fraction * 100.0,
            );
        } else {
            eprint!(
                "\r{} [{}{}] {:.0}%  ",
                self.message,
                "#".repeat(filled),
                "-".repeat(empty),
                fraction * 100.0,
            );
        }

        let _ = io::stderr().flush();
    }

    /// Finishes the progress bar with a newline.
    pub fn finish(&self) {
        eprintln!();
    }

    /// Finishes the progress bar and prints a final message.
    pub fn finish_with_message(&self, message: &str) {
        eprint!("\r{}\r", " ".repeat(self.width + self.message.len() + 20));
        eprintln!("{message}");
    }
}

/// Simple ANSI foreground helper (avoids pulling in console module).
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
    fn progress_bar_clamps_fraction() {
        let pb = ProgressBar::new("test", 20);
        // Should not panic with out-of-range values
        pb.render(1.5);
        pb.render(-0.5);
        pb.finish();
    }
}
