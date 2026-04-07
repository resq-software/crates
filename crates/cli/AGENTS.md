# resq-cli — Agent Guide

## Mission
Developer CLI for the ResQ platform. Provides a unified command-line interface for blockchain auditing, copyright management, secret scanning, dependency analysis, and launching service explorers.

## Stack
- Runtime: Rust
- CLI: Clap
- Serialization: Serde + Toml
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — Main entry point and command routing
- `src/lib.rs` — Library interface
- `src/commands/` — Individual command implementations
- `src/commands/explore.rs` — TUI tool launcher logic
- `README.md` — User documentation
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p resq-cli
cargo test -p resq-cli
```

## Rules
- New commands should be added as modules in `src/commands/`.
- Use `resq-tui` for any interactive CLI elements.
- Ensure commands degrade gracefully for CI usage (exit codes, non-interactive modes).
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Never log or store secrets found during `secrets` scan.
- Ensure `copyright` modifications are atomic and respect shebangs.

## Workflow
1. Run `cargo build -p resq-cli` before finalizing code changes.
2. Run `cargo test -p resq-cli` to verify command logic.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, new commands (if any), and test results.

## References
- [README](README.md)
- [Tools Guide](../../AGENTS.md)
