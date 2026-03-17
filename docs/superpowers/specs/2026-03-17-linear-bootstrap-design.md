# Linear Bootstrap Tool Design

## Goal

Provide a safe, repo-owned terminal tool that bootstraps the existing `resq` Linear team plus selected workspace-level objects from a checked-in config file.

## Context

The desired workflow is:

- checked-in config, not interactive prompts
- additive-only behavior
- explicit `plan` and `apply` phases
- lightweight script, not a first-class Rust subcommand yet

The `cli` repo already has a scripts-based tooling style and a Nix shell that includes `jq`, making a shell + API implementation appropriate.

## Decision

Implement a bash script in `cli/scripts/linear-bootstrap.sh` backed by a checked-in JSON config file at `cli/config/linear/resq.json`.

## Architecture

### Inputs

- `LINEAR_API_KEY` environment variable
- checked-in JSON config
- optional `LINEAR_API_URL` override for tests

### Commands

- `validate`: verify config shape and required env vars
- `plan`: fetch current Linear state and print additive create/update actions
- `apply`: execute the exact plan actions

### Managed scope

Workspace-level:
- labels
- project templates

Team-level for existing `resq` team:
- cycle settings
- issue statuses / workflow
- issue templates

### Safety rules

- additive-only: create missing objects, update safe mutable fields, never delete
- fail on ambiguous matches
- no implicit apply during `plan`

## Testing

- use a mocked GraphQL endpoint via `LINEAR_API_URL`
- verify `validate` catches bad config
- verify `plan` prints expected creates/updates from sample current state
- verify `apply` sends only the intended mutations
