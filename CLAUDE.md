# ResQ CLI — Monorepo Agent Guide

## Mission
Developer tooling for the ResQ platform. This monorepo contains a suite of CLI and TUI tools for auditing, deployment, performance monitoring, and repository maintenance.

## Workspace Layout
All crates live under the `crates/` directory:
- `crates/resq-dsa/` — Data structures and algorithms library (zero dependencies, `no_std`-compatible).
- `crates/cli/` — The main `resq` CLI tool (entry point).
- `crates/resq-tui/` — Shared component library for all TUI tools.
- `crates/bin-explorer/` — Machine code and binary analyzer (`resq-bin`).
- `crates/cleanup/` — Workspace cleaner (`resq-clean`).
- `crates/deploy-cli/` — Environment manager (`resq-deploy`).
- `crates/flame-graph/` — CPU profiler (`resq-flame`).
- `crates/health-checker/` — Service health monitor (`resq-health`).
- `crates/log-viewer/` — Log aggregator (`resq-logs`).
- `crates/perf-monitor/` — Performance dashboard (`resq-perf`).

## Shared Standards
- **Runtime**: Rust (latest stable).
- **UI Architecture**: Ratatui with a shared `resq-tui` theme and header/footer components.
- **CLI Framework**: Clap v4 (derive mode).
- **Safety**: Tools must be read-only by default (except `cleanup` and `copyright`).
- **Sync**: Always keep `AGENTS.md` and `CLAUDE.md` in sync using `./agent-sync.sh`.

## Global Commands
```bash
cargo build                  # Build all tools
cargo test                   # Run all tests
./agent-sync.sh --check      # Verify all agent guides are in sync
```

## Repository Rules
- Do not commit `target/` or generated binaries.
- All new source files must include the Apache-2.0 license header (managed by `resq copyright`).
- Keep binary names consistent: `resq-<name>`.

## References
- [Root README](README.md)
- [Individual Crate READMEs](crates/cli/README.md, crates/resq-tui/README.md, etc.)
