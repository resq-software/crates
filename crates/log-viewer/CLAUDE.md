# resq-logs — Agent Guide

## Mission
Multi-source log aggregator for ResQ. Streams and aggregates logs from Docker Compose containers or local files with filtering and search capabilities.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- Pattern matching: `regex`
- Async runtime: Tokio
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — TUI application and mode selection
- `src/parser.rs` — Log parsing logic (JSON, plain text)
- `src/sources.rs` — Docker and file log streaming sources
- `README.md` — Operator usage and source types
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-logs
cargo test -p resq-logs
```

## Rules
- Support both structured (JSON) and unstructured (plain text) logs in `parser.rs`.
- Ensure the 10,000-line ring buffer remains performant during high-throughput streaming.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Be aware of terminal performance when streaming very high volumes of logs.
- Handle Docker and file rotation gracefully to avoid stream disconnection.

## Workflow
1. Run `cargo build -p resq-logs` before finalizing code changes.
2. Run `cargo test -p resq-logs` to verify log parsing logic.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, parser improvements (if any), and test results.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
