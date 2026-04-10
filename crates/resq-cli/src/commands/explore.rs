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

//! ResQ Explore commands — Unified TUI launcher.

use anyhow::{Context, Result};
use clap::Parser;
use std::process::Command;

/// Arguments for the 'explore' (resq-perf) command
#[derive(Parser, Debug)]
pub struct ExploreArgs {
    /// Service URL to monitor
    #[arg(default_value = "http://localhost:3000/admin/status")]
    pub url: String,
    /// Refresh rate in milliseconds
    #[arg(long, default_value_t = 500)]
    pub refresh_ms: u64,
}

/// Arguments for the 'logs' (resq-logs) command
#[derive(Parser, Debug)]
pub struct LogsArgs {
    /// Log source: "docker" or "file"
    #[arg(long, default_value = "docker")]
    pub source: String,
    /// Filter to a specific service name
    #[arg(long)]
    pub service: Option<String>,
}

/// Arguments for the 'health' (resq-health) command
#[derive(Parser, Debug)]
pub struct HealthArgs {
    /// Poll interval in seconds
    #[arg(short, long, default_value_t = 5)]
    pub interval: u64,
}

/// Arguments for the 'deploy' (resq-deploy) command
#[derive(Parser, Debug)]
pub struct DeployArgs {
    /// Target environment: dev, staging, prod
    #[arg(long, default_value = "dev")]
    pub env: String,
    /// Use Kubernetes instead of Docker Compose
    #[arg(long)]
    pub k8s: bool,
}

/// Arguments for the 'clean' (resq-clean) command
#[derive(Parser, Debug)]
pub struct CleanArgs {
    /// Preview what would be deleted without removing anything
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

/// Arguments for the 'asm' (resq-bin) command
#[derive(Parser, Debug)]
pub struct AsmArgs {
    /// Analyze a single binary path
    #[arg(long, conflicts_with = "dir")]
    pub file: Option<String>,
    /// Analyze binaries under a directory
    #[arg(long, conflicts_with = "file")]
    pub dir: Option<String>,
    /// Recursively traverse directory in batch mode
    #[arg(long, default_value_t = false)]
    pub recursive: bool,
    /// Optional suffix filter in batch mode (e.g. .so, .o)
    #[arg(long)]
    pub ext: Option<String>,
    /// Optional resq-bin config TOML path
    #[arg(long)]
    pub config: Option<String>,
    /// Disable cache reads/writes
    #[arg(long, default_value_t = false)]
    pub no_cache: bool,
    /// Force cache rebuild
    #[arg(long, default_value_t = false)]
    pub rebuild_cache: bool,
    /// Disable disassembly and only collect metadata
    #[arg(long, default_value_t = false)]
    pub no_disasm: bool,
    /// Maximum functions to disassemble per binary
    #[arg(long)]
    pub max_functions: Option<usize>,
    /// Force interactive TUI mode
    #[arg(long, default_value_t = false)]
    pub tui: bool,
    /// Use non-interactive plain output
    #[arg(long, default_value_t = false)]
    pub plain: bool,
    /// Emit JSON report output
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

/// Run resq-perf (Performance Explorer)
pub async fn run_explore(args: ExploreArgs) -> Result<()> {
    run_tool(
        "resq-perf",
        &[&args.url, "--refresh-ms", &args.refresh_ms.to_string()],
    )
}

/// Run resq-logs (Log Explorer)
pub async fn run_logs(args: LogsArgs) -> Result<()> {
    let mut cmd_args = vec!["--source", &args.source];
    if let Some(ref s) = args.service {
        cmd_args.push("--service");
        cmd_args.push(s);
    }
    run_tool("resq-logs", &cmd_args)
}

/// Run resq-health (Health Explorer)
pub async fn run_health(args: HealthArgs) -> Result<()> {
    run_tool("resq-health", &["--interval", &args.interval.to_string()])
}

/// Run resq-deploy (Deploy Explorer)
pub async fn run_deploy(args: DeployArgs) -> Result<()> {
    let mut cmd_args = vec!["--env", &args.env];
    if args.k8s {
        cmd_args.push("--k8s");
    }
    run_tool("resq-deploy", &cmd_args)
}

/// Run resq-clean (Workspace Cleaner)
pub async fn run_clean(args: CleanArgs) -> Result<()> {
    let mut cmd_args = Vec::new();
    if args.dry_run {
        cmd_args.push("--dry-run");
    }
    run_tool("resq-clean", &cmd_args)
}

/// Run resq-bin (Binary Explorer)
pub async fn run_asm(args: AsmArgs) -> Result<()> {
    let mut cmd_args = Vec::new();
    if let Some(ref file) = args.file {
        cmd_args.push("--file");
        cmd_args.push(file);
    }
    if let Some(ref dir) = args.dir {
        cmd_args.push("--dir");
        cmd_args.push(dir);
    }
    if args.recursive {
        cmd_args.push("--recursive");
    }
    if let Some(ref ext) = args.ext {
        cmd_args.push("--ext");
        cmd_args.push(ext);
    }
    if let Some(ref config) = args.config {
        cmd_args.push("--config");
        cmd_args.push(config);
    }
    if args.no_cache {
        cmd_args.push("--no-cache");
    }
    if args.rebuild_cache {
        cmd_args.push("--rebuild-cache");
    }
    if args.no_disasm {
        cmd_args.push("--no-disasm");
    }
    let max_functions = args.max_functions.map(|v| v.to_string());
    if let Some(ref max_functions) = max_functions {
        cmd_args.push("--max-functions");
        cmd_args.push(max_functions);
    }
    if args.tui {
        cmd_args.push("--tui");
    }
    if args.plain {
        cmd_args.push("--plain");
    }
    if args.json {
        cmd_args.push("--json");
    }
    run_tool("resq-bin", &cmd_args)
}

fn run_tool(name: &str, args: &[&str]) -> Result<()> {
    // We assume the tool is built and available via 'cargo run -p'
    // or eventually as a pre-built binary in the path.
    // For now, using 'cargo run -p' ensures we always run the latest version in dev.

    let mut child = Command::new("cargo")
        .arg("run")
        .arg("-q")
        .arg("-p")
        .arg(name)
        .arg("--")
        .args(args)
        .spawn()
        .with_context(|| format!("Failed to launch tool: {name}"))?;

    let status = child.wait().context("Tool crashed or was interrupted")?;
    if !status.success() {
        // We don't necessarily want to exit the main CLI with error if the TUI was just closed
    }
    Ok(())
}
