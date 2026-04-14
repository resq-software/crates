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

//! `resq format` — polyglot formatter with a shared implementation that
//! powers both the standalone command and the pre-commit format steps.
//!
//! Design: each language exports `format_<lang>(root, files, check)` —
//! pre-commit calls these on its staged file list and then restages; the
//! CLI wrapper (`resq format`) calls them on an empty list, which tells
//! each formatter to operate on the whole project.

use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Outcome of a per-language format step.
#[derive(Debug, PartialEq, Eq)]
pub enum FormatOutcome {
    /// The formatter ran and made no changes (or found no issues in `--check`).
    Clean,
    /// The formatter ran and either rewrote files or — in `--check` — found issues.
    /// Returns the original stderr (if any) for reporting.
    Formatted,
    /// Skipped: either no matching files or the required tool isn't installed.
    Skipped(String),
    /// Formatter exited with a non-zero status unexpectedly.
    Failed(String),
}

impl FormatOutcome {
    /// `true` iff the step should be treated as a pass for pre-commit gating.
    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self, Self::Clean | Self::Formatted | Self::Skipped(_))
    }
}

/// Arguments for the `format` command.
#[derive(Parser, Debug)]
pub struct FormatArgs {
    /// Language to format. If omitted, runs every detected language.
    #[arg(long, value_parser = ["rust", "ts", "python", "cpp", "csharp", "all"])]
    pub language: Option<String>,

    /// Report issues without rewriting files. Exits non-zero if any found.
    #[arg(long)]
    pub check: bool,
}

fn has_cmd(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn find_root() -> PathBuf {
    crate::utils::find_project_root()
}

/// Format Rust files via `cargo fmt` (runs against the whole workspace
/// when `files` is empty).
///
/// # Errors
/// Never — failures are reported via `FormatOutcome::Failed(stderr)`.
#[allow(clippy::unnecessary_wraps)]
pub fn format_rust(root: &Path, files: &[String], check: bool) -> Result<FormatOutcome> {
    let workspace_mode = files.is_empty();
    if workspace_mode && !root.join("Cargo.toml").exists() {
        return Ok(FormatOutcome::Skipped("no Cargo.toml".into()));
    }
    if !workspace_mode && !files.iter().any(|f| f.ends_with(".rs")) {
        return Ok(FormatOutcome::Skipped("no .rs files".into()));
    }
    if !has_cmd("cargo") {
        return Ok(FormatOutcome::Skipped("cargo not on PATH".into()));
    }
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).arg("fmt").arg("--all");
    if check {
        cmd.args(["--", "--check"]);
    }
    let out = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output();
    finalize(out, check)
}

/// Format JS/TS/JSON/CSS files via Biome (preferring `biome` over `bunx --bun biome`).
///
/// # Errors
/// Never — failures are reported via `FormatOutcome::Failed(stderr)`.
#[allow(clippy::unnecessary_wraps)]
pub fn format_ts(root: &Path, files: &[String], check: bool) -> Result<FormatOutcome> {
    const EXTS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".json", ".css"];
    let workspace_mode = files.is_empty();
    if workspace_mode
        && !root.join("package.json").exists()
        && !root.join("biome.json").exists()
        && !root.join("biome.jsonc").exists()
    {
        return Ok(FormatOutcome::Skipped(
            "no package.json / biome config".into(),
        ));
    }
    if !workspace_mode && !files.iter().any(|f| EXTS.iter().any(|e| f.ends_with(e))) {
        return Ok(FormatOutcome::Skipped("no TS/JS files".into()));
    }
    let (cmd, prefix) = if has_cmd("biome") {
        ("biome", Vec::<&str>::new())
    } else if has_cmd("bunx") {
        ("bunx", vec!["--bun", "biome"])
    } else {
        return Ok(FormatOutcome::Skipped("biome / bunx not on PATH".into()));
    };
    let mut args: Vec<String> = prefix.iter().map(|s| (*s).to_string()).collect();
    args.push("format".into());
    if !check {
        args.push("--write".into());
    }
    if workspace_mode {
        args.push(".".into());
    } else {
        args.extend(files.iter().cloned());
    }
    let out = Command::new(cmd)
        .args(&args)
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    finalize(out, check)
}

/// Format Python files via `ruff format`.
///
/// # Errors
/// Never — failures are reported via `FormatOutcome::Failed(stderr)`.
#[allow(clippy::unnecessary_wraps)]
pub fn format_python(root: &Path, files: &[String], check: bool) -> Result<FormatOutcome> {
    let workspace_mode = files.is_empty();
    if workspace_mode
        && !root.join("pyproject.toml").exists()
        && !root.join("setup.py").exists()
        && !root.join("setup.cfg").exists()
    {
        return Ok(FormatOutcome::Skipped("no Python project markers".into()));
    }
    if !workspace_mode && !files.iter().any(|f| f.ends_with(".py")) {
        return Ok(FormatOutcome::Skipped("no .py files".into()));
    }
    if !has_cmd("ruff") {
        return Ok(FormatOutcome::Skipped("ruff not on PATH".into()));
    }
    let mut cmd = Command::new("ruff");
    cmd.current_dir(root).arg("format");
    if check {
        cmd.arg("--check");
    }
    if workspace_mode {
        cmd.arg(".");
    } else {
        cmd.args(files);
    }
    let out = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output();
    finalize(out, check)
}

