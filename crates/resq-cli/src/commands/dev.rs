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

//! ResQ Dev commands — Repository and development utilities.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

/// Arguments for the 'dev' command.
#[derive(Parser, Debug)]
pub struct DevArgs {
    /// Dev subcommand to execute
    #[command(subcommand)]
    pub command: DevCommands,
}

/// Developer subcommands.
#[derive(Subcommand, Debug)]
pub enum DevCommands {
    /// Kill processes listening on specified ports
    KillPorts(KillPortsArgs),
    /// Sync environment variables from .env.example files to turbo.json
    SyncEnv(SyncEnvArgs),
    /// Upgrade dependencies across all monorepo silos
    Upgrade(UpgradeArgs),
    /// Install git hooks from .git-hooks directory
    InstallHooks,
}

/// Arguments for the 'kill-ports' command.
#[derive(Parser, Debug)]
pub struct KillPortsArgs {
    /// Ports or ranges (e.g. 8000 or 8000..8010)
    #[arg(required = true)]
    pub targets: Vec<String>,

    /// Use SIGKILL instead of default SIGTERM
    #[arg(short, long)]
    pub force: bool,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

/// Arguments for the 'sync-env' command.
#[derive(Parser, Debug)]
pub struct SyncEnvArgs {
    /// Tasks to update in turbo.json (comma-separated)
    #[arg(short, long, default_value = "build,dev,start,test")]
    pub tasks: String,

    /// Preview changes without writing to turbo.json
    #[arg(short, long)]
    pub dry_run: bool,

    /// Maximum directory depth to search
    #[arg(long, default_value_t = 10)]
    pub max_depth: usize,
}

/// Arguments for the 'upgrade' command.
#[derive(Parser, Debug)]
pub struct UpgradeArgs {
    /// Specific silo to upgrade (python, rust, js, cpp, csharp, nix, or all)
    #[arg(default_value = "all")]
    pub silo: String,
}

/// Executes the dev command.
pub fn run(args: DevArgs) -> Result<()> {
    match args.command {
        DevCommands::KillPorts(args) => run_kill_ports(args),
        DevCommands::SyncEnv(args) => run_sync_env(args),
        DevCommands::Upgrade(args) => run_upgrade(args),
        DevCommands::InstallHooks => run_install_hooks(),
    }
}

/// Canonical hook templates. Kept in sync with
/// https://github.com/resq-software/dev/tree/main/scripts/git-hooks — both
/// sources ship the same content until `dev` retires its copy.
const HOOK_TEMPLATES: &[(&str, &str)] = &[
    ("pre-commit",         include_str!("../../templates/git-hooks/pre-commit")),
    ("commit-msg",         include_str!("../../templates/git-hooks/commit-msg")),
    ("prepare-commit-msg", include_str!("../../templates/git-hooks/prepare-commit-msg")),
    ("pre-push",           include_str!("../../templates/git-hooks/pre-push")),
    ("post-checkout",      include_str!("../../templates/git-hooks/post-checkout")),
    ("post-merge",         include_str!("../../templates/git-hooks/post-merge")),
];

fn run_install_hooks() -> Result<()> {
    let root = crate::utils::find_project_root();
    let hooks_dir = root.join(".git-hooks");

    // Scaffold from embedded templates if the directory is missing or empty.
    // This makes `resq dev install-hooks` usable from a fresh clone without
    // pre-shipped hook files. Existing hook content is never overwritten.
    let should_scaffold = !hooks_dir.exists()
        || std::fs::read_dir(&hooks_dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(true);

    if should_scaffold {
        println!("📝 Scaffolding canonical ResQ git hooks from embedded templates...");
        std::fs::create_dir_all(&hooks_dir)
            .with_context(|| format!("Failed to create {}", hooks_dir.display()))?;
        for (name, body) in HOOK_TEMPLATES {
            let dest = hooks_dir.join(name);
            if dest.exists() {
                continue;
            }
            std::fs::write(&dest, body)
                .with_context(|| format!("Failed to write {}", dest.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&dest)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&dest, perms)?;
            }
        }
    }

    println!("🔧 Setting up ResQ git hooks...");

    // Configure git to use custom hooks directory
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".git-hooks"])
        .current_dir(&root)
        .status()
        .context("Failed to run git config")?;

    if !status.success() {
        anyhow::bail!("Failed to set git core.hooksPath");
    }

    // Make hooks executable
    let mut count = 0;
    for entry in std::fs::read_dir(&hooks_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "README.md" {
                continue;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&path, perms)?;
            }

            println!("  • {name}");
            count += 1;
        }
    }

    println!("\n✅ Successfully installed {count} git hooks!");
    Ok(())
}

