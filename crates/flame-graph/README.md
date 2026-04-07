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

# resq-flame — CPU Profiler

Generates SVG flame graphs for ResQ services. Provides an interactive TUI for selecting profiling targets and subcommands to profile specific environments, piping the output through `inferno-flamegraph`.

## Prerequisites

```bash
# inferno — converts folded stack traces to SVG (required for all subcommands)
cargo install inferno

# py-spy — Python profiler (required for pdie / python subcommands)
pip install py-spy

# perf — Linux kernel profiler (required for offcpu subcommand)
# Usually installed via: sudo apt install linux-perf  or  sudo pacman -S perf
```

## Build

```bash
# Build from workspace root
cargo build --release -p resq-flame
```

Binary: `target/release/resq-flame`

## Usage

```bash
# Interactive TUI (default)
resq-flame

# Subcommand mode
resq-flame hce --duration 5000
```

## Profiling Targets (TUI)

The interactive TUI supports the following targets:

- **Coordination HCE**: Node.js/Bun service via HTTP metrics
- **Infrastructure API**: Rust backend via pprof
- **Intelligence PDIE**: Python AI engine via py-spy
- **Linux Perf**: System-wide profiling via `perf record`

## Subcommands

### `hce` — Coordination HCE

Connects to the `node:inspector` Session API exposed by the HCE service to take a V8 CPU profile.

```bash
resq-flame hce
resq-flame hce --url http://localhost:5000 --duration 5000
```

## Common Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--url <url>` | service default | Service base URL |
| `--token <jwt>` | `$RESQ_TOKEN` | Bearer token (HCE) |
| `--api-key <key>` | `$RESQ_API_KEY` | API key (infra-api, pdie) |
| `--duration <secs>` | `30` | Profile duration in seconds |
| `--output <path>` | `flamegraph.svg` | Output SVG path |
| `--pid <pid>` | — | Target process ID |

## Reading Flame Graphs

- **Width** of a frame = proportion of total samples where that function was on the stack
- **Height** = call depth (bottom = root, top = leaf)
- **Wide frames at the top** = hot leaf functions — primary optimization targets
- **Wide frames in the middle** = called frequently from many paths

Open the SVG in a browser for interactive search (`Ctrl+F`) and zoom by clicking frames.

## Typical Workflow

```bash
# 1. Start services
cargo run --manifest-path services/infrastructure-api/Cargo.toml

# 2. Generate load (simulation harness or curl loop)
cd services/simulation-harness && dotnet run

# 3. Profile while load is running
resq-flame api --duration 30 --output before.svg

# 4. Make optimization changes, restart, repeat
resq-flame api --duration 30 --output after.svg

# 5. Compare
resq-flame diff --before <(cat before.svg) --after <(cat after.svg) --output diff.svg
```

## Full Documentation

See [`docs/PROFILING_FLAMEGRAPH_GUIDE.md`](../../docs/PROFILING_FLAMEGRAPH_GUIDE.md) for all subcommands, auth setup, and worked examples.
