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

//! Terminal lifecycle helpers — init, restore, and event-loop runner.

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// A `ratatui` terminal backed by Crossterm.
pub type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Initialise raw mode and enter the alternate screen.
///
/// Returns a ready-to-use [`Term`]. Call [`restore`] when finished.
///
/// # Errors
/// Propagates any I/O error from Crossterm or Ratatui.
pub fn init() -> anyhow::Result<Term> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    Ok(terminal)
}

/// Leave the alternate screen and disable raw mode.
///
/// Safe to call even if the terminal is in a partially-initialised state.
pub fn restore() {
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// Implement this trait on your app state to use [`run_loop`].
pub trait TuiApp {
    /// Draw the current frame.
    fn draw(&mut self, frame: &mut ratatui::Frame);

    /// Handle a key event. Return `false` to exit the loop.
    ///
    /// # Errors
    /// Returns any application-specific error that should terminate the TUI loop.
    fn handle_key(&mut self, key: event::KeyEvent) -> anyhow::Result<bool>;
}

/// Run a standard TUI event loop with the given app.
///
/// `poll_ms` controls how frequently the loop polls for keyboard input.
/// Ctrl+C always exits. The terminal is **not** automatically initialised
/// or restored — wrap the call site with [`init`] / [`restore`].
///
/// # Errors
/// Propagates draw or event errors, and errors from the app's `handle_key`.
pub fn run_loop(terminal: &mut Term, poll_ms: u64, app: &mut dyn TuiApp) -> anyhow::Result<()> {
    let timeout = Duration::from_millis(poll_ms);
    loop {
        terminal.draw(|f| app.draw(f))?;
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                if !app.handle_key(key)? {
                    break;
                }
            }
        }
    }
    Ok(())
}
