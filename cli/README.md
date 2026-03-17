<!--
  Copyright 2026 ResQ

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# resq — Developer CLI

Rust developer tooling CLI for the ResQ monorepo. Handles license headers, image placeholders, security audits, dependency cost analysis, secret scanning, and provides a suite of TUI explorers for logs, performance, health, and deployments.

## Build & Install

```bash
# Build from workspace root
cargo build --release -p resq-cli

# Preferred day-to-day developer entrypoint
cargo resq help

# Optional: install globally
cargo install --path cli
```

Binary: `target/release/resq`

Additional workspace aliases are defined in `.cargo/config.toml`, including `cargo check-all`, `cargo t`, `cargo c`, `cargo health`, `cargo logs`, `cargo perf`, `cargo deploy`, `cargo cleanup`, `cargo bin`, and `cargo flame`.

## Commands

### `copyright` — License Header Management

Adds or checks copyright headers across every source file in the repo.

**Supported formats**: C-style block (`/** */`), XML/HTML (`<!-- -->`), hash-line (`#`), double-dash (`--`), Elisp (`;;`), AsciiDoc (`////`). Shebangs (`#!/...`) are always preserved at line 0.

**Licenses**: `mit` (default), `apache-2.0`, `gpl-3.0`, `bsd-3-clause`

```bash
# Check all tracked files (CI — exits 1 if any missing)
resq copyright --check

# Preview what would be added without writing
resq copyright --dry-run

# Add headers to all files missing them
resq copyright

# Overwrite existing headers (e.g. change license or author)
resq copyright --force --license apache-2.0 --author "Acme Corp" --year 2026
```

---

### `lqip` — Low-Quality Image Placeholders

Generates tiny base64-encoded data URIs from images for use as blur-up placeholders in the web dashboard.

```bash
# Single image → prints data URI
resq lqip --target services/web-dashboard/public/hero.jpg

# Directory of images → text list
resq lqip --target services/web-dashboard/public/

# Recursive with JSON output (for import into JS)
resq lqip --target services/web-dashboard/public/ --recursive --format json
```

---

### `audit` — Security & Quality Audit

Three-pass security and quality sweep covering all language ecosystems in the monorepo. Runs OSV Scanner (cross-ecosystem), npm audit-ci, and React Doctor.

```bash
# Full audit (all three passes)
resq audit

# Run only the OSV Scanner pass
resq audit --skip-npm --skip-react
```

---

### `cost` — Dependency Size Analysis

Fetches package sizes from registries (npm, crates.io, PyPI) and categorizes dependencies by download footprint.

```bash
# Auto-detect project type and analyze
resq cost

# Specific project
resq cost --root services/coordination-hce
```

---

### `secrets` — Secret Scanner

Scans source files for hardcoded credentials, API keys, private keys, tokens, and high-entropy strings.

```bash
# Scan all git-tracked files (default)
resq secrets

# Only scan staged changes (pre-commit hook)
resq secrets --staged
```

---

### `tree-shake` — TypeScript Dead Code Removal

Runs [`tsr`](https://github.com/line/ts-remove-unused) to remove unused TypeScript exports from the project entry points.

```bash
resq tree-shake
```

---

### `dev` — Development Utilities

Unified entry point for repository-level development tasks.

```bash
# List all development scripts
resq dev --list

# Run a specific developer workflow
resq dev run setup-env
```

---

### `explore` — Performance Monitor (TUI)

Launches the `resq-perf` TUI to monitor live CPU usage, memory consumption, and request metrics for ResQ services (primarily `coordination-hce`).

```bash
# Launch the performance explorer
resq explore
```

Dedicated TUI shortcuts are also available:
- `resq logs` — Multi-source log aggregator (launches `resq-logs`)
- `resq health` — Service health dashboard (launches `resq-health`)
- `resq deploy` — Docker/K8s deployment manager (launches `resq-deploy`)
- `resq clean` — Visual workspace cleaner (launches `resq-clean`)
- `resq asm` — Machine code & binary analyzer (launches `resq-bin`)

---

### `pre-commit` — Unified Hook

Runs a suite of checks suitable for a git pre-commit hook: copyright headers, secret scanning, and basic audits.

```bash
resq pre-commit
```

---

### `version` — Monorepo Versioning

Manages package versions and changesets across the monorepo.

```bash
# Check version status
resq version status

# Bump versions based on changesets
resq version bump
```

---

### `docs` — Documentation Export

Manages documentation generation, export, and publication for the monorepo.

```bash
# Export all documentation to HTML/PDF
resq docs export --format html
```
