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

//! Terminal environment detection.
//!
//! Mirrors the gh-aw pattern of gating all styling through environment checks:
//! - TTY detection via `crossterm::tty::IsTty`
//! - `NO_COLOR` standard (<https://no-color.org/>)
//! - `TERM=dumb` detection
//! - `ACCESSIBLE` env var for screen-reader mode
//!
//! All console output formatting in [`crate::console`] is gated through
//! [`should_style`] so no ANSI codes bleed into pipes or redirects.

use std::io;
use std::sync::OnceLock;

use crossterm::tty::IsTty;

/// Detected terminal color capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Dark terminal background (default assumption).
    Dark,
    /// Light terminal background.
    Light,
    /// No color support — plain text only.
    None,
}

// ---------------------------------------------------------------------------
// Cached detection (computed once per process)
// ---------------------------------------------------------------------------

struct DetectedEnv {
    stdout_tty: bool,
    stderr_tty: bool,
    no_color: bool,
    term_dumb: bool,
    accessible: bool,
    color_mode: ColorMode,
}

fn detect() -> &'static DetectedEnv {
    static ENV: OnceLock<DetectedEnv> = OnceLock::new();
    ENV.get_or_init(|| {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let term_dumb = std::env::var("TERM")
            .map(|t| t == "dumb")
            .unwrap_or(false);
        let accessible = std::env::var_os("ACCESSIBLE").is_some();

        let stdout_tty = io::stdout().is_tty();
        let stderr_tty = io::stderr().is_tty();

        let color_mode = if no_color || term_dumb || accessible {
            ColorMode::None
        } else if !stdout_tty && !stderr_tty {
            ColorMode::None
        } else {
            // Heuristic: check COLORFGBG (set by some terminals like rxvt)
            // Format: "foreground;background" — if bg >= 8, it's likely light.
            // Also check macOS Terminal.app `TERM_PROGRAM` or `ITERM_PROFILE`.
            // Fallback: assume dark.
            if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
                if let Some(bg) = colorfgbg.rsplit(';').next().and_then(|s| s.parse::<u8>().ok())
                {
                    if bg >= 8 {
                        return DetectedEnv {
                            stdout_tty,
                            stderr_tty,
                            no_color,
                            term_dumb,
                            accessible,
                            color_mode: ColorMode::Light,
                        };
                    }
                }
            }
            ColorMode::Dark
        };

        DetectedEnv {
            stdout_tty,
            stderr_tty,
            no_color,
            term_dumb,
            accessible,
            color_mode,
        }
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if stdout is connected to a terminal.
pub fn is_tty_stdout() -> bool {
    detect().stdout_tty
}

/// Returns `true` if stderr is connected to a terminal.
pub fn is_tty_stderr() -> bool {
    detect().stderr_tty
}

/// Returns `true` if the environment requests accessible / plain output.
///
/// Checks: `NO_COLOR`, `TERM=dumb`, `ACCESSIBLE`.
pub fn is_accessible_mode() -> bool {
    let env = detect();
    env.no_color || env.term_dumb || env.accessible
}

/// Returns `true` if styled output should be emitted to stderr.
///
/// This is the master gate — all console formatters check this before
/// applying any ANSI styling.
pub fn should_style() -> bool {
    let env = detect();
    env.stderr_tty && !env.no_color && !env.term_dumb && !env.accessible
}

/// Returns the detected color mode for adaptive color selection.
pub fn detect_color_mode() -> ColorMode {
    detect().color_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mode_variants_are_comparable() {
        assert_ne!(ColorMode::Dark, ColorMode::Light);
        assert_ne!(ColorMode::Dark, ColorMode::None);
    }
}
