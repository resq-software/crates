# bin-explorer — Agent Guide

## Mission
Terminal binary and machine-code analyzer for ResQ. Used to inspect compiled artifacts, symbol/layout changes, and disassembly details during performance and debugging work.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- Analysis: `object`, `capstone`, `regex`, `crc32fast`
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — CLI entry point and mode selection
- `src/lib.rs` — Shared analysis helpers
- `src/tui.rs` — Interactive terminal UI
- `src/cache.rs` — Cached inspection data
- `src/analysis/` — Binary inspection and reporting modules
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-bin
cargo test -p resq-bin
```

## Rules
- Keep analysis read-only — never mutate inspected binaries.
- TUI output should degrade cleanly to non-interactive CLI output when needed.
- Prefer shared `resq-tui` components over bespoke terminal widgets.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Do not run generated binaries automatically as part of inspection.
- Keep symbol/disassembly parsing resilient to stripped and partially malformed binaries.

## Workflow
1. Run `cargo build -p bin-explorer` before finalizing code changes.
2. Run `cargo test -p bin-explorer` if parsing or cache behavior changed.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, behavior change, and test output.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
