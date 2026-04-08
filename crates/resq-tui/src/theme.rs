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

//! Centralized theme and adaptive colors for ResQ TUI and CLI output.
//!
//! Mirrors the gh-aw `pkg/styles/theme.go` pattern:
//! - All colors defined as [`AdaptiveColor`] with explicit light and dark
//!   variants (Dracula-inspired dark palette).
//! - No direct color usage outside this module and [`crate::console`].
//! - Resolution is gated through [`crate::detect::detect_color_mode`].

use ratatui::style::Color;

use crate::detect::{self, ColorMode};

// ---------------------------------------------------------------------------
// Adaptive Color
// ---------------------------------------------------------------------------

/// A color that adapts to the terminal background (light vs dark).
///
/// Mirrors `lipgloss.AdaptiveColor` from the Charmbracelet ecosystem.
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveColor {
    /// Color for light terminal backgrounds.
    pub light: Color,
    /// Color for dark terminal backgrounds.
    pub dark: Color,
}

impl AdaptiveColor {
    /// Resolves to the appropriate color variant based on the detected
    /// terminal color mode. Returns [`Color::Reset`] when color is disabled.
    pub fn resolve(&self) -> Color {
        match detect::detect_color_mode() {
            ColorMode::Dark => self.dark,
            ColorMode::Light => self.light,
            ColorMode::None => Color::Reset,
        }
    }
}

// ---------------------------------------------------------------------------
// Palette Constants (Dracula-inspired dark, readable light)
// ---------------------------------------------------------------------------

/// Primary brand color (Cyan).
pub const COLOR_PRIMARY: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(0, 139, 139),   // Dark cyan for light bg
    dark: Color::Rgb(139, 233, 253),   // Dracula cyan
};

/// Secondary supporting color (Blue/Purple).
pub const COLOR_SECONDARY: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(68, 71, 144),    // Muted blue for light bg
    dark: Color::Rgb(189, 147, 249),   // Dracula purple
};

/// Accent color for metadata (Magenta/Pink).
pub const COLOR_ACCENT: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(163, 55, 136),   // Dark magenta for light bg
    dark: Color::Rgb(255, 121, 198),   // Dracula pink
};

/// Success state (Green).
pub const COLOR_SUCCESS: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(40, 130, 40),    // Dark green for light bg
    dark: Color::Rgb(80, 250, 123),    // Dracula green
};

/// Warning/pending state (Yellow/Orange).
pub const COLOR_WARNING: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(180, 120, 0),    // Dark amber for light bg
    dark: Color::Rgb(241, 250, 140),   // Dracula yellow
};

/// Error/critical state (Red).
pub const COLOR_ERROR: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(215, 55, 55),    // Dark red for light bg
    dark: Color::Rgb(255, 85, 85),     // Dracula red
};

/// Foreground text.
pub const COLOR_FG: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(40, 42, 54),     // Dracula bg as fg on light
    dark: Color::Rgb(248, 248, 242),   // Dracula fg
};

/// Background.
pub const COLOR_BG: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(248, 248, 242),  // Dracula fg as bg on light
    dark: Color::Rgb(40, 42, 54),      // Dracula bg
};

/// Inactive / muted / comment.
pub const COLOR_INACTIVE: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(140, 140, 140),  // Medium gray for light bg
    dark: Color::Rgb(98, 114, 164),    // Dracula comment
};

/// Highlight / selection background.
pub const COLOR_HIGHLIGHT: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(230, 230, 230),  // Light gray selection
    dark: Color::Rgb(68, 71, 90),      // Dracula current line
};

/// Progress bar gradient start.
pub const COLOR_PROGRESS_START: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(100, 60, 180),   // Deeper purple for light bg
    dark: Color::Rgb(189, 147, 249),   // Dracula purple
};

/// Progress bar gradient end.
pub const COLOR_PROGRESS_END: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(0, 139, 139),    // Darker cyan for light bg
    dark: Color::Rgb(139, 233, 253),   // Dracula cyan
};

/// Progress bar empty track.
pub const COLOR_PROGRESS_EMPTY: AdaptiveColor = AdaptiveColor {
    light: Color::Rgb(200, 200, 200),  // Light gray track
    dark: Color::Rgb(98, 114, 164),    // Dracula comment
};

// ---------------------------------------------------------------------------
// Theme struct (for TUI full-screen apps)
// ---------------------------------------------------------------------------

/// Standard ResQ TUI Theme with adaptive color support.
///
/// Consumers should call [`Theme::adaptive`] to get colors that match the
/// detected terminal background, or [`Theme::default`] for the classic
/// hardcoded dark palette (backward-compatible).
pub struct Theme {
    /// Primary brand color (Cyan).
    pub primary: Color,
    /// Secondary supporting color (Blue/Purple).
    pub secondary: Color,
    /// Accent color for metadata (Magenta/Pink).
    pub accent: Color,
    /// Success state (Green).
    pub success: Color,
    /// Warning/pending state (Yellow/Orange).
    pub warning: Color,
    /// Error/critical state (Red).
    pub error: Color,
    /// Background color.
    pub bg: Color,
    /// Foreground text color.
    pub fg: Color,
    /// Highlight/selection color.
    pub highlight: Color,
    /// Inactive/muted/comment color.
    pub inactive: Color,
}

impl Theme {
    /// Creates a theme that adapts to the detected terminal color mode.
    ///
    /// Uses [`AdaptiveColor::resolve`] for every field. This is the
    /// recommended constructor for new code.
    pub fn adaptive() -> Self {
        Self {
            primary: COLOR_PRIMARY.resolve(),
            secondary: COLOR_SECONDARY.resolve(),
            accent: COLOR_ACCENT.resolve(),
            success: COLOR_SUCCESS.resolve(),
            warning: COLOR_WARNING.resolve(),
            error: COLOR_ERROR.resolve(),
            bg: COLOR_BG.resolve(),
            fg: COLOR_FG.resolve(),
            highlight: COLOR_HIGHLIGHT.resolve(),
            inactive: COLOR_INACTIVE.resolve(),
        }
    }
}

impl Default for Theme {
    /// Creates the classic hardcoded dark-theme palette.
    ///
    /// Exists for backward compatibility. Prefer [`Theme::adaptive`] for
    /// new code.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_default_has_inactive() {
        let t = Theme::default();
        assert_eq!(t.inactive, Color::DarkGray);
    }

    #[test]
    fn adaptive_color_resolve_does_not_panic() {
        // Just exercise the resolve path — actual color depends on env.
        let _ = COLOR_PRIMARY.resolve();
        let _ = COLOR_ERROR.resolve();
    }

    #[test]
    fn theme_adaptive_does_not_panic() {
        let _ = Theme::adaptive();
    }
}
