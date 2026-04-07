# resq-clean — Agent Guide

## Mission
Visual workspace cleaner for ResQ. Analyzes build artifacts and gitignored files to help developers reclaim disk space and maintain a clean repository.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- Path handling: `ignore`, `walkdir`
- CLI: Clap
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — TUI application and scanning logic
- `README.md` — Operator usage and artifact types
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-clean
cargo test -p resq-clean
```

## Rules
- Always respect `.gitignore` rules via the `ignore` crate.
- Never delete `.env` files even if they are gitignored.
- Provide a `dry_run` mode that logs but does not execute deletions.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Be extremely cautious with recursive deletions.
- Ensure the user has visual confirmation (TUI) or explicit intent (`--dry-run` vs final) before removing data.

## Workflow
1. Run `cargo build -p resq-clean` before finalizing code changes.
2. Verify scanning logic against various `.gitignore` patterns.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, artifact detection improvements, and test output.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
