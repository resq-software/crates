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

# resq-logs — Log Aggregator

Ratatui terminal UI that streams and aggregates logs from Docker Compose containers or local files. Supports live search, log-level filtering, and scrolling through a 10,000-line ring buffer.

## Build

```bash
# Build from workspace root
cargo build --release -p resq-logs
```

Binary: `target/release/resq-logs`

## Usage

```bash
# Stream all Docker Compose service logs
resq-logs --source docker

# Stream logs from a specific service only
resq-logs --source docker --service infrastructure-api

# Tail a log file
resq-logs --source file --path services/infrastructure-api/logs/api.log

# Start with error-level filter active
resq-logs --source docker --level error
```

## TUI Layout

```
┌─ ResQ Log Viewer ──────── [/] search  [f] filter: All  [c] clear ─┐
│ 14:23:01 INFO  [infra-api]  Incident INC-042 created               │
│ 14:23:01 INFO  [infra-api]  Uploading evidence to IPFS...          │
│ 14:23:02 INFO  [infra-api]  CID: QmXxx... pinned successfully      │
│ 14:23:02 WARN  [coord-hce]  Drone drone-07 telemetry delayed 2s    │
│ 14:23:03 ERROR [infra-api]  Neo RPC timeout after 5000ms           │
│ 14:23:03 INFO  [infra-api]  Retrying with exponential backoff...   │
│ ...                                                                  │
│                                                                      │
│ [search mode: "Neo"]                                                 │
├──────────────────────────────────────────────────────────────────────┤
│ 1247 lines  ↑↓ scroll  PgUp/PgDn  [g] bottom  [q] quit             │
└──────────────────────────────────────────────────────────────────────┘
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `/` | Enter search mode (type to filter visible lines) |
| `Esc` | Exit search mode |
| `f` | Cycle log-level filter: All → Error → Warn → Info → Debug → Trace |
| `c` | Clear all buffered lines |
| `g` | Jump to bottom / toggle follow mode |
| `↑` / `↓` | Scroll one line |
| `PgUp` / `PgDn` | Scroll one page |

## Log Sources

### Docker (`--source docker`)

Attaches to `docker compose logs --follow` for all services (or a single service with `--service`). Parses the Docker log prefix to extract service name and timestamp.

Requires a running Docker Compose stack. The viewer reads from whichever `docker-compose.yml` is found in `infra/docker/`.

### File (`--source file`)

Tails a local log file (equivalent to `tail -F`). Watches for file rotation.

```bash
resq-logs --source file --path /var/log/resq/infrastructure-api.log
```

## Log Levels

Filtering applies to the detected log level in each line. The viewer parses:
- Structured JSON logs: reads the `"level"` field
- Plain text logs: matches keywords `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`

When a filter is active, only lines at or above that level are shown. The current filter is displayed in the header.

## Buffer

The viewer keeps the most recent 10,000 lines in memory. Older lines are dropped as new ones arrive. Use `c` to manually clear the buffer.

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--source <src>` | `docker` | Log source: `docker` or `file` |
| `--path <path>` | — | File path (required for `--source file`) |
| `--service <name>` | all | Docker service name to filter |
| `--level <level>` | `all` | Initial level filter: `error`, `warn`, `info`, `debug`, `trace` |
