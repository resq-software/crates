# resq-perf — Agent Guide

## Mission
Real-time performance diagnostic dashboard for ResQ services. Polls service `/status` endpoints to display CPU, memory, and request latency metrics in a TUI.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- HTTP Client: `reqwest`
- Metrics: `procfs`, `object`, `addr2line` (Memory Profiler)
- Async runtime: Tokio
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — TUI application and polling logic
- `README.md` — User usage and metric definitions
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-perf
cargo test -p resq-perf
```

## Rules
- Polling logic must stay non-blocking using `tokio` or dedicated threads.
- Maintain a rolling history for sparkline charts (e.g. 120 samples).
- Support Bearer token authentication via `RESQ_TOKEN`.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Ensure the polling interval is configurable to avoid overloading services.
- Correctly handle connection errors and display them as warning states in the UI.

## Workflow
1. Run `cargo build -p resq-perf` before finalizing code changes.
2. Verify metrics parsing against the `coordination-hce` `/status` endpoint schema.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, metric calculation improvements, and test output.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
