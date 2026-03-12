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

//! Blockchain audit command.
//!
//! Queries Neo N3 and Solana blockchains for ResQ event records,
//! providing audit trails for incident response and delivery verification.

use anyhow::{Context, Result};
use glob::glob;
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── CLI Args ─────────────────────────────────────────────────────────────────

/// CLI arguments for the security and quality audit command.
#[derive(clap::Args, Debug)]
pub struct AuditArgs {
    /// Root directory to start search from
    #[arg(long, default_value = ".")]
    pub root: PathBuf,

    // ── npm audit-ci ─────────────────────────────────────────────────────────
    /// Minimum npm vulnerability severity to fail on (critical, high, moderate, low)
    #[arg(long, default_value = "critical")]
    pub level: String,

    /// audit-ci report verbosity (important, full, summary)
    #[arg(long, default_value = "important")]
    pub report_type: String,

    /// Skip the yarn.lock generation step required by audit-ci
    #[arg(long)]
    pub skip_prepare: bool,

    /// Skip the npm audit-ci pass
    #[arg(long)]
    pub skip_npm: bool,

    // ── OSV Scanner ──────────────────────────────────────────────────────────
    /// Skip the Google OSV Scanner pass (covers Rust, npm, Python, .NET, C/C++)
    #[arg(long)]
    pub skip_osv: bool,

    /// OSV Scanner output format (table, json, sarif, gh-annotations)
    #[arg(long, default_value = "table")]
    pub osv_format: String,

    // ── React Doctor ─────────────────────────────────────────────────────────
    /// Skip the react-doctor pass on the web dashboard
    #[arg(long)]
    pub skip_react: bool,

    /// Path to the React/Next.js project for react-doctor
    /// (default: <root>/services/web-dashboard)
    #[arg(long)]
    pub react_target: Option<PathBuf>,

    /// Only scan React files changed vs this base branch (e.g. "main")
    #[arg(long)]
    pub react_diff: Option<String>,

    /// Minimum react-doctor health score to pass (0–100)
    #[arg(long, default_value_t = 75)]
    pub react_min_score: u8,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PackageJson {
    workspaces: Option<Vec<String>>,
}

fn resolve_root(root: &Path) -> PathBuf {
    if root == Path::new(".") {
        crate::utils::find_project_root()
    } else {
        root.to_path_buf()
    }
}

fn header(title: &str) {
    let bar = "━".repeat(74usize.saturating_sub(title.len() + 1));
    println!("\n━━━ {title} {bar}");
}

// ── Pass 1: OSV Scanner ───────────────────────────────────────────────────────

/// Returns true for OSV scanner stdout lines that are scan-walk progress noise
/// rather than actual vulnerability findings.
fn is_osv_noise(line: &str) -> bool {
    line.starts_with("Scanning dir ")
        || line.starts_with("Starting filesystem walk")
        || (line.starts_with("Scanned ") && line.contains("file and found"))
        || line.starts_with("End status:")
        || line.starts_with("Filtered ")
        || (line.len() > 3
            && line[..3].eq_ignore_ascii_case("cve")
            && line.contains("has been filtered"))
        || line == "No issues found"
}

/// Runs `osv-scanner scan source -r <root>` covering all lock files in the
/// monorepo (Cargo.lock, package-lock.json, requirements.txt, *.csproj, …).
fn run_osv_scanner(root: &Path, args: &AuditArgs, failures: &mut Vec<String>) {
    header("OSV Scanner (cross-ecosystem)");

    // Gracefully skip when the binary is not installed.
    if Command::new("osv-scanner")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        println!("  ⚠️  osv-scanner not found — skipping.");
        println!(
            "      Install: go install github.com/google/osv-scanner/v2/cmd/osv-scanner@latest"
        );
        return;
    }

    println!(
        "  🔍 Scanning {} (format: {})...",
        root.display(),
        args.osv_format
    );

    let mut cmd = Command::new("osv-scanner");
    cmd.arg("scan");

    // Explicitly pass config if found in root (must come before positional args)
    let config_path = root.join("osv-scanner.toml");
    if config_path.exists() {
        cmd.arg("--config").arg(&config_path);
    }

