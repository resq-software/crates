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

//! Version management command - Polyglot Changeset Implementation.

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

/// Arguments for the version command.
#[derive(Args)]
pub struct VersionArgs {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: VersionCommands,
}

/// Commands for version management.
#[derive(Subcommand)]
pub enum VersionCommands {
    /// Create a new changeset (intent to change version)
    Add(AddArgs),
    /// Consume changesets and apply version bumps across the monorepo
    Apply(ApplyArgs),
    /// Check if all versions are synchronized
    Check,
}

/// Arguments for adding a changeset.
#[derive(Args)]
pub struct AddArgs {
    /// Type of change (patch, minor, major)
    #[arg(short, long, default_value = "patch")]
    pub bump: String,
    /// Summary of what changed
    #[arg(short, long)]
    pub message: String,
}

/// Arguments for applying version bumps.
#[derive(Args)]
pub struct ApplyArgs {
    /// Dry run - see what would change without modifying files
    #[arg(long)]
    pub dry_run: bool,
}

/// Run the version command
pub fn run(args: VersionArgs) -> Result<()> {
    match args.command {
        VersionCommands::Add(add_args) => add_changeset(add_args),
        VersionCommands::Apply(apply_args) => apply_versions(apply_args),
        VersionCommands::Check => check_versions(),
    }
}

fn add_changeset(args: AddArgs) -> Result<()> {
    let root = get_repo_root()?;
    let changeset_dir = root.join(".changesets");
    fs::create_dir_all(&changeset_dir)?;

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("{}-{}.md", timestamp, args.bump);
    let path = changeset_dir.join(filename);

    let content = format!("---\nbump: {}\n---\n\n{}", args.bump, args.message);

    fs::write(&path, content)?;
    println!(
        "✅ Created changeset: .changesets/{}",
        path.file_name().unwrap().to_string_lossy()
    );
    Ok(())
}

fn apply_versions(args: ApplyArgs) -> Result<()> {
    let root = get_repo_root()?;
    let changeset_dir = root.join(".changesets");

    if !changeset_dir.exists() {
        println!("ℹ️  No changesets found. Nothing to apply.");
        return Ok(());
    }

    let entries = fs::read_dir(&changeset_dir)?;
    let mut bump_type = "patch"; // Default

    let mut messages = Vec::new();
    let mut files_to_delete = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let content = fs::read_to_string(&path)?;

            // Basic "frontmatter" parsing
            if content.contains("bump: major") {
                bump_type = "major";
            } else if content.contains("bump: minor") && bump_type != "major" {
                bump_type = "minor";
            }

            // Extract message (everything after the second ---)
            if let Some(msg) = content.split("---").last() {
                messages.push(msg.trim().to_string());
            }
            files_to_delete.push(path);
        }
    }

    if messages.is_empty() {
        println!("ℹ️  No valid changesets found.");
        return Ok(());
    }

    // Determine current version from root package.json
    let current_version = get_current_version(&root)?;
    let next_version = bump_version(&current_version, bump_type)?;

    println!("🚀 Bumping version: {current_version} -> {next_version} ({bump_type})");

    if args.dry_run {
        println!("🏃 DRY RUN: Would update all manifests to {next_version}");
        return Ok(());
    }

    // Apply to all manifests
    update_manifests(&root, &current_version, &next_version)?;

    // Append to CHANGELOG.md
    update_changelog(&root, &next_version, &messages)?;

    // Cleanup changesets
    for file in files_to_delete {
        fs::remove_file(file)?;
    }

    println!("✨ Successfully synchronized all versions to {next_version}!");
    Ok(())
}

fn check_versions() -> Result<()> {
    let root = get_repo_root()?;
    let version = get_current_version(&root)?;

    let manifests = [
        root.join("package.json"),
        root.join("Cargo.toml"),
        root.join("pyproject.toml"),
        root.join("Directory.Build.props"),
    ];

    let mut out_of_sync = false;
    for path in manifests {
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            if !content.contains(&version) {
                println!(
                    "❌ Out of sync: {} (expected version {})",
                    path.display(),
                    version
                );
                out_of_sync = true;
            }
        }
    }

    if out_of_sync {
        return Err(anyhow::anyhow!(
            "Versions are out of sync. Run 'resq version apply' or fix manually."
        ));
    }
    println!("✅ All manifests are synchronized at version {version}");

    Ok(())
}

