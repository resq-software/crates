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
use resq_cli::commands;
use std::io::IsTerminal;
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
    /// Increase structured-log verbosity (`--verbose` info, `-vv` debug, `-vvv` trace).
    /// Affects `tracing` output only — subcommand `-v` flags are unchanged. Overridden by
    /// `--quiet`. `RUST_LOG`, if set, wins over both.
    #[arg(long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress non-error `tracing` output (sets the log level to `error`).
    /// Does not silence subcommands that write to stdout/stderr directly.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable ANSI colors in `tracing` output. `clap` help and subcommands that write
    /// directly to stdout/stderr are unaffected by this flag; export `NO_COLOR=1`
    /// before invocation for fully-uncolored output (also propagates to child tools).
    /// Color auto-detection is on by default: when stderr is not a TTY, ANSI is
    /// suppressed even without this flag.
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
    Completions(commands::completions::CompletionsArgs),
}

/// Initialize tracing based on `--verbose` / `--quiet` / `--no-color` flags.
///
/// Writes to **stderr** so that structured logs never pollute subcommand stdout
/// (e.g. `resq completions bash > file` must produce a clean file). Default level
/// is `warn`; `--verbose` bumps to `info`, `-vv` to `debug`, `-vvv`+ to `trace`;
/// `--quiet` forces `error`. If `RUST_LOG` is set in the environment it takes
/// precedence over the computed level.
///
/// ANSI colors auto-detect from stderr's TTY status; `--no-color` forces them
/// off explicitly.
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
    let use_ansi = !no_color && std::io::stderr().is_terminal();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_ansi(use_ansi)
        .with_target(false)
        .init();
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
        Commands::Completions(args) => commands::completions::run(args, Cli::command())?,
    }

    Ok(())
}