fn run_upgrade(args: UpgradeArgs) -> Result<()> {
    let silo = args.silo.to_lowercase();
    let root = crate::utils::find_project_root();

    println!("🚀 Starting ResQ Polyglot Upgrade (Silo: {silo})...");

    match silo.as_str() {
        "python" => upgrade_python(&root)?,
        "rust" => upgrade_rust(&root)?,
        "js" | "javascript" | "ts" | "typescript" => upgrade_js(&root)?,
        "cpp" | "c++" => upgrade_cpp(&root)?,
        "csharp" | "c#" => upgrade_csharp(&root)?,
        "nix" => upgrade_nix(&root)?,
        "all" => {
            let _ = upgrade_nix(&root);
            let _ = upgrade_python(&root);
            let _ = upgrade_rust(&root);
            let _ = upgrade_js(&root);
            let _ = upgrade_cpp(&root);
            let _ = upgrade_csharp(&root);
        }
        _ => anyhow::bail!("Unknown silo: {silo}. Valid: python, rust, js, cpp, csharp, nix, all"),
    }

    println!("\n✅ Upgrade complete!");
    Ok(())
}

fn upgrade_python(root: &Path) -> Result<()> {
    println!("\n[Python/uv] Upgrading dependencies...");
    let _ = Command::new("uv")
        .args(["lock", "--upgrade"])
        .current_dir(root)
        .status();
    let _ = Command::new("uv").args(["sync"]).current_dir(root).status();
    Ok(())
}

fn upgrade_rust(root: &Path) -> Result<()> {
    println!("\n[Rust/cargo] Upgrading dependencies...");
    let has_upgrade = Command::new("cargo")
        .arg("upgrade")
        .arg("--version")
        .output()
        .is_ok();
    if has_upgrade {
        let _ = Command::new("cargo")
            .args(["upgrade", "--workspace"])
            .current_dir(root)
            .status();
    }
    let _ = Command::new("cargo")
        .arg("update")
        .current_dir(root)
        .status();
    Ok(())
}

fn upgrade_js(root: &Path) -> Result<()> {
    println!("\n[JS/TS/bun] Upgrading dependencies...");
    let _ = Command::new("bun")
        .args([
            "x",
            "npm-check-updates",
            "-u",
            "--packageManager",
            "bun",
            "--workspaces",
            "--root",
        ])
        .current_dir(root)
        .status();
    let _ = Command::new("bun")
        .arg("install")
        .current_dir(root)
        .status();
    Ok(())
}

fn upgrade_cpp(root: &Path) -> Result<()> {
    println!("\n[C++] Upgrading dependencies...");
    for entry in walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy();
        if name == "conanfile.txt" || name == "conanfile.py" {
            let dir = entry
                .path()
                .parent()
                .expect("Conan file should have a parent directory");
            println!("   Found Conan config in {}. Upgrading...", dir.display());
            let _ = Command::new("conan")
                .args(["install", ".", "--update", "--build=missing"])
                .current_dir(dir)
                .status();
        }
    }
    Ok(())
}

fn upgrade_csharp(root: &Path) -> Result<()> {
    println!("\n[C#] Upgrading dependencies...");
    let _ = Command::new("dotnet")
        .args(["outdated", "--upgrade"])
        .current_dir(root)
        .status();
    let _ = Command::new("dotnet")
        .arg("restore")
        .current_dir(root)
        .status();
    Ok(())
}

fn upgrade_nix(root: &Path) -> Result<()> {
    if root.join("flake.nix").exists() {
        println!("\n[Nix] Updating flake lockfile...");
        let _ = Command::new("nix")
            .args(["flake", "update"])
            .current_dir(root)
            .status();
    }
    Ok(())
}

