# resq-cli

![CI](https://img.shields.io/github/actions/workflow/status/resq-software/cli/ci.yml?branch=main&label=ci&style=flat-square)
![crates.io](https://img.shields.io/crates/v/resq-cli?style=flat-square)
![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square)

A unified Rust-based CLI and TUI toolchain designed to streamline developer workflows for the ResQ autonomous drone platform. `resq-cli` consolidates auditing, security, performance monitoring, and deployment orchestration into a single, cohesive binary architecture.

---

## Overview

The `resq` ecosystem is a high-performance Rust monorepo. It distinguishes between the end-user CLI experience (the `resq` binary) and the internal library architecture (`resq-tui`) which powers consistent terminal interfaces across all tooling.

```mermaid
graph TD
    subgraph CLI_Workspace
        CLI[resq-cli]
        TUI_LIB[resq-tui]
    end

    subgraph Tool_Modules
        Health[resq-health]
        Deploy[resq-deploy]
        Explorer[bin-explorer]
        Perf[perf-monitor]
    end

    CLI --> TUI_LIB
    CLI --> Health
    CLI --> Deploy
    CLI --> Explorer
    CLI --> Perf
```

---

## Features

*   **Security First:** Integrated secret scanning and repository auditing.
*   **Performance Monitoring:** Real-time metrics and flame graph generation for polyglot services.
*   **Orchestration:** TUI-based Kubernetes and Docker Compose deployment management.
*   **Developer Productivity:** Automated copyright header enforcement, tree-shaking, and `.gitignore`-aware workspace cleaning.
*   **Unified TUI Library:** Shared component library (`resq-tui`) ensuring consistent UX across the entire toolchain.

---

## Installation

### Prerequisites
- **Nix:** Recommended for reproducible development environments.
- **Rust:** Latest stable toolchain.

### Via Cargo
```sh
cargo install resq-cli
```

### From Source
```sh
git clone https://github.com/resq-software/cli.git
cd cli
cargo build --release --workspace
```

### Nix Troubleshooting
If you encounter environment issues within `nix develop`:
1. **Clean state:** Ensure no conflicting `CARGO_HOME` or `RUSTUP_HOME` variables are bleeding from your local shell into the Nix shell.
2. **Library Paths:** If native dependencies fail to link, run `nix-collect-garbage -d` followed by a re-entry into `nix develop`.
3. **Caching:** If builds are unexpectedly slow, ensure `~/.cache/nix` is accessible and not exceeding disk quotas.

---

## Quick Start

1. **Bootstrap local environment:**
   ```sh
   ./bootstrap.sh
   ```
2. **Run a security audit:**
   ```sh
   resq audit
   ```
3. **Clean build artifacts:**
   ```sh
   resq clean
   ```

---

## Usage

### Interactive Pre-commit Audit
```sh
resq pre-commit
```

### Service Health Monitoring
```sh
resq health
```

### Binary Analysis
```sh
resq asm --file ./path/to/binary
```

---

## Configuration

| Environment Variable | Description |
| :--- | :--- |
| `GIT_HOOKS_SKIP` | Disables automated pre-commit hooks. |
| `RESQ_NIX_RECURSION` | Internal safety flag for recursive execution in Nix environments. |

---

## Development

The project utilizes `Nix` to maintain consistency across team environments.

1. **Environment:** Enter the shell with `nix develop`.
2. **Testing:** Execute `cargo nextest run` for optimized parallel testing.
3. **Consistency:** Always keep `AGENTS.md` and `CLAUDE.md` in sync using `./agent-sync.sh`.

---

## Contributing

We strictly adhere to [Conventional Commits](https://www.conventionalcommits.org/).

1. **Branching:** Use `feat/`, `fix/`, or `refactor/` prefixes.
2. **Quality:** Run `cargo clippy --workspace -- -D warnings` before submitting.
3. **Automation:** Ensure all CI workflows (including `osv-scan`) pass.
4. **License Headers:** Run `resq copyright` to automatically update headers on all new source files.

---

## License

Copyright 2026 ResQ. Licensed under the [Apache License, Version 2.0](./LICENSE).