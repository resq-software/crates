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

# perf-monitor — Performance Dashboard

Ratatui terminal UI showing live CPU usage, memory consumption, and request metrics for ResQ services. Pulls data from the services' `/metrics` endpoints.

## Build

```bash
cargo build --release --manifest-path tools/Cargo.toml -p resq-perf-monitor
```

Binary: `tools/perf-monitor/target/release/perf-monitor`

## Usage

```bash
# Monitor all services (default: http://localhost:5000)
perf-monitor

# Target a specific service URL
perf-monitor --url http://localhost:3000

# Authenticated service (coordination-hce uses JWT)
perf-monitor --url http://localhost:3000 --token eyJhbGc...

# Infrastructure API uses RESQ_API_KEY header
perf-monitor --url http://localhost:5000 --api-key your-api-key

# Adjust refresh rate (default: 2 seconds)
perf-monitor --refresh 5
```

Environment variables are also accepted:

```bash
export RESQ_TOKEN=eyJhbGc...
export RESQ_API_KEY=your-key
perf-monitor --url http://localhost:3000
```

## TUI Layout

```
┌─ ResQ Performance Monitor ──────────────────────────────────┐
│ Service: coordination-hce @ http://localhost:3000  [Tab: next]│
├────────────────────────────────────────────────────────────┤
│ CPU Usage          Memory              Requests/sec         │
│ ████████░░ 78%     ██████░░ 1.2/2 GB   ████░░ 207 req/s    │
├────────────────────────────────────────────────────────────┤
│ Latency Percentiles                                        │
│   p50:  45ms    p95: 120ms    p99: 350ms                   │
├────────────────────────────────────────────────────────────┤
│ Recent (60s)                                               │
│ CPU  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅                           │
│ Mem  ▃▃▄▄▄▅▅▅▅▄▄▄▄▃▃▃▄▄▅▅▅▅▄▄▄                           │
├────────────────────────────────────────────────────────────┤
│ [q] quit   [Tab] next service   [r] refresh                │
└────────────────────────────────────────────────────────────┘
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Cycle to next service |
| `r` | Force immediate refresh |

## Authentication

| Service | Auth header | Flag |
|---------|-------------|------|
| coordination-hce | `Authorization: Bearer <token>` | `--token` / `RESQ_TOKEN` |
| infrastructure-api | `X-API-Key: <key>` | `--api-key` / `RESQ_API_KEY` |
| intelligence-pdie | `X-API-Key: <key>` | `--api-key` / `RESQ_API_KEY` |

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--url <url>` | `http://localhost:5000` | Service base URL |
| `--token <jwt>` | `$RESQ_TOKEN` | Bearer token for JWT-authenticated services |
| `--api-key <key>` | `$RESQ_API_KEY` | API key for key-authenticated services |
| `--refresh <secs>` | `2` | Metrics refresh interval in seconds |

## Related

For point-in-time CPU flame graphs rather than live dashboards, use [`flame-graph`](../flame-graph/README.md).

For comprehensive profiling workflows see [`docs/PROFILING_FLAMEGRAPH_GUIDE.md`](../../docs/PROFILING_FLAMEGRAPH_GUIDE.md).
