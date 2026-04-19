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

use clap::{Args, CommandFactory, Parser, Subcommand};
use resq_cli::commands;
use std::io::IsTerminal;
use tracing_subscriber::EnvFilter;

const LONG_ABOUT: &str = "\
ResQ developer CLI — audits, formatting, git hooks, and six TUI explorers.

Common commands:
  resq scan audit      Run cargo/bun/uv audit across the workspace
  resq scan secrets    Scan the repo for leaked secrets and credentials
  resq format          Format Rust / TS / Python / C++ / C# in one pass
  resq pre-commit      Run the full pre-commit gate (copyright, secrets, audit, format)
  resq hooks           Inspect and maintain installed git hooks
  resq tui <screen>    Launch a TUI explorer (explore, logs, health, deploy, clean, asm)

The old flat forms (`resq audit`, `resq explore`, …) still work as hidden aliases
for one release cycle. Run `resq <command> --help` for per-command options, or
`resq completions <shell>` to install shell tab-completion.";

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
    // ── Grouped commands (new, visible) ──────────────────────────────────────
    /// Scan the workspace (audit | secrets | copyright)
    Scan(ScanArgs),
    /// Launch a TUI explorer (explore | logs | health | deploy | clean | asm)
    Tui(TuiArgs),

    // ── Kept-flat commands ───────────────────────────────────────────────────
    /// Format source files (Rust / TS / Python / C++ / C#)
    Format(commands::format::FormatArgs),
    /// Repository and development utilities
    Dev(commands::dev::DevArgs),
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

    // ── Legacy flat aliases (hidden; still callable for backwards compat) ────
    /// Deprecated: use `resq scan copyright`
    #[command(hide = true)]
    Copyright(commands::copyright::CopyrightArgs),
    /// Deprecated: use `resq scan audit`
    #[command(hide = true)]
    Audit(commands::audit::AuditArgs),
    /// Deprecated: use `resq scan secrets`
    #[command(hide = true)]
    Secrets(commands::secrets::SecretsArgs),
    /// Deprecated: use `resq tui explore`
    #[command(hide = true)]
    Explore(commands::explore::ExploreArgs),
    /// Deprecated: use `resq tui logs`
    #[command(hide = true)]
    Logs(commands::explore::LogsArgs),
    /// Deprecated: use `resq tui health`
    #[command(hide = true)]
    Health(commands::explore::HealthArgs),
    /// Deprecated: use `resq tui deploy`
    #[command(hide = true)]
    Deploy(commands::explore::DeployArgs),
    /// Deprecated: use `resq tui clean`
    #[command(hide = true)]
    Clean(commands::explore::CleanArgs),
    /// Deprecated: use `resq tui asm`
    #[command(hide = true)]
    Asm(commands::explore::AsmArgs),
}

/// Args for the `resq scan` group.
#[derive(Args, Debug)]
struct ScanArgs {
    #[command(subcommand)]
    kind: ScanKind,
}

#[derive(Subcommand, Debug)]
enum ScanKind {
    /// Run audit in workspaces (cargo audit / bun audit / uv pip-audit)
    Audit(commands::audit::AuditArgs),
    /// Scan for secrets and credentials
    Secrets(commands::secrets::SecretsArgs),
    /// Manage copyright headers
    Copyright(commands::copyright::CopyrightArgs),
}

/// Args for the `resq tui` group.
#[derive(Args, Debug)]
struct TuiArgs {
    #[command(subcommand)]
    screen: TuiScreen,
}

#[derive(Subcommand, Debug)]
enum TuiScreen {
    /// Perf-Explorer — live performance metrics
    Explore(commands::explore::ExploreArgs),
    /// Log-Explorer — tail + filter service logs
    Logs(commands::explore::LogsArgs),
    /// Health-Explorer — cluster & service health
    Health(commands::explore::HealthArgs),
    /// Deploy-Explorer — deploy/rollback workflows
    Deploy(commands::explore::DeployArgs),
    /// Cleanup-Explorer — stale branches, artifacts, state
    Clean(commands::explore::CleanArgs),
    /// Asm-Explorer — machine-code analysis of build artifacts
    Asm(commands::explore::AsmArgs),
}

/// Emit a one-line deprecation notice to stderr when a legacy flat subcommand
/// is invoked. Non-fatal: the command still runs. Goes through `tracing::warn!`
/// so `--quiet` suppresses it and `NO_COLOR` / TTY-aware ANSI handling apply.
fn warn_deprecated(old: &str, new: &str) {
    tracing::warn!("`resq {old}` is deprecated; use `resq {new}` instead.");
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
        // ── Grouped commands ────────────────────────────────────────────────
        Commands::Scan(ScanArgs { kind }) => match kind {
            ScanKind::Audit(args) => commands::audit::run(args).await?,
            ScanKind::Secrets(args) => commands::secrets::run(args).await?,
            ScanKind::Copyright(args) => commands::copyright::run(&args)?,
        },
        Commands::Tui(TuiArgs { screen }) => match screen {
            TuiScreen::Explore(args) => commands::explore::run_explore(args).await?,
            TuiScreen::Logs(args) => commands::explore::run_logs(args).await?,
            TuiScreen::Health(args) => commands::explore::run_health(args).await?,
            TuiScreen::Deploy(args) => commands::explore::run_deploy(args).await?,
            TuiScreen::Clean(args) => commands::explore::run_clean(args).await?,
            TuiScreen::Asm(args) => commands::explore::run_asm(args).await?,
        },

        // ── Kept-flat commands ──────────────────────────────────────────────
        Commands::Format(args) => commands::format::run(args).await?,
        Commands::Dev(args) => commands::dev::run(args)?,
        Commands::PreCommit(args) => commands::pre_commit::run(args).await?,
        Commands::Version(args) => commands::version::run(args)?,
        Commands::Docs(args) => commands::docs::run(args)?,
        Commands::Hooks(args) => commands::hooks::run(args)?,
        Commands::Commit(args) => commands::commit::run(args).await?,
        Commands::Completions(args) => commands::completions::run(args, Cli::command())?,

        // ── Legacy flat aliases (hidden but still routed) ───────────────────
        // Each emits a deprecation warning through `tracing::warn!` before
        // dispatching, so users know to migrate to the grouped form.
        Commands::Copyright(args) => {
            warn_deprecated("copyright", "scan copyright");
            commands::copyright::run(&args)?;
        }
        Commands::Audit(args) => {
            warn_deprecated("audit", "scan audit");
            commands::audit::run(args).await?;
        }
        Commands::Secrets(args) => {
            warn_deprecated("secrets", "scan secrets");
            commands::secrets::run(args).await?;
        }
        Commands::Explore(args) => {
            warn_deprecated("explore", "tui explore");
            commands::explore::run_explore(args).await?;
        }
        Commands::Logs(args) => {
            warn_deprecated("logs", "tui logs");
            commands::explore::run_logs(args).await?;
        }
        Commands::Health(args) => {
            warn_deprecated("health", "tui health");
            commands::explore::run_health(args).await?;
        }
        Commands::Deploy(args) => {
            warn_deprecated("deploy", "tui deploy");
            commands::explore::run_deploy(args).await?;
        }
        Commands::Clean(args) => {
            warn_deprecated("clean", "tui clean");
            commands::explore::run_clean(args).await?;
        }
        Commands::Asm(args) => {
            warn_deprecated("asm", "tui asm");
            commands::explore::run_asm(args).await?;
        }
    }

    Ok(())
}