/// Format C/C++ files via `clang-format`.
///
/// # Errors
/// Never — failures are reported via `FormatOutcome::Failed(stderr)`.
#[allow(clippy::unnecessary_wraps)]
pub fn format_cpp(root: &Path, files: &[String], check: bool) -> Result<FormatOutcome> {
    const EXTS: &[&str] = &[".cpp", ".cc", ".h", ".hpp"];
    let workspace_mode = files.is_empty();
    let targets: Vec<String> = if workspace_mode {
        // Discover .cpp/.cc/.h/.hpp under the workspace root, depth-limited.
        walkdir::WalkDir::new(root)
            .max_depth(10)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.path().to_str().map(String::from))
            .filter(|p| EXTS.iter().any(|ext| p.ends_with(ext)))
            .filter(|p| !p.contains("/target/") && !p.contains("/node_modules/"))
            .collect()
    } else {
        files
            .iter()
            .filter(|f| EXTS.iter().any(|e| f.ends_with(e)))
            .cloned()
            .collect()
    };
    if targets.is_empty() {
        return Ok(FormatOutcome::Skipped("no C/C++ files".into()));
    }
    if !has_cmd("clang-format") {
        return Ok(FormatOutcome::Skipped("clang-format not on PATH".into()));
    }
    let mut cmd = Command::new("clang-format");
    cmd.current_dir(root);
    if check {
        cmd.args(["--dry-run", "--Werror"]);
    } else {
        cmd.arg("-i");
    }
    cmd.args(&targets);
    let out = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output();
    finalize(out, check)
}

/// Format C# via `dotnet format`.
///
/// # Errors
/// Never — failures are reported via `FormatOutcome::Failed(stderr)`.
#[allow(clippy::unnecessary_wraps)]
pub fn format_csharp(root: &Path, files: &[String], check: bool) -> Result<FormatOutcome> {
    let workspace_mode = files.is_empty();
    if !workspace_mode && !files.iter().any(|f| f.ends_with(".cs")) {
        return Ok(FormatOutcome::Skipped("no .cs files".into()));
    }
    if !has_cmd("dotnet") {
        return Ok(FormatOutcome::Skipped("dotnet not on PATH".into()));
    }
    let sln = root.join("libs/dotnet/ResQ.Packages.sln");
    if !sln.exists() {
        return Ok(FormatOutcome::Skipped("no ResQ.Packages.sln".into()));
    }
    let mut cmd = Command::new("dotnet");
    cmd.current_dir(root).args([
        "format",
        "libs/dotnet/ResQ.Packages.sln",
        "--verbosity",
        "quiet",
    ]);
    if check {
        cmd.arg("--verify-no-changes");
    }
    let out = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output();
    finalize(out, check)
}

fn finalize(out: std::io::Result<std::process::Output>, check: bool) -> Result<FormatOutcome> {
    let Ok(output) = out else {
        return Ok(FormatOutcome::Failed("process spawn failed".into()));
    };
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(if check {
            FormatOutcome::Clean
        } else {
            FormatOutcome::Formatted
        })
    } else if check {
        // In --check mode, a non-zero exit is the expected signal for
        // "would-be-formatted". Record it as Failed so callers can surface
        // the stderr, but the CLI wrapper treats this as the --check signal.
        Ok(FormatOutcome::Failed(stderr))
    } else {
        Ok(FormatOutcome::Failed(stderr))
    }
}

/// Executes the `format` command.
///
/// # Errors
/// Returns an error only if the argument validation fails. Per-language
/// failures are reported on stderr and accumulate into the CLI exit code.
pub async fn run(args: FormatArgs) -> Result<()> {
    let root = find_root();
    let langs: &[&str] = match args.language.as_deref() {
        None | Some("all") => &["rust", "ts", "python", "cpp", "csharp"],
        Some("rust") => &["rust"],
        Some("ts") => &["ts"],
        Some("python") => &["python"],
        Some("cpp") => &["cpp"],
        Some("csharp") => &["csharp"],
        Some(other) => anyhow::bail!("Unknown --language '{other}'"),
    };

    let mut any_failed = false;
    for lang in langs {
        let outcome = match *lang {
            "rust" => format_rust(&root, &[], args.check)?,
            "ts" => format_ts(&root, &[], args.check)?,
            "python" => format_python(&root, &[], args.check)?,
            "cpp" => format_cpp(&root, &[], args.check)?,
            "csharp" => format_csharp(&root, &[], args.check)?,
            _ => unreachable!(),
        };
        match outcome {
            FormatOutcome::Clean => println!("  ✅ {lang}: clean"),
            FormatOutcome::Formatted => {
                if args.check {
                    // Shouldn't reach here — check-mode success maps to Clean.
                    println!("  ✅ {lang}: clean");
                } else {
                    println!("  ✨ {lang}: formatted");
                }
            }
            FormatOutcome::Skipped(reason) => {
                println!("  ⏭  {lang}: skipped ({reason})");
            }
            FormatOutcome::Failed(stderr) => {
                if args.check {
                    println!("  ❌ {lang}: would reformat (run without --check to fix)");
                } else {
                    println!("  ❌ {lang}: formatter failed");
                }
                if !stderr.trim().is_empty() {
                    for line in stderr.lines().take(20) {
                        println!("      {line}");
                    }
                }
                any_failed = true;
            }
        }
    }

    if any_failed {
        anyhow::bail!(
            "{} issue(s); run without --check to fix",
            if args.check { "format" } else { "formatter" }
        );
    }
    Ok(())
}
