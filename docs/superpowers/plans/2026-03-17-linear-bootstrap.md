# Linear Bootstrap Tool Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a safe `plan`/`apply` Linear bootstrap script to the `cli` repo for the existing `resq` team and selected workspace-level objects.

**Architecture:** Store desired Linear state in a checked-in JSON file, fetch current state via GraphQL, compute an additive-only plan, and only mutate when `apply` is explicitly requested.

**Tech Stack:** Bash, curl, jq, GraphQL API, checked-in JSON config

---

## Chunk 1: Tool Skeleton and Red/Green Harness

### Task 1: Create the script, config, and test harness

**Files:**
- Create: `config/linear/resq.json`
- Create: `scripts/linear-bootstrap.sh`
- Create: `scripts/tests/linear-bootstrap-smoke.sh`

- [ ] **Step 1: Write the failing smoke test**

Use a mocked GraphQL endpoint and verify that `plan` currently fails because the script does not exist.

- [ ] **Step 2: Add the initial config**

Define the `resq` team plus workspace/team objects that should be managed.

- [ ] **Step 3: Add the script skeleton**

Support `validate`, `plan`, and `apply` modes with argument parsing and shared helpers.

- [ ] **Step 4: Run the smoke test**

Confirm the first targeted expectation now passes.

## Chunk 2: Planning Logic

### Task 2: Compute additive creates/updates safely

**Files:**
- Modify: `scripts/linear-bootstrap.sh`
- Modify: `scripts/tests/linear-bootstrap-smoke.sh`

- [ ] **Step 1: Extend the failing test**

Assert that `plan` prints the expected create/update actions from mocked current state.

- [ ] **Step 2: Implement GraphQL fetch + diff logic**

Fetch workspace/team state and compute additive-only actions.

- [ ] **Step 3: Re-run the smoke test**

Confirm `plan` output matches expected actions.

## Chunk 3: Apply Mode and Docs

### Task 3: Execute approved actions and document usage

**Files:**
- Modify: `scripts/linear-bootstrap.sh`
- Modify: `scripts/tests/linear-bootstrap-smoke.sh`
- Modify: `README.md`

- [ ] **Step 1: Extend the failing test**

Assert that `apply` sends only the intended mocked mutations.

- [ ] **Step 2: Implement `apply`**

Execute the exact action set emitted by the planner.

- [ ] **Step 3: Document usage**

Add a short README section covering config location, `LINEAR_API_KEY`, and `plan/apply`.

- [ ] **Step 4: Re-run the smoke test**

Confirm validate/plan/apply all behave correctly under the mocked API.
