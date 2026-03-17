# resq-tui — Agent Guide

## Mission
Shared TUI component library for all ResQ developer tools. Provides standardized themes, layout helpers, and interactive widgets to ensure a consistent operator experience across explorers.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- Serialization: Serde (Themes)

## Repo Map
- `src/lib.rs` — Main library entry point and export list
- `src/theme.rs` — Standardized ResQ color palette
- `src/console.rs` — Styling and console UI helpers
- `src/table.rs` — Shared table and list widgets
- `src/progress.rs` — Progress bars and spinners
- `README.md` — Library usage and component list
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-tui
cargo test -p resq-tui
```

## Rules
- New components must adhere to the global `Theme`.
- Prefer generic, reusable widgets over service-specific ones.
- Avoid external dependencies beyond Ratatui and Crossterm if possible.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Ensure components handle various terminal sizes and aspect ratios gracefully.
- Widgets must be thread-safe for use in async TUI loops.

## Workflow
1. Run `cargo build -p resq-tui` before finalizing code changes.
2. Verify visual changes in at least one consumer explorer (e.g. `log-viewer`).
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, new components (if any), and visual impact.

## References
- [README](README.md)
- [Tools Guide](../AGENTS.md)
