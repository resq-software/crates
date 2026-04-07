# resq-flame — Agent Guide

## Mission
SVG CPU flame graph profiler for ResQ services. Provides an interactive TUI for selecting profiling targets and subcommands to profile specific environments, piping the output through `inferno-flamegraph`.

## Stack
- Runtime: Rust
- Profiling engines: `inferno`, `py-spy`, `perf`
- UI: Ratatui + Crossterm
- CLI: Clap
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — TUI application and profiler logic
- `README.md` — User usage and profiler types
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-flame
cargo test -p resq-flame
```

## Rules
- New profiling targets must be added as options in the TUI.
- Maintain compatibility with `inferno` folded stack format.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Ensure that external profiler binaries (`py-spy`, `perf`) are handled gracefully if missing.
- Be cautious when profiling production environments — use reasonable durations.

## Workflow
1. Run `cargo build -p resq-flame` before finalizing code changes.
2. Verify SVG generation with a sample folded stack file.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, new profiling targets (if any), and test results.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
