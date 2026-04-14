# ResQ Crates — Workspace Agent Guide

## Mission
Crate registry and developer tooling for the ResQ platform. This workspace contains all Rust crates published to crates.io: a zero-dependency data structures library and a suite of CLI/TUI tools for auditing, deployment, performance monitoring, and repository maintenance.

## Workspace Layout
All crates live under the `crates/` directory:
- `crates/resq-dsa/` — Data structures and algorithms library (zero dependencies, `no_std`-compatible).
- `crates/resq-cli/` — The main `resq` CLI tool (entry point).
- `crates/resq-tui/` — Shared component library for all TUI tools.
- `crates/resq-bin/` — Machine code and binary analyzer (`resq-bin`).
- `crates/resq-clean/` — Workspace cleaner (`resq-clean`).
- `crates/resq-deploy/` — Environment manager (`resq-deploy`).
- `crates/resq-flame/` — CPU profiler (`resq-flame`).
- `crates/resq-health/` — Service health monitor (`resq-health`).
- `crates/resq-logs/` — Log aggregator (`resq-logs`).
- `crates/resq-perf/` — Performance dashboard (`resq-perf`).

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

## Git hooks

This repo is the **source of truth** for canonical git-hook templates across the ResQ org. They live in [`crates/resq-cli/templates/git-hooks/`](crates/resq-cli/templates/git-hooks) and are embedded into the `resq` binary via `include_str!`, so `resq dev install-hooks` can scaffold them offline into any repo. The `.git-hooks/` copy in this repo's root is kept byte-identical by `hooks-sync.yml` CI.

Sibling repos install these via [`resq-software/dev/scripts/install-hooks.sh`](https://github.com/resq-software/dev/blob/main/scripts/install-hooks.sh), which prefers the offline `resq dev install-hooks` path when the binary is available and falls back to raw-fetching from this repo's `master` branch. See [resq/AGENTS.md#git-hooks](https://github.com/resq-software/dev/blob/main/AGENTS.md#git-hooks) for the full contract.

## References
- [Root README](README.md)
- [Individual Crate READMEs](crates/resq-cli/README.md, crates/resq-tui/README.md, etc.)
