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

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use resq_cli::commands;
use tracing_subscriber::EnvFilter;

const LONG_ABOUT: &str = "\
ResQ developer CLI — audits, formatting, secrets scanning, git hooks, and six TUI explorers.

Common commands:
  resq audit           Run cargo/bun/uv audit across the workspace
  resq secrets         Scan the repo for leaked secrets and credentials
  resq format          Format Rust / TS / Python / C++ / C# in one pass
  resq pre-commit      Run the full pre-commit gate (copyright, secrets, audit, format)
  resq hooks           Inspect and maintain installed git hooks

Run `resq <command> --help` for per-command options, or `resq completions <shell>`
to install shell tab-completion.";

/// Command-line arguments for the `ResQ` CLI.
#[derive(Parser)]
#[command(name = "resq")]
#[command(version, about = "ResQ CLI tools", long_about = LONG_ABOUT)]
pub struct Cli {
    /// Increase log verbosity (`-v` info, `-vv` debug, `-vvv` trace). Overridden by `--quiet`.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress non-error output. Wins over `--verbose`.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable colored output in this process. Set `NO_COLOR=1` to propagate to
    /// child tools (cargo, bun, uv, …).
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage copyright headers
    Copyright(commands::copyright::CopyrightArgs),
    /// Run audit in workspaces
    Audit(commands::audit::AuditArgs),
    /// Scan for secrets and credentials
    Secrets(commands::secrets::SecretsArgs),
    /// Format source files (Rust / TS / Python / C++ / C#)
    Format(commands::format::FormatArgs),
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
    /// Generate AI-powered commit messages from staged changes
    Commit(commands::commit::CommitArgs),
    /// Emit shell completions to stdout (bash, zsh, fish, elvish, powershell)
    Completions {
        /// Target shell for completion script.
        shell: Shell,
    },
}

/// Initialize tracing based on `-v` / `-q` / `--no-color` flags.
///
/// Default is `warn`; `-v` bumps to `info`, `-vv` to `debug`, `-vvv`+ to `trace`.
/// `--quiet` forces `error` regardless of verbose count. If `RUST_LOG` is set
/// in the environment it takes precedence over the computed level.
fn init_tracing(verbose: u8, quiet: bool, no_color: bool) {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    let builder = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_target(false);

    if no_color {
        builder.with_ansi(false).init();
    } else {
        builder.init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    init_tracing(cli.verbose, cli.quiet, cli.no_color);

    match cli.command {
        Commands::Copyright(args) => commands::copyright::run(&args)?,
        Commands::Audit(args) => commands::audit::run(args).await?,
        Commands::Secrets(args) => commands::secrets::run(args).await?,
        Commands::Format(args) => commands::format::run(args).await?,
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
        Commands::Commit(args) => commands::commit::run(args).await?,
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin, &mut std::io::stdout());
        }
    }

    Ok(())
}
