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

//! Tree-shaking command for removing unused code.
//!
//! Analyzes JavaScript/TypeScript bundles to identify and report
//! unused code that can be removed to reduce bundle size.

use anyhow::{Context, Result};
use std::process::Command;

/// CLI arguments for the tree-shake command.
#[derive(clap::Args, Debug)]
pub struct TreeShakeArgs {}

/// Run the tree-shake command.
pub async fn run(_args: TreeShakeArgs) -> Result<()> {
    println!("Running tree-shake (tsr)...");

    let root = crate::utils::find_project_root();

    let status = Command::new("bunx")
        .args([
            "tsr",
            "--write",
            "--recursive",
            r"^src/(main|index)\.ts$",
            r"^src/app/.*\.(ts|tsx)$",
        ])
        .current_dir(&root)
        .status()
        .context("Failed to execute tsr")?;

    if !status.success() {
        anyhow::bail!("Tree-shake failed");
    }

    println!("Tree-shake completed successfully.");
    Ok(())
}
