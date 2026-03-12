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

# flame-graph — CPU Profiler

Generates SVG flame graphs for every ResQ service. Each subcommand knows how to profile its target: calling the right endpoint, running the right profiler binary, and piping the output through `inferno-flamegraph`.

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
cargo build --release --manifest-path tools/Cargo.toml -p resq-flame-graph
```

Binary: `tools/flame-graph/target/release/flame-graph`

## Subcommands

### Service Profilers

#### `hce` — coordination-hce (Node.js/Bun)

Connects to the `node:inspector` Session API exposed by the HCE service to take a V8 CPU profile.

```bash
flame-graph hce
flame-graph hce --url http://localhost:3000 --token $RESQ_TOKEN --duration 30 --output hce.svg
```

Requires the service to be running with `--inspect` or with the `/admin/cpu-profile` endpoint enabled.

#### `api` — infrastructure-api (Rust/pprof-rs)

Hits the `/admin/profile` endpoint which triggers `pprof-rs` CPU sampling (SIGPROF-based).

```bash
flame-graph api
flame-graph api --url http://localhost:8080 --api-key $RESQ_API_KEY --duration 30 --output api.svg
```

Returns folded stacks in the pprof protobuf format, converted internally to the inferno input format.

#### `pdie` — intelligence-pdie (Python/py-spy)

Attaches `py-spy` to the running PDIE process and samples its call stack.

```bash
flame-graph pdie
flame-graph pdie --url http://localhost:8000 --duration 30 --output pdie.svg
```

Requires `py-spy` on `PATH` and the ability to attach to the process (may need `sudo` on some systems).

### General Profilers

#### `python <pid>` — Attach py-spy to any Python process

```bash
flame-graph python --pid 12345 --duration 30 --output python.svg
```

#### `file <path>` — Convert existing folded stacks file to SVG

```bash
flame-graph file --input stacks.txt --output flamegraph.svg
```

Input format: one line per stack sample in `inferno` folded format:
```
root;parent;leaf 42
root;parent;other_leaf 17
```

#### `perf` — Linux `perf record` + `perf script`

Runs `perf record` on a PID for the given duration, then converts the output.

```bash
# Profile a specific PID
flame-graph perf --pid 12345 --duration 30 --output perf.svg

# Profile a command
flame-graph perf --cmd "cargo bench" --output bench.svg
```

#### `bun` — Bun runtime CPU profile

Collects a Bun CPU profile by attaching to the running Bun process.

```bash
flame-graph bun --pid $(pgrep -f "bun run") --duration 30 --output bun.svg
```

#### `dotnet` — .NET simulation-harness

Generates a flame graph for the .NET simulation-harness using `dotnet-trace`.

```bash
flame-graph dotnet
flame-graph dotnet --pid $(pgrep -f ResQ.SimulationHarness) --duration 30 --output dotnet.svg
```

Internally runs:
```
dotnet-trace collect --process-id <pid> --providers cpu-sampling --duration 00:00:<N>
```

### Analysis Subcommands

#### `offcpu` — Off-CPU time analysis

Captures time threads spend blocked (I/O, locks, syscalls) rather than running on CPU.

```bash
flame-graph offcpu --pid 12345 --duration 30 --output offcpu.svg
```

Requires `perf` with eBPF support or `bpftrace`.

#### `diff` — Differential flame graph

Compares two folded stack files and highlights regressions in red, improvements in blue.

```bash
flame-graph diff --before before.txt --after after.txt --output diff.svg
```

#### `hotcold` — Hot/cold flame graph

Color-codes frames by sample frequency: red (hot) → blue (cold).

```bash
flame-graph hotcold --input stacks.txt --output hotcold.svg
```

#### `memory` — Memory allocation flame graph

Profiles heap allocations rather than CPU time.

```bash
# Rust (via jemalloc profiling)
flame-graph memory --pid 12345 --output memory.svg

# Python (via tracemalloc snapshot)
flame-graph memory --pid 12345 --lang python --output memory.svg
```

#### `explain` — AI-assisted analysis

Sends the top frames from a flame graph to the Gemini AI endpoint for plain-English explanation of hot spots.

```bash
flame-graph explain --input flamegraph.svg --api-url http://localhost:5000
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
flame-graph api --duration 30 --output before.svg

# 4. Make optimization changes, restart, repeat
flame-graph api --duration 30 --output after.svg

# 5. Compare
flame-graph diff --before <(cat before.svg) --after <(cat after.svg) --output diff.svg
```

## Full Documentation

See [`docs/PROFILING_FLAMEGRAPH_GUIDE.md`](../../docs/PROFILING_FLAMEGRAPH_GUIDE.md) for all subcommands, auth setup, and worked examples.
