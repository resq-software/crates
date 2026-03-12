# deploy-cli — Agent Guide

## Mission
Interactive deployment manager for ResQ environments. Coordinates Docker Compose and Kubernetes actions across dev, staging, and prod from a TUI or single-action CLI mode.

## Stack
- Runtime: Rust
- UI: Ratatui + Crossterm
- CLI: Clap
- Async runtime: Tokio
- Shared UI: `resq-tui`

## Repo Map
- `src/main.rs` — CLI parsing and application entry point
- `src/docker.rs` — Docker Compose actions
- `src/k8s.rs` — Kubernetes actions and overlay targeting
- `README.md` — Operator usage and action matrix
- `Cargo.toml` — Crate manifest

## Commands
```bash
cargo build -p deploy-cli
cargo test -p deploy-cli
```

## Rules
- Every mutating action must support a non-interactive path for automation.
- Environment selection must stay explicit: `dev`, `staging`, or `prod`.
- Keep Docker and Kubernetes behaviors aligned with `infra/docker/` and `infra/k8s/overlays/`.
- `AGENTS.md` is the source of truth for the local `CLAUDE.md` mirror — never edit `CLAUDE.md` directly.

## Safety
- Do not default destructive actions (`down`, `destroy`) without clear operator intent.
- Production actions must target the correct overlay or namespace and remain auditable in output.

## Workflow
1. Run `cargo build -p deploy-cli` before finalizing code changes.
2. Verify the affected action path against the matching Docker Compose file or K8s overlay.
3. If you edited any `AGENTS.md`, run `./agent-sync.sh` from the repo root before finishing.
4. Summarize: files changed, behavior change, and verification run.

## References
- [README](README.md)
- [Infrastructure Guide](../../infra/AGENTS.md)
- [Tools Guide](../AGENTS.md)