fn get_repo_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?;
    let path_str = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(PathBuf::from(path_str))
}

fn get_current_version(root: &Path) -> Result<String> {
    // Try Cargo.toml first in this workspace
    let cargo_path = root.join("Cargo.toml");
    if cargo_path.exists() {
        let content = fs::read_to_string(&cargo_path)?;
        for line in content.lines() {
            if line.trim().starts_with("version = \"") {
                let v = line
                    .split('"')
                    .nth(1)
                    .context("Invalid version line in Cargo.toml")?;
                return Ok(v.to_string());
            }
        }
    }

    // Fallback to package.json
    let pkg_path = root.join("package.json");
    if pkg_path.exists() {
        let pkg_json = fs::read_to_string(pkg_path)?;
        let v: serde_json::Value = serde_json::from_str(&pkg_json)?;
        if let Some(version) = v["version"].as_str() {
            return Ok(version.to_string());
        }
    }

    Err(anyhow::anyhow!(
        "Could not find version in Cargo.toml or package.json"
    ))
}

fn bump_version(current: &str, bump: &str) -> Result<String> {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() < 3 {
        return Err(anyhow::anyhow!("Invalid version format: {current}"));
    }

    let mut major: u32 = parts[0].parse()?;
    let mut minor: u32 = parts[1].parse()?;
    let mut patch: u32 = parts[2].parse()?;

    match bump {
        "major" => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        "minor" => {
            minor += 1;
            patch = 0;
        }
        _ => {
            patch += 1;
        }
    }

    Ok(format!("{major}.{minor}.{patch}"))
}

fn update_manifests(root: &Path, old_version: &str, new_version: &str) -> Result<()> {
    // 1. package.json
    let pkg_path = root.join("package.json");
    if pkg_path.exists() {
        let pkg_content = fs::read_to_string(&pkg_path)?;
        let new_pkg = pkg_content.replace(
            &format!("\"version\": \"{old_version}\""),
            &format!("\"version\": \"{new_version}\""),
        );
        fs::write(pkg_path, new_pkg)?;
    }

    // 2. pyproject.toml
    let py_path = root.join("pyproject.toml");
    if py_path.exists() {
        let py_content = fs::read_to_string(&py_path)?;
        let new_py = py_content.replace(
            &format!("version = \"{old_version}\""),
            &format!("version = \"{new_version}\""),
        );
        fs::write(py_path, new_py)?;
    }

    // 3. Cargo.toml (Workspace)
    let cargo_path = root.join("Cargo.toml");
    if cargo_path.exists() {
        let cargo_content = fs::read_to_string(&cargo_path)?;
        let new_cargo = cargo_content.replace(
            &format!("version = \"{old_version}\""),
            &format!("version = \"{new_version}\""),
        );
        fs::write(cargo_path, new_cargo)?;
    }

    // 4. Directory.Build.props
    let props_path = root.join("Directory.Build.props");
    if props_path.exists() {
        let props_content = fs::read_to_string(&props_path)?;
        let new_props = props_content.replace(
            &format!("<Version>{old_version}</Version>"),
            &format!("<Version>{new_version}</Version>"),
        );
        fs::write(props_path, new_props)?;
    }

    Ok(())
}

fn update_changelog(root: &Path, version: &str, messages: &[String]) -> Result<()> {
    let path = root.join("CHANGELOG.md");
    let date = Utc::now().format("%Y-%m-%d");
    let mut new_entry = format!("\n## [{version}] - {date}\n\n");
    for msg in messages {
        new_entry.push_str(&format!("- {msg}\n"));
    }

    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let updated = format!(
            "# Changelog\n{}{}",
            new_entry,
            content.replace("# Changelog", "")
        );
        fs::write(path, updated)?;
    } else {
        fs::write(path, format!("# Changelog\n{new_entry}"))?;
    }
    Ok(())
}
