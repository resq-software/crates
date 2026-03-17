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

# Rust Workspace Optimization Design

## Goal

Improve this `cli` workspace by borrowing the strongest Rust ergonomics from `~/github/wrk/resQ` and applying them in a targeted way: better root-level Rust configuration, less manifest drift across crates, and docs that match the actual developer workflow.

## Context

The current workspace already has a solid root [`Cargo.toml`](../../../Cargo.toml) with shared dependencies and basic workspace lints, but it is missing several repo-level conventions present in `resQ`:

- A pinned Rust toolchain via `rust-toolchain.toml`
- Shared cargo aliases and environment defaults via `.cargo/config.toml`
- Explicit workspace build profiles for better developer build performance
- Slightly stricter workspace lint policy

The member crates also show small but real manifest drift, including dependencies that bypass the workspace version table without an obvious reason. The documentation also mixes commands and workflows in ways that do not reflect a single recommended Rust developer path.

## Non-Goals

- Do not redesign packaging or release policy
- Do not rename binaries, crates, or commands
- Do not introduce aggressive lint policy that creates large warning debt
- Do not copy `resQ`-specific commands or branding that do not fit this repo

## Proposed Changes

### 1. Root Rust Configuration

Add root-level Rust workspace ergonomics that transfer cleanly from `resQ`:

- Add `rust-toolchain.toml` pinned to `stable`
- Include `rustfmt` and `clippy` components
- Add `.cargo/config.toml` with a focused alias set for this repo
- Add root workspace build profiles:
  - `[profile.dev] opt-level = 1`
  - `[profile.dev.package."*"] opt-level = 3`
  - `[profile.release] lto = true`
  - `[profile.release] codegen-units = 1`
  - `[profile.release] strip = true`
- Add `unreachable_pub = "warn"` to workspace rust lints if it passes cleanly

The alias set should stay small and tied to the real tools in this workspace. The likely baseline is:

- `t = "test --workspace"`
- `c = "clippy --workspace --all-targets -- -D warnings"`
- `check-all = "check --workspace --all-targets"`
- direct run aliases for the main binaries where they improve local ergonomics

### 2. Targeted Manifest Normalization

Normalize crate manifests where the current drift adds maintenance cost without delivering a clear benefit.

This includes:

- Preserving shared package metadata inheritance
- Moving obvious shared dependency versions back to `[workspace.dependencies]`
- Normalizing internal path dependency declarations where a single style can be used consistently
- Leaving crate-specific metadata local when it is package-specific

This does not include:

- Forcing all crates onto `version.workspace = true`
- Refactoring dependency features without evidence
- Any naming or packaging policy changes

### 3. Documentation Alignment

Update docs so the repo has one clear Rust workflow.

This includes:

- Root [`README.md`](../../../README.md) updates for:
  - pinned stable toolchain
  - preferred cargo aliases
  - preferred local check, test, and lint commands
- Crate README adjustments where the commands should reflect the workspace conventions
- Removing or replacing stale command guidance where it conflicts with the new root workflow

## Risks

### Lint strictness

Even a small lint increase can expose existing issues. The design intentionally limits lint expansion to changes likely to be low-noise.

### Dependency normalization

Centralizing dependency versions can surface feature mismatches if a crate was relying on a local override. Each normalization change needs verification.

### Build profile changes

The proposed profile settings are standard and low risk, but they can change developer build behavior enough that the workspace should be checked and tested immediately after the change.

## Verification Strategy

Verification should happen in layers after each meaningful chunk:

1. `cargo check --workspace`
2. `cargo test --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. Spot-check the new cargo aliases from `.cargo/config.toml`
5. Spot-check at least one binary build path in release mode if profile changes land

If a proposed lint or normalization change creates disproportionate churn, back it out and keep the repo on the simpler version.

## Expected Outcome

After this work:

- Developers get a consistent Rust toolchain and cargo UX without relying on tribal knowledge
- Common workspace tasks become faster and easier to remember
- Member manifests become cheaper to maintain
- Docs point to a single, coherent Rust workflow for the repo
