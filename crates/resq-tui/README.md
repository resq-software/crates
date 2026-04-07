<!--
  Copyright 2026 ResQ

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# resq-tui — Shared TUI Library

A library crate providing standardized themes, layout components, and widgets for all ResQ developer TUIs. Built on top of `ratatui` and `crossterm`.

## Core Features

- **Theme Engine**: Unified color palette and styling (`Theme`).
- **Standard Headers/Footers**: Consistent brand identity across all explorers.
- **Interactive Widgets**: Tabs, popups, and centered layouts.
- **Formatting Utilities**: Human-readable bytes and durations.
- **Spinner Support**: Standardized loading animations.

## Usage (Internal Only)

Add to `Cargo.toml`:

```toml
[dependencies]
resq-tui = { workspace = true }
```

### Rendering a Standard Header

```rust
use resq_tui::{self as tui, Theme};

fn draw(f: &mut Frame) {
    let theme = Theme::default();
    tui::draw_header(
        f,
        area,
        "My-Explorer",
        "READY",
        theme.success,
        Some(1234), // PID
        "http://localhost:3000",
        &theme,
    );
}
```

### Rendering a Footer with Shortcuts

```rust
tui::draw_footer(
    f,
    area,
    &[("Q", "Quit"), ("Tab", "Focus"), ("↑↓", "Navigate")],
    &theme,
);
```

### Formatting Bytes

```rust
use resq_tui::format_bytes;
let size = format_bytes(1024 * 1024 * 5); // "5.0 MiB"
```

## Shared Components

| Component | Purpose |
|-----------|---------|
| `Theme` | Standardized color definitions for ResQ brand |
| `draw_header` | Renders a ResQ-branded header with service metadata |
| `draw_footer` | Renders keyboard shortcuts in a stylized footer |
| `draw_tabs` | Renders a tab bar with selection highlights |
| `draw_popup` | Renders a centered modal for help or errors |
| `format_bytes` | Standardized human-readable byte sizing |
| `format_duration`| Standardized human-readable durations |
| `SPINNER_FRAMES` | Consistent loading indicator frames |
