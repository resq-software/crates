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

//! Spinner frames and non-TUI spinner for CLI output.
//!
//! Provides:
//! - [`SPINNER_FRAMES`] — Braille animation frames for TUI usage.
//! - [`Spinner`] — A thread-safe stderr spinner that respects
//!   [`crate::detect::should_style`] and falls back to plain dots in
//!   accessible mode.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::detect;

/// Braille spinner animation frames (for ratatui TUI rendering).
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Accessible fallback spinner: plain dots.
const DOTS_FRAMES: &[&str] = &[".", "..", "...", "....", "....."];

/// A non-TUI stderr spinner.
///
/// Mirrors gh-aw's `pkg/console/spinner.go` with thread-safe lifecycle.
///
/// # Example
/// ```no_run
/// use resq_tui::spinner::Spinner;
///
/// let spinner = Spinner::start("Loading data");
/// // ... do work ...
/// spinner.stop_with_message("✅ Loaded 42 items");
/// ```
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Starts a spinner on stderr with the given message.
    ///
    /// In non-TTY or accessible mode, prints the message once without
    /// animation.
    pub fn start(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));

        if !detect::should_style() || !detect::is_tty_stderr() {
            eprintln!("{message}...");
            return Self {
                running,
                handle: None,
            };
        }

        let running_clone = running.clone();
        let msg = message.to_string();

        let handle = thread::spawn(move || {
            let frames = if detect::is_accessible_mode() {
                DOTS_FRAMES
            } else {
                SPINNER_FRAMES
            };

            let mut idx = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = frames[idx % frames.len()];
                eprint!("\r{frame} {msg}  ");
                let _ = io::stderr().flush();
                idx += 1;
                thread::sleep(Duration::from_millis(80));
            }
            // Clear the spinner line
            eprint!("\r{}\r", " ".repeat(msg.len() + 10));
            let _ = io::stderr().flush();
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stops the spinner and prints a final message.
    pub fn stop_with_message(self, message: &str) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
        eprintln!("{message}");
    }

    /// Stops the spinner without printing a final message.
    pub fn stop(self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle {
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frames_are_not_empty() {
        assert!(!SPINNER_FRAMES.is_empty());
    }

    #[test]
    fn spinner_frames_are_single_width() {
        for frame in SPINNER_FRAMES {
            assert_eq!(frame.chars().count(), 1);
        }
    }
}
