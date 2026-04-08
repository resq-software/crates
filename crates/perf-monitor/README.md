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

# resq-perf — Performance Dashboard

Ratatui terminal UI showing live CPU usage, memory consumption, and request metrics for ResQ services. Pulls data from the services' `/status` endpoints (compatible with `coordination-hce`).

## Build

```bash
# Build from workspace root
cargo build --release -p resq-perf
```

Binary: `target/release/resq-perf`

## Usage

```bash
# Monitor HCE service (default: http://localhost:3000/admin/status)
resq-perf

# Target a specific service status URL (positional argument)
resq-perf http://localhost:3000/admin/status

# Authenticated service (reads RESQ_TOKEN env var)
resq-perf --token eyJhbGc...

# Adjust refresh rate in milliseconds
resq-perf --refresh-ms 1000
```

Environment variables are also accepted:

```bash
export RESQ_TOKEN=eyJhbGc...
resq-perf http://localhost:3000/admin/status
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

The performance monitor supports Bearer token authentication via the `--token` flag or the `RESQ_TOKEN` environment variable. This is compatible with the `coordination-hce` service's admin endpoints.

## Flags

| Argument / Flag | Default | Description |
|------|---------|-------------|
| `url` | `http://localhost:3000/admin/status` | Positional: status endpoint to monitor |
| `--token <jwt>` | `$RESQ_TOKEN` | Bearer token for authenticated services |
| `--refresh-ms <ms>` | `500` | Refresh interval in milliseconds |

## Related

For point-in-time CPU flame graphs rather than live dashboards, use [`flame-graph`](../flame-graph/README.md).

For comprehensive profiling workflows see [`docs/PROFILING_FLAMEGRAPH_GUIDE.md`](../../docs/PROFILING_FLAMEGRAPH_GUIDE.md).