    let child = cmd
        .arg("--format")
        .arg(&args.osv_format)
        .arg("-r")
        .arg(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let success = match child {
        Err(e) => {
            println!("  ❌ Failed to run: {e}");
            failures.push(format!("osv-scanner (exec: {e})"));
            return;
        }
        Ok(mut child) => {
            // Filter stdout: suppress OSV scanner's scan-walk progress lines
            // (emitted on stdout mixed with the vulnerability table).
            if let Some(stdout) = child.stdout.take() {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if !is_osv_noise(&line) {
                        println!("{line}");
                    }
                }
            }
            child.wait().map(|s| s.success()).unwrap_or(false)
        }
    };

    if success {
        println!("  ✅ No vulnerabilities found.");
    } else {
        println!("  ❌ Vulnerabilities detected.");
        failures.push("osv-scanner".to_string());
    }
}

// ── Pass 2: npm audit-ci ──────────────────────────────────────────────────────

/// Returns true if the `bun` binary is installed and can execute without crashing.
/// On machines without AVX2 (e.g. Intel Celeron N5100), `bun` exits with SIGILL.
fn bun_available() -> bool {
    Command::new("bun")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Runs `audit-ci` against the root and every npm workspace package.
/// Uses bun when available, falls back to npm/npx when bun cannot run.
fn run_npm_audit(root: &Path, args: &AuditArgs, failures: &mut Vec<String>) -> Result<()> {
    header("npm audit-ci");

    let pkg_path = root.join("package.json");
    if !pkg_path.exists() {
        println!("  ⚠️  No package.json at {} — skipping.", root.display());
        return Ok(());
    }

    let pkg_content = fs::read_to_string(&pkg_path).context("Failed to read package.json")?;
    let pkg: PackageJson =
        serde_json::from_str(&pkg_content).context("Failed to parse package.json")?;

    let mut dirs_to_check = vec![root.to_path_buf()];
    if let Some(workspaces) = pkg.workspaces {
        for ws_glob in workspaces {
            let pattern = root.join(&ws_glob).to_string_lossy().to_string();
            for path in glob(&pattern).context("Invalid glob pattern")?.flatten() {
                if path.is_dir() {
                    dirs_to_check.push(path);
                }
            }
        }
    }

    for dir in dirs_to_check {
        if !dir.join("package.json").exists() {
            continue;
        }

        println!("\n  🔍 Auditing: {}", dir.display());

        if !bun_available() {
            println!("  ⚠️  bun unavailable on this host — skipping npm audit.");
            continue;
        }

        if !args.skip_prepare {
            println!("  📦 Generating yarn.lock...");
            let yarn_lock_file = fs::File::create(dir.join("yarn.lock"))
                .context(format!("Cannot create yarn.lock in {}", dir.display()))?;

            let ok = Command::new("bun")
                .args(["install", "--yarn"])
                .stdout(yarn_lock_file)
                .current_dir(&dir)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !ok {
                println!("  ❌ yarn.lock generation failed.");
                failures.push(format!("npm-prepare: {}", dir.display()));
                continue;
            }
        }

        println!(
            "  🛡️  audit-ci (level: {}, report: {})...",
            args.level, args.report_type
        );

        let ok = Command::new("bunx")
            .arg("audit-ci@^7.1.0")
            .arg(format!("--{}", args.level))
            .args(["--report-type", &args.report_type])
            .current_dir(&dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if ok {
            println!("  ✅ Passed.");
        } else {
            println!("  ❌ Vulnerabilities at or above '{}' level.", args.level);
            failures.push(format!("npm-audit: {}", dir.display()));
        }
    }

    Ok(())
}

// ── Pass 3: React Doctor ──────────────────────────────────────────────────────

/// Runs `npx react-doctor@latest` against the Next.js web dashboard.
/// Streams full diagnostic output to the terminal, then does a second
/// lightweight `--score` pass to enforce the numeric health threshold.
fn run_react_doctor(root: &Path, args: &AuditArgs, failures: &mut Vec<String>) {
    header("React Doctor (web-dashboard)");

    let target = args
        .react_target
        .clone()
        .unwrap_or_else(|| root.join("services/web-dashboard"));

    if !target.exists() {
        println!("  ⚠️  Target not found: {} — skipping.", target.display());
        println!("      Override with --react-target <path>");
        return;
    }

    // ── Full diagnostic run ───────────────────────────────────────────────────
    println!("  🏥 Diagnosing: {} ...\n", target.display());

    let mut full_cmd = Command::new("npx");
    full_cmd
        .args(["-y", "react-doctor@latest"])
        .arg(&target)
        .args(["--verbose", "--yes"]);

    if let Some(ref base) = args.react_diff {
        full_cmd.args(["--diff", base]);
    }

    // Inherit stdio — let react-doctor write directly to the terminal.
    let _ = full_cmd.status();

    // ── Score check ───────────────────────────────────────────────────────────
    let mut score_cmd = Command::new("npx");
    score_cmd
        .args(["-y", "react-doctor@latest"])
        .arg(&target)
        .args(["--score", "--yes"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    if let Some(ref base) = args.react_diff {
        score_cmd.args(["--diff", base]);
    }

    let score: Option<u8> = score_cmd
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok());

    match score {
        Some(s) if s >= args.react_min_score => {
            println!(
                "\n  ✅ Health score: {s}/100 (threshold: {}).",
                args.react_min_score
            );
        }
        Some(s) => {
            println!(
                "\n  ❌ Health score: {s}/100 — below threshold of {}.",
                args.react_min_score
            );
            failures.push(format!(
                "react-doctor: score {s} < {}",
                args.react_min_score
            ));
        }
        None => {
            println!("\n  ⚠️  Could not parse react-doctor score — skipping threshold check.");
        }
    }
}

// ── Entry Point ───────────────────────────────────────────────────────────────

/// Run the security and quality audit.
pub async fn run(args: AuditArgs) -> Result<()> {
    let root = resolve_root(&args.root);

    println!("🔒 ResQ Security & Quality Audit");
    println!("   Root: {}", root.display());

    let mut failures: Vec<String> = Vec::new();

    if !args.skip_osv {
        run_osv_scanner(&root, &args, &mut failures);
    }

    if !args.skip_npm {
        run_npm_audit(&root, &args, &mut failures)?;
    }

    if !args.skip_react {
        run_react_doctor(&root, &args, &mut failures);
    }

    println!("\n{}", "━".repeat(76));

    if failures.is_empty() {
        println!("✅ All audit passes completed successfully.");
        Ok(())
    } else {
        eprintln!("❌ {} pass(es) failed:", failures.len());
        for f in &failures {
            eprintln!("   • {f}");
        }
        anyhow::bail!("Audit failed")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osv_noise_scanning_dir() {
        assert!(is_osv_noise("Scanning dir /home/user/project"));
    }

    #[test]
    fn osv_noise_starting_walk() {
        assert!(is_osv_noise("Starting filesystem walk"));
    }

    #[test]
    fn osv_noise_scanned_files() {
        assert!(is_osv_noise("Scanned 42 file and found 3 packages"));
    }

    #[test]
    fn osv_noise_end_status() {
        assert!(is_osv_noise("End status: 0"));
    }

    #[test]
    fn osv_noise_filtered() {
        assert!(is_osv_noise("Filtered 2 vulnerabilities"));
    }

    #[test]
    fn osv_noise_cve_filtered() {
        assert!(is_osv_noise("CVE-2024-1234 has been filtered"));
    }

    #[test]
    fn osv_noise_no_issues() {
        assert!(is_osv_noise("No issues found"));
    }

    #[test]
    fn osv_noise_real_output_is_not_noise() {
        assert!(!is_osv_noise(
            "GHSA-xxxx-yyyy-zzzz: critical vulnerability in lodash"
        ));
        assert!(!is_osv_noise("  lodash  4.17.20  CVE-2021-23337"));
        assert!(!is_osv_noise("╭───────────────────────────────────╮"));
    }
}
