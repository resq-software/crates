# resq-health — Agent Guide

## Mission
Service health diagnostic dashboard for ResQ. Polls all service health endpoints and displays status/latency in a TUI or a single-check CLI mode.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- HTTP Client: `reqwest`
- Async runtime: Tokio
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — TUI application and event loop
- `src/services.rs` — Service registry and poll logic
- `src/integration.rs` — Integration test helpers
- `README.md` — Operator usage and service list
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-health
cargo test -p resq-health
```

## Rules
- New services must be added to the registry in `src/services.rs`.
- Support JSON-RPC or other non-standard health checks (e.g. Neo N3) in `check_service`.
- Ensure `--check` mode exits with correct status codes for CI.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — keep `CLAUDE.md` synchronized with `./agent-sync.sh` and never edit `CLAUDE.md` directly.

## Safety
- Be aware of polling frequency to avoid accidental DoS on local services.
- Time out all requests to prevent the TUI from hanging.

## Workflow
1. Run `cargo build -p resq-health` before finalizing code changes.
2. Verify polling logic against mock endpoints if possible.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, new services added, and test results.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
