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

# health-checker — Service Health Monitor

Ratatui terminal UI that polls all ResQ service health endpoints and displays live status with latency. Doubles as a CI health gate via `--check`.

## Build

```bash
cargo build --release --manifest-path tools/Cargo.toml -p resq-health-checker
```

Binary: `tools/health-checker/target/release/health-checker`

## Usage

```bash
# Interactive TUI (auto-refreshes every 5 seconds)
health-checker

# CI mode — single check, exits non-zero on unhealthy services
health-checker --check

# Adjust refresh interval
health-checker --interval 10

# Run integration tests defined in a JSON file
health-checker --test tests/integration.json
```

## Services Monitored

| Service | Default URL | Health Endpoint |
|---------|-------------|-----------------|
| infrastructure-api | `http://localhost:8080` | `GET /health` |
| coordination-hce | `http://localhost:5000` | `GET /health` |
| intelligence-pdie | `http://localhost:8080` | `GET /health` |
| web-dashboard | `http://localhost:3000` | `GET /api/health` |

Requests time out after 5 seconds. Services that don't respond within the timeout are marked **Unhealthy**.

## TUI Layout

```
┌─────────────────────────────────────────────────────┐
│  ResQ Health Monitor          Last refresh: 14:23:01 │
├─────────────────────────────────────────────────────┤
│  ✅ infrastructure-api    HEALTHY      45ms          │
│  ✅ coordination-hce      HEALTHY      23ms          │
│  ⚠️  intelligence-pdie    DEGRADED   1250ms          │
│  ❌ web-dashboard         UNHEALTHY   timeout        │
├─────────────────────────────────────────────────────┤
│  [q] quit   [r] refresh   [Esc] quit                │
└─────────────────────────────────────────────────────┘
```

**Status levels**:
- `HEALTHY` — responded within timeout with a success status
- `DEGRADED` — responded but with high latency or partial health
- `UNHEALTHY` — timed out or returned an error
- `UNKNOWN` — not yet checked

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `r` | Force immediate refresh |

## CI Mode (`--check`)

In `--check` mode the TUI is not shown. Health checks run once and the process exits with:

| Exit Code | Meaning |
|-----------|---------|
| `0` | All services healthy |
| `1` | One or more services degraded |
| `2` | One or more services unhealthy / unreachable |

```bash
# Use in CI / pre-deploy gates
health-checker --check || { echo "Services not ready"; exit 1; }
```

## Integration Tests (`--test`)

Pass a JSON file defining HTTP assertions to run against the live services:

```json
[
  {
    "name": "infrastructure-api health",
    "method": "GET",
    "url": "http://localhost:5000/health",
    "expect_status": 200
  },
  {
    "name": "create incident",
    "method": "POST",
    "url": "http://localhost:5000/incidents",
    "body": { "incident_type": "FLOOD", "severity": "HIGH" },
    "expect_status": 201
  }
]
```

```bash
health-checker --test tools/health-checker/tests/smoke.json
```

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--check` | off | CI mode — single poll, exits with status code |
| `--interval <N>` | `5` | Refresh interval in seconds (TUI mode) |
| `--test <path>` | — | Path to integration test JSON file |
