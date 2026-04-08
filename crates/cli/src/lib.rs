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

#![allow(clippy::pedantic)]

//! `ResQ` CLI - Command-line interface for managing `ResQ` services.
//!
//! This crate provides a unified CLI for interacting with the `ResQ` platform,
//! including service management, blockchain queries, and deployment operations.
//!
//! # Commands
//!
//! - `status` - Check service health
//! - `deploy` - Deploy services to Kubernetes
//! - `logs` - View and filter logs
//! - `secrets` - Manage secrets
//! - `audit` - Audit blockchain events
//!
//! # Usage
//!
//! ```bash
//! resq status
//! resq deploy --service infrastructure-api
//! resq logs --service edge-aeai --level debug
//! ```

#![deny(missing_docs)]

/// CLI command implementations.
pub mod commands;
/// Gitignore pattern utilities.
pub mod gitignore;
/// Shared utility functions.
pub mod utils;
