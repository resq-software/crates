# resq-health Examples

Service health monitor that polls health endpoints and displays status, latency, and error information.

## Demo: Mock Service Fleet

[`mock_services.py`](mock_services.py) starts 5 HTTP servers that simulate the full ResQ service fleet on the default ports resq-health expects.

### What the mock services do

| Service | Port | Endpoint | Behavior |
|---------|------|----------|----------|
| coordination-hce | 5000 | `/health` | Healthy, briefly degraded every 15s |
| infrastructure-api | 8080 | `/health` | Always healthy |
| intelligence-pdie | 8000 | `/health` | Healthy for 30s, then unhealthy (simulated OOM) |
| neo-n3-rpc | 20332 | JSON-RPC POST | Always responds to `getversion` |
| ipfs-gateway | 8081 | `/api/v0/version` | Always healthy |

### Run it

```bash
# Terminal 1: Start mock services
python3 examples/resq-health/mock_services.py

# Terminal 2: Launch the health dashboard
cargo run -p resq-health

# Or run a single check (CI mode)
cargo run -p resq-health -- --check

# Custom poll interval
cargo run -p resq-health -- --interval 2
```

### What you'll see

**Interactive TUI:**
- A table of all 5 services with real-time status updates
- Green "Healthy", yellow "Degraded", red "Unhealthy" indicators
- Latency in milliseconds for each service
- Watch `intelligence-pdie` go red after ~30 seconds
- Watch `coordination-hce` flicker to degraded periodically

**CI mode (`--check`):**
```
Checking 5 services...
  ✓ coordination-hce    12ms
  ✓ infrastructure-api   3ms
  ✓ intelligence-pdie    5ms
  ✓ neo-n3-rpc          8ms
  ✓ ipfs-gateway         4ms

Result: 5/5 healthy
```

(Run `--check` after 30 seconds to see `intelligence-pdie` fail.)

### Keyboard shortcuts (TUI)

| Key | Action |
|-----|--------|
| `↑`/`↓` | Scroll service list |
| `q` | Quit |
