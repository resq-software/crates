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

//! `resq hooks` — visibility and maintenance for installed git hooks.
//!
//! - `resq hooks doctor` reports drift between installed `.git-hooks/<file>`
//!   and the canonical content embedded in this binary.
//! - `resq hooks update` rewrites the canonical hooks (preserving any
//!   `local-*` files the repo committed).
//! - `resq hooks status` prints a one-line summary suitable for shells.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::commands::hook_templates::HOOK_TEMPLATES;

/// Arguments for the `hooks` command.
#[derive(Parser, Debug)]
pub struct HooksArgs {
    /// Hooks subcommand to execute.
    #[command(subcommand)]
    pub command: HooksCommands,
}

/// Hooks subcommands.
#[derive(Subcommand, Debug)]
pub enum HooksCommands {
    /// Report installed hook status; exit 1 if any drift / missing file detected.
    Doctor,
    /// Rewrite installed canonical hooks from embedded templates (preserves `local-*`).
    Update,
    /// Print a one-line summary for scripts (e.g. `installed=clean local=pre-push`).
    Status,
}

/// Executes a `hooks` subcommand.
///
/// # Errors
/// Returns an error if filesystem access or `git config` invocation fails.
pub fn run(args: HooksArgs) -> Result<()> {
    match args.command {
        HooksCommands::Doctor => run_doctor(),
        HooksCommands::Update => run_update(),
        HooksCommands::Status => run_status(),
    }
}

/// Result of inspecting the installed hooks layout.
struct HookAudit {
    hooks_dir: PathBuf,
    hooks_path_set: bool,
    /// (name, status). status is `Match` / `Drift` / `Missing`.
    canonical: Vec<(String, HookStatus)>,
    local: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum HookStatus {
    Match,
    Drift,
    Missing,
}

fn audit() -> Result<HookAudit> {
    let root = crate::utils::find_project_root();
    let hooks_dir = root.join(".git-hooks");

    let hooks_path_set = read_hooks_path(&root)
        .map(|p| p.trim() == ".git-hooks")
        .unwrap_or(false);

    let mut canonical = Vec::with_capacity(HOOK_TEMPLATES.len());
    for (name, body) in HOOK_TEMPLATES {
        let installed = hooks_dir.join(name);
        let status = if !installed.exists() {
            HookStatus::Missing
        } else {
            match std::fs::read_to_string(&installed) {
                Ok(content) if content == *body => HookStatus::Match,
                _ => HookStatus::Drift,
            }
        };
        canonical.push(((*name).to_string(), status));
    }

    let mut local = Vec::new();
    if hooks_dir.exists() {
        for entry in std::fs::read_dir(&hooks_dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stripped) = name.strip_prefix("local-") {
                local.push(stripped.to_string());
            }
        }
        local.sort();
    }

    Ok(HookAudit {
        hooks_dir,
        hooks_path_set,
        canonical,
        local,
    })
}

fn read_hooks_path(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run_doctor() -> Result<()> {
    let audit = audit()?;
    let mut issues = 0u32;

    println!("🔎 ResQ hooks doctor");
    println!("   .git-hooks/         {}", audit.hooks_dir.display());

    if audit.hooks_path_set {
        println!("   core.hooksPath      ✅ set to .git-hooks");
    } else {
        println!("   core.hooksPath      ❌ not set");
        println!("     fix:  git config core.hooksPath .git-hooks");
        issues += 1;
    }

    println!("\n   Canonical hooks:");
    for (name, status) in &audit.canonical {
        match status {
            HookStatus::Match => println!("     ✅ {name}"),
            HookStatus::Drift => {
                println!("     ❌ {name}  (drifts from embedded canonical)");
                issues += 1;
            }
            HookStatus::Missing => {
                println!("     ❌ {name}  (missing)");
                issues += 1;
            }
        }
    }
    if audit.canonical.iter().any(|(_, s)| *s != HookStatus::Match) {
        println!("     fix:  resq hooks update");
    }

    println!("\n   Local hooks (.git-hooks/local-*):");
    if audit.local.is_empty() {
        println!("     (none)");
    } else {
        for name in &audit.local {
            println!("     • local-{name}");
        }
    }

    if issues == 0 {
        println!("\n✅ All hooks healthy.");
        Ok(())
    } else {
        println!("\n❌ {issues} issue(s) detected.");
        std::process::exit(1);
    }
}

fn run_update() -> Result<()> {
    let root = crate::utils::find_project_root();
    let hooks_dir = root.join(".git-hooks");
    std::fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("Failed to create {}", hooks_dir.display()))?;

    let mut updated = 0u32;
    for (name, body) in HOOK_TEMPLATES {
        let dest = hooks_dir.join(name);
        let needs_write = match std::fs::read_to_string(&dest) {
            Ok(existing) => existing != *body,
            Err(_) => true,
        };
        if needs_write {
            std::fs::write(&dest, body)
                .with_context(|| format!("Failed to write {}", dest.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms)?;
            }
            updated += 1;
            println!("  ↻ {name}");
        }
    }

    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".git-hooks"])
        .current_dir(&root)
        .status()
        .context("Failed to run git config")?;
    if !status.success() {
        anyhow::bail!("Failed to set core.hooksPath");
    }

    if updated == 0 {
        println!("✅ Hooks already canonical; nothing to do.");
    } else {
        println!("✅ {updated} hook(s) updated. Local-* files were not touched.");
    }
    Ok(())
}

fn run_status() -> Result<()> {
    let audit = audit()?;
    let canonical_state =
        if audit.canonical.iter().all(|(_, s)| *s == HookStatus::Match) && audit.hooks_path_set {
            "clean"
        } else {
            "drift"
        };
    let local = if audit.local.is_empty() {
        "none".to_string()
    } else {
        audit.local.join(",")
    };
    println!("installed={canonical_state} local={local}");
    Ok(())
}

/// Returns the canonical hook count — used by docs/tests.
#[must_use]
pub fn canonical_count() -> usize {
    HOOK_TEMPLATES.len()
}
