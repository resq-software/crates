/*
 * Copyright 2026 ResQ
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! `ResQ` CLI - Command-line interface for managing `ResQ` services.

// The blanket `#![allow(clippy::pedantic)]` (which silenced ~70 lints) was
// replaced with this explicit, reviewed list. EVERY pedantic lint not named here
// stays active, so new code is still checked.
//
// Group 1 — deliberate choices for a large CLI surface. Fixing these means
// dropping `async`/`Result` from a uniform command-dispatch signature, threading
// borrows through clap handlers, or adding boilerplate `# Errors`/`# Panics`
// sections to self-describing anyhow-returning commands.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unused_async,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]
// Group 2 — low-value local nits (test float asserts, benign display-int casts,
// minor style). Most remaining occurrences live in secrets.rs / pre_commit.rs /
// gitignore.rs, which are being rewritten by in-flight PRs; fixing them here
// would only create merge conflicts. Track them down as a focused follow-up once
// those land, then delete the entries below.
#![allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::unreadable_literal,
    clippy::similar_names,
    clippy::items_after_statements,
    clippy::format_push_string,
    clippy::assigning_clones,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::must_use_candidate
)]
//!
//! This crate provides a unified CLI for interacting with the `ResQ` platform,
//! including service management, blockchain queries, and deployment operations.
//!
//! # Commands
//!
//! Grouped:
//! - `scan audit` — run cargo/bun/uv audit across the workspace
//! - `scan secrets` — scan for leaked credentials
//! - `scan copyright` — check or apply license headers
//! - `tui explore` / `logs` / `health` / `deploy` / `clean` / `asm` — TUI explorers
//!
//! Top-level:
//! - `format` — format Rust / TS / Python / C++ / C# in one pass
//! - `pre-commit` — full pre-commit gate (copyright, secrets, audit, format)
//! - `hooks` — inspect / update installed git hooks
//! - `dev` — repository utilities (workspace ops)
//! - `version` / `docs` / `commit` — release + docs + AI commit messages
//! - `completions` — emit shell completions for bash/zsh/fish/elvish/powershell
//!
//! Legacy flat forms (`resq audit`, `resq explore`, etc.) remain as hidden
//! aliases for one release cycle.
//!
//! # Usage
//!
//! ```bash
//! resq scan audit
//! resq format --check
//! resq pre-commit
//! resq tui health
//! resq completions bash > /usr/local/share/bash-completion/completions/resq
//! ```

#![deny(missing_docs)]

/// CLI command implementations.
pub mod commands;
/// Gitignore pattern utilities.
pub mod gitignore;
/// Shared utility functions.
pub mod utils;