fn run_sync_env(args: SyncEnvArgs) -> Result<()> {
    let root = crate::utils::find_project_root();
    let turbo_path = root.join("turbo.json");

    if !turbo_path.exists() {
        anyhow::bail!(
            "turbo.json not found in project root: {}",
            turbo_path.display()
        );
    }

    println!("🔍 Scanning for environment files in {}...", root.display());

    let tasks: Vec<String> = args
        .tasks
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let mut env_vars = std::collections::HashSet::new();

    let mut stack = vec![(root.clone(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > args.max_depth {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if path.is_dir() {
                if name_str == "node_modules" || name_str == ".git" || name_str == "target" {
                    continue;
                }
                stack.push((path, depth + 1));
            } else if path.is_file()
                && (name_str == ".env.example" || name_str.ends_with(".env.example"))
            {
                println!(
                    "   📄 Reading {}",
                    path.strip_prefix(&root).unwrap_or(&path).display()
                );
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }
                        let Some(equal_idx) = trimmed.find('=') else {
                            continue;
                        };
                        let var_name = trimmed[..equal_idx].trim();
                        if !var_name.is_empty() {
                            env_vars.insert(var_name.to_string());
                        }
                    }
                }
            }
        }
    }

    if env_vars.is_empty() {
        println!("⚠️  No environment variables found in .env.example files.");
        return Ok(());
    }

    let mut sorted_vars: Vec<_> = env_vars.into_iter().collect();
    sorted_vars.sort();

    println!(
        "🔧 Found {} unique environment variables.",
        sorted_vars.len()
    );

    let turbo_content = std::fs::read_to_string(&turbo_path)?;
    let mut turbo_json: serde_json::Value = serde_json::from_str(&turbo_content)?;

    if let Some(tasks_obj) = turbo_json.get_mut("tasks").and_then(|t| t.as_object_mut()) {
        for task in tasks {
            if let Some(task_config) = tasks_obj.get_mut(&task).and_then(|t| t.as_object_mut()) {
                println!("   ✅ Updating task: {task}");
                task_config.insert("env".to_string(), serde_json::to_value(&sorted_vars)?);
            }
        }
    }

    if args.dry_run {
        println!("\n🏃 DRY RUN - Preview of updated turbo.json tasks:");
        if let Some(tasks_obj) = turbo_json.get_mut("tasks") {
            println!("{}", serde_json::to_string_pretty(tasks_obj)?);
        }
    } else {
        let updated_content = serde_json::to_string_pretty(&turbo_json)? + "\n";
        std::fs::write(&turbo_path, updated_content)?;
        println!("\n✅ Successfully updated turbo.json!");
    }

    Ok(())
}

fn run_kill_ports(args: KillPortsArgs) -> Result<()> {
    let mut ports = Vec::new();
    for target in args.targets {
        let target_str: &str = &target;
        if target_str.contains("..") {
            let parts: Vec<&str> = target_str.split("..").collect();
            if parts.len() == 2 {
                let start: u16 = parts[0].parse().context("Invalid start port")?;
                let end: u16 = parts[1].parse().context("Invalid end port")?;
                for p in start..=end {
                    ports.push(p);
                }
            }
        } else {
            let p: u16 = target_str.parse().context("Invalid port")?;
            ports.push(p);
        }
    }

    if ports.is_empty() {
        println!("No ports specified.");
        return Ok(());
    }

    println!("🔍 Searching for processes on ports: {ports:?}...");

    let ports_str = ports
        .iter()
        .map(|p: &u16| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let output = Command::new("lsof")
        .args([
            "-i",
            &format!("TCP:{ports_str}"),
            "-sTCP:LISTEN",
            "-P",
            "-n",
            "-t",
        ])
        .output()
        .context("Failed to run lsof. Is it installed?")?;

    let pids_raw = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<&str> = pids_raw.lines().filter(|l| !l.trim().is_empty()).collect();

    if pids.is_empty() {
        println!("✅ No processes found listening on these ports.");
        return Ok(());
    }

    println!("⚠️  Found {} process(es):", pids.len());
    for pid in &pids {
        let info = Command::new("ps")
            .args(["-p", pid, "-o", "comm="])
            .output()
            .ok();
        let comm = info.map_or_else(
            || "unknown".into(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        );
        println!("   - PID {pid} ({comm})");
    }

    if !args.yes && !args.force {
        print!("\nTerminate these processes? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let signal = if args.force { "-9" } else { "-15" };
    let mut success = 0;
    let mut failed = 0;

    for pid in pids {
        let status = Command::new("kill").args([signal, pid]).status();

        if status.is_ok_and(|s| s.success()) {
            success += 1;
        } else {
            failed += 1;
        }
    }

    println!("\nSummary:");
    println!("   ✅ Successfully signaled {success} process(es).");
    if failed > 0 {
        println!("   ❌ Failed to signal {failed} process(es). (Try with sudo?)");
    }

    Ok(())
}
