# resq-deploy Examples

Interactive deployment manager for Docker Compose and Kubernetes environments.

## Demo: Docker Compose Service Fleet

[`docker-compose.yml`](docker-compose.yml) defines 4 lightweight services using `python:3.12-alpine` that simulate the ResQ fleet. Each exposes health endpoints and runs real HTTP servers.

### Services included

| Service | Port | What it does |
|---------|------|-------------|
| infrastructure-api | 8080 | HTTP server with `/health` endpoint |
| coordination-hce | 5000 | HTTP server with `/health` and `/admin/status` |
| intelligence-pdie | 8000 | HTTP server with `/health` endpoint |
| web-dashboard | 3001 | Simple HTML page |

### Setup

```bash
# Copy to the expected location (resq-deploy looks in infra/docker/)
mkdir -p infra/docker
cp examples/resq-deploy/docker-compose.yml infra/docker/docker-compose.yml
```

### Run it

```bash
# Interactive TUI
cargo run -p resq-deploy

# Non-interactive: check status
cargo run -p resq-deploy -- --env dev --action status

# Non-interactive: bring everything up
cargo run -p resq-deploy -- --env dev --action up

# Non-interactive: view logs
cargo run -p resq-deploy -- --env dev --action logs

# Non-interactive: bring everything down
cargo run -p resq-deploy -- --env dev --action down

# Target a specific service
cargo run -p resq-deploy -- --env dev --service coordination-hce --action logs
```

### What you'll see

**Interactive TUI:**
- Left panel: list of 4 services with status (running/stopped)
- Right panel: action menu (status, build, up, down, restart, logs)
- Bottom panel: real-time command output streaming
- Tab between panels, Enter to execute

**Non-interactive (`--action status`):**
```json
[
  {"service": "infrastructure-api", "state": "running", "status": "Up 2 minutes"},
  {"service": "coordination-hce", "state": "running", "status": "Up 2 minutes"},
  {"service": "intelligence-pdie", "state": "running", "status": "Up 2 minutes"},
  {"service": "web-dashboard", "state": "running", "status": "Up 2 minutes"}
]
```

### Keyboard shortcuts (TUI)

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between Services and Actions |
| `↑`/`↓` | Navigate within focused panel |
| `Enter` | Execute selected action |
| `q` | Quit |

### Bonus: Use with resq-health and resq-perf

Once the docker-compose services are running, you can point the other tools at them:

```bash
# Monitor service health
cargo run -p resq-health

# Monitor performance metrics (coordination-hce exposes /admin/status)
cargo run -p resq-perf -- http://localhost:5000/admin/status
```
