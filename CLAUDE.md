# ResQ Crates — Workspace Agent Guide

## Mission
Crate registry and developer tooling for the ResQ platform. This workspace contains all Rust crates published to crates.io: a zero-dependency data structures library and a suite of CLI/TUI tools for auditing, deployment, performance monitoring, and repository maintenance.

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

## resq-dsa Rules
- **Zero-dependency policy**: `resq-dsa` must have zero production dependencies (`[dependencies]` must remain empty). Only `[dev-dependencies]` are allowed (e.g., `big-o-test`). This is a hard requirement for the crate's value proposition.
- **`no_std` compatibility**: The crate must compile with `default-features = false` (no `std`). It uses `#![cfg_attr(not(feature = "std"), no_std)]` and `extern crate alloc`. All types must be usable in `no_std + alloc` environments.
- **Complexity tests**: Algorithmic complexity tests using `big-o-test` must be annotated with `#[ignore]` because they are timing-sensitive and may flake in CI. Run them explicitly with `cargo test -p resq-dsa -- --ignored`.
- **Hash implementations**: The crate uses hand-rolled FNV-1a hashing. Do not introduce external hash crate dependencies.
- **Module structure**: Each data structure lives in its own module (`bloom`, `count_min`, `graph`, `heap`, `trie`). The `trie` module also contains the `rabin_karp` function.

## Global Commands
```bash
cargo build                  # Build all tools
cargo test                   # Run all tests
cargo test -p resq-dsa       # Run DSA tests only
cargo test -p resq-dsa -- --ignored  # Run complexity benchmarks
./agent-sync.sh --check      # Verify all agent guides are in sync
```

## Repository Rules
- Do not commit `target/` or generated binaries.
- All new source files must include the Apache-2.0 license header (managed by `resq copyright`).
- Keep binary names consistent: `resq-<name>`.
- Do not add production dependencies to `resq-dsa`.
- Ensure `resq-dsa` compiles under `no_std` before merging.

## References
- [Root README](README.md)
- [Individual Crate READMEs](crates/cli/README.md, crates/resq-tui/README.md, etc.)
