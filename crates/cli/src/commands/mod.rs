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

//! CLI commands for the ResQ tool.
//!
//! # Commands
//!
//! - [`audit`] - Audit blockchain events
//! - [`copyright`] - Check/update copyright headers
//! - [`cost`] - Estimate cloud costs
//! - [`lqip`] - Low-quality image placeholder generation
//! - [`secrets`] - Secret management
//! - [`tree_shake`] - Remove unused code

/// Blockchain event auditing.
pub mod audit;
/// Copyright header management.
pub mod copyright;
/// Cloud cost estimation.
pub mod cost;
/// Development server management.
pub mod dev;
/// Documentation management and publication.
pub mod docs;
/// Service exploration and operations.
pub mod explore;
/// Low-quality image placeholder generation.
pub mod lqip;
/// Pre-commit hook logic.
pub mod pre_commit;
/// Secret management.
pub mod secrets;
/// Unused code removal.
pub mod tree_shake;
/// Version management and changesets.
pub mod version;
