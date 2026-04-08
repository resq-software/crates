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

# resq-deploy — Deployment Manager

Ratatui terminal UI for managing Docker Compose and Kubernetes deployments across dev, staging, and production environments. Provides an interactive 3-panel interface as well as a non-interactive `--action` flag for scripting.

## Build

```bash
# Build from workspace root
cargo build --release -p resq-deploy
```

Binary: `target/release/resq-deploy`

## Usage

```bash
# Interactive TUI (default: dev environment, Docker Compose)
resq-deploy

# Target a specific environment
resq-deploy --env staging

# Kubernetes mode
resq-deploy --k8s --env prod

# Non-interactive / CI: run a single action
resq-deploy --env dev --action up
resq-deploy --env prod --k8s --action deploy
resq-deploy --env dev --service infrastructure-api --action restart
```

## TUI Layout

```
┌── Services ──────┬── Actions ─────────┬── Output ─────────────────┐
│                  │                    │                             │
│ ● infra-api      │ > status           │ [14:22:01] Starting up...  │
│ ● coord-hce      │   build            │ [14:22:02] infra-api OK    │
│ ○ intelligence   │   up               │ [14:22:03] coord-hce OK    │
│ ○ web-dashboard  │   down             │ [14:22:04] All services up  │
│                  │   restart          │                             │
│                  │   logs             │                             │
│                  │                    │                             │
├──────────────────┴────────────────────┴─────────────────────────────┤
│ ENV: dev   [e] cycle env   [Tab] focus   [↑↓] select   [Enter] run  │
│ [q] quit                                                             │
└──────────────────────────────────────────────────────────────────────┘
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Cycle focus between panels |
| `↑` / `↓` | Navigate list in focused panel |
| `Enter` | Execute selected action |
| `e` | Cycle environment: dev → staging → prod |

## Docker Compose Actions

| Action | Description |
|--------|-------------|
| `status` | Show container status for all services |
| `build` | Build images (equivalent to `docker compose build`) |
| `up` | Start all services (`docker compose up -d`) |
| `down` | Stop and remove containers (`docker compose down`) |
| `restart` | Restart one or all services |
| `logs` | Tail logs (streams to Output panel or stdout) |

## Kubernetes Actions

| Action | Description |
|--------|-------------|
| `status` | `kubectl get pods -n resq-<env>` |
| `deploy` | `kubectl apply -k infra/k8s/overlays/<env>` |
| `destroy` | `kubectl delete -k infra/k8s/overlays/<env>` |
| `logs` | `kubectl logs -f deployment/<service> -n resq-<env>` |

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--env <env>` | `dev` | Target environment: `dev`, `staging`, `prod` |
| `--service <name>` | all | Scope action to a single service |
| `--k8s` | off | Use Kubernetes instead of Docker Compose |
| `--action <action>` | — | Non-interactive: run one action and exit |

## Environments

| Env | Docker Compose file | K8s overlay |
|-----|---------------------|-------------|
| `dev` | `infra/docker/docker-compose.yml` | `infra/k8s/overlays/dev` |
| `staging` | `infra/docker/docker-compose.staging.yml` | `infra/k8s/overlays/staging` |
| `prod` | `infra/docker/docker-compose.prod.yml` | `infra/k8s/overlays/prod` |

## CI / Scripting

```bash
# Bring up dev stack and wait
deploy-cli --env dev --action up
health-checker --check

# Deploy to prod (Kubernetes)
deploy-cli --env prod --k8s --action deploy

# Tear down staging
deploy-cli --env staging --action down
```
