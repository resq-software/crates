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

//! `ResQ` CLI - Main entry point.
//!
//! Provides a unified command-line interface for `ResQ` service management.

#![deny(missing_docs)]

use clap::{Parser, Subcommand};
use resq_cli::commands;

/// Command-line arguments for the `ResQ` CLI.
#[derive(Parser)]
#[command(name = "resq")]
#[command(version, about = "ResQ CLI tools", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage copyright headers
    Copyright(commands::copyright::CopyrightArgs),
    /// Generate LQIP for images
    Lqip(commands::lqip::LqipArgs),
    /// Run audit in workspaces
    Audit(commands::audit::AuditArgs),
    /// Analyze package cost
    Cost(commands::cost::CostArgs),
    /// Run tree-shake (tsr)
    TreeShake(commands::tree_shake::TreeShakeArgs),
    /// Scan for secrets and credentials
    Secrets(commands::secrets::SecretsArgs),
    /// Repository and development utilities
    Dev(commands::dev::DevArgs),
    /// Launch Perf-Explorer (TUI)
    Explore(commands::explore::ExploreArgs),
    /// Launch Log-Explorer (TUI)
    Logs(commands::explore::LogsArgs),
    /// Launch Health-Explorer (TUI)
    Health(commands::explore::HealthArgs),
    /// Launch Deploy-Explorer (TUI)
    Deploy(commands::explore::DeployArgs),
    /// Launch Cleanup-Explorer (TUI)
    Clean(commands::explore::CleanArgs),
    /// Launch Asm-Explorer for machine code analysis
    Asm(commands::explore::AsmArgs),
    /// Run pre-commit checks (copyright, secrets, audit, formatting)
    PreCommit(commands::pre_commit::PreCommitArgs),
    /// Manage versions and changesets across the monorepo
    Version(commands::version::VersionArgs),
    /// Manage documentation export and publication
    Docs(commands::docs::DocsArgs),
    /// Inspect and maintain installed git hooks (doctor, update, status)
    Hooks(commands::hooks::HooksArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Copyright(args) => commands::copyright::run(&args)?,
        Commands::Lqip(args) => commands::lqip::run(args).await?,
        Commands::Audit(args) => commands::audit::run(args).await?,
        Commands::Cost(args) => commands::cost::run(args).await?,
        Commands::TreeShake(args) => commands::tree_shake::run(args).await?,
        Commands::Secrets(args) => commands::secrets::run(args).await?,
        Commands::Dev(args) => commands::dev::run(args)?,
        Commands::Explore(args) => commands::explore::run_explore(args).await?,
        Commands::Logs(args) => commands::explore::run_logs(args).await?,
        Commands::Health(args) => commands::explore::run_health(args).await?,
        Commands::Deploy(args) => commands::explore::run_deploy(args).await?,
        Commands::Clean(args) => commands::explore::run_clean(args).await?,
        Commands::Asm(args) => commands::explore::run_asm(args).await?,
        Commands::PreCommit(args) => commands::pre_commit::run(args).await?,
        Commands::Version(args) => commands::version::run(args)?,
        Commands::Docs(args) => commands::docs::run(args)?,
        Commands::Hooks(args) => commands::hooks::run(args)?,
    }

    Ok(())
}
