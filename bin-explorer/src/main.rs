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

//! Bin-Explorer CLI and TUI.

#![deny(missing_docs)]

mod cache;
mod tui;

use anyhow::{Context, Result};
use bin_explorer::analysis::{AnalyzeOptions, BinaryAnalyzer, BinaryReport};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Analyze machine-level metadata and disassembly for binaries.
#[derive(Debug, Parser)]
#[command(name = "bin-explorer", version, about)]
struct Args {
    /// Analyze a single binary.
    #[arg(long, conflicts_with = "dir")]
    file: Option<PathBuf>,

    /// Analyze all object-like files under a directory.
    #[arg(long, conflicts_with = "file")]
    dir: Option<PathBuf>,

    /// Include recursive traversal for --dir mode.
    #[arg(long, default_value_t = false)]
    recursive: bool,

    /// Optional filename suffix filter in --dir mode (e.g. .so, .o).
    #[arg(long)]
    ext: Option<String>,

    /// Disable disassembly and only collect metadata.
    #[arg(long, default_value_t = false)]
    no_disasm: bool,

    /// Maximum functions to disassemble per binary.
    #[arg(long)]
    max_functions: Option<usize>,

    /// Optional config file path (TOML).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Disable result cache reads/writes.
    #[arg(long, default_value_t = false)]
    no_cache: bool,

    /// Force refresh cached reports by rebuilding analysis artifacts.
    #[arg(long, default_value_t = false)]
    rebuild_cache: bool,

    /// Force interactive terminal mode.
    #[arg(long, default_value_t = false, conflicts_with_all = ["json", "plain"])]
    tui: bool,

    /// Emit a human-readable non-interactive report.
    #[arg(long, default_value_t = false, conflicts_with_all = ["json", "tui"])]
    plain: bool,

    /// Emit JSON instead of human-readable output.
    #[arg(long, default_value_t = false, conflicts_with_all = ["plain", "tui"])]
    json: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
enum OutputMode {
    /// Interactive TUI.
    Tui,
    /// Human-readable plain text.
    Plain,
    /// JSON report.
    Json,
}

#[derive(Debug, Default, Deserialize)]
struct AppConfig {
    recursive: Option<bool>,
    ext: Option<String>,
    no_disasm: Option<bool>,
    max_functions: Option<usize>,
    output: Option<OutputMode>,
    no_cache: Option<bool>,
    rebuild_cache: Option<bool>,
    cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct AnalyzeIssue {
    path: PathBuf,
    error: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct RunStats {
    total: usize,
    processed: usize,
    failed: usize,
    cache_hits: usize,
}

struct CacheOptions {
    enabled: bool,
    rebuild: bool,
    dir: PathBuf,
}

#[derive(Debug, Clone)]
struct AnalyzeRun {
    reports: Vec<BinaryReport>,
    issues: Vec<AnalyzeIssue>,
    stats: RunStats,
}

#[derive(Debug, Serialize)]
struct JsonOutput {
    stats: RunStats,
    reports: Vec<BinaryReport>,
    issues: Vec<AnalyzeIssue>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = load_config(args.config.as_deref())?;

    let recursive = args.recursive || config.recursive.unwrap_or(false);
    let ext = args.ext.or(config.ext);
    let no_disasm = args.no_disasm || config.no_disasm.unwrap_or(false);
    let max_functions = args.max_functions.or(config.max_functions).unwrap_or(40);
    let no_cache = args.no_cache || config.no_cache.unwrap_or(false);
    let rebuild_cache = args.rebuild_cache || config.rebuild_cache.unwrap_or(false);
    let cache_dir = config
        .cache_dir
        .unwrap_or_else(|| PathBuf::from(".cache/resq/bin-explorer"));
    let cache_options = CacheOptions {
        enabled: !no_cache,
        rebuild: rebuild_cache,
        dir: cache_dir,
    };

    let options = AnalyzeOptions {
        include_disassembly: !no_disasm,
        max_functions,
        ..Default::default()
    };

    let run = if let Some(file) = args.file {
        let analyzed = analyze_one(&file, &options, &cache_options)?;
        AnalyzeRun {
            reports: vec![analyzed.report],
            issues: Vec::new(),
            stats: RunStats {
                total: 1,
                processed: 1,
                failed: 0,
                cache_hits: usize::from(analyzed.cache_hit),
            },
        }
    } else if let Some(dir) = args.dir {
        analyze_dir(&dir, recursive, ext.as_deref(), &options, &cache_options)?
    } else {
        anyhow::bail!("either --file or --dir must be provided");
    };

    if args.json || matches!(config.output, Some(OutputMode::Json)) {
        println!(
            "{}",
            serde_json::to_string_pretty(&JsonOutput {
                stats: run.stats,
                reports: run.reports,
                issues: run.issues,
            })?
        );
        return Ok(());
    }

    let run_tui = if args.plain {
        false
    } else if args.tui {
        true
    } else if matches!(config.output, Some(OutputMode::Plain)) {
        false
    } else if matches!(config.output, Some(OutputMode::Tui)) {
        true
    } else {
        std::io::stdout().is_terminal()
    };

    if run_tui {
        let issues = run
            .issues
            .iter()
            .map(|i| format!("{} :: {}", i.path.display(), i.error))
            .collect::<Vec<_>>();
        tui::run_tui(run.reports, run.stats, issues)
    } else {
        print_human(&run);
        Ok(())
    }
}

fn load_config(path: Option<&Path>) -> Result<AppConfig> {
    let path = if let Some(path) = path {
        path.to_path_buf()
    } else {
        PathBuf::from(".resq-bin-explorer.toml")
    };

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed reading config file {}", path.display()))?;
    let parsed = toml::from_str::<AppConfig>(&raw)
        .with_context(|| format!("invalid config TOML in {}", path.display()))?;
    Ok(parsed)
}

fn analyze_one(
    path: &Path,
    options: &AnalyzeOptions,
    cache_options: &CacheOptions,
) -> Result<cache::CacheLookup> {
    cache::load_or_analyze(path, options, cache_options, BinaryAnalyzer::analyze_path)
}

fn analyze_dir(
    dir: &Path,
    recursive: bool,
    ext_filter: Option<&str>,
    options: &AnalyzeOptions,
    cache_options: &CacheOptions,
) -> Result<AnalyzeRun> {
    let mut reports = Vec::new();
    let mut issues = Vec::new();
    let mut total = 0usize;
    let mut cache_hits = 0usize;

    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .with_context(|| format!("failed reading directory {}", current.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let ty = entry.file_type()?;

            if ty.is_dir() {
                if recursive {
                    stack.push(path);
                }
                continue;
            }

            if !ty.is_file() {
                continue;
            }

            if let Some(ext) = ext_filter {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.ends_with(ext) {
                    continue;
                }
            }

            total += 1;
            match analyze_one(&path, options, cache_options) {
                Ok(cached) => {
                    cache_hits += usize::from(cached.cache_hit);
                    reports.push(cached.report);
                },
                Err(err) => issues.push(AnalyzeIssue {
                    path,
                    error: err.to_string(),
                }),
            }
        }
    }

    let failed = issues.len();
    let processed = total.saturating_sub(failed);

    Ok(AnalyzeRun {
        reports,
        issues,
        stats: RunStats {
            total,
            processed,
            failed,
            cache_hits,
        },
    })
}

fn print_human(run: &AnalyzeRun) {
    println!(
        "scan: total={} processed={} failed={} cache_hits={}",
        run.stats.total, run.stats.processed, run.stats.failed, run.stats.cache_hits
    );

    for report in &run.reports {
        println!("== {} ==", report.path.display());
        println!(
            "format={} arch={} endian={} size={}B entry=0x{:x}",
            report.format, report.architecture, report.endianness, report.size_bytes, report.entry
        );
        println!(
            "sections={} symbols={} functions={}",
            report.sections.len(),
            report.symbols.len(),
            report.functions.len()
        );
        println!(
            "disasm_backend={} attempts={}",
            report.disassembly_backend.as_deref().unwrap_or("<none>"),
            report.disassembly_attempts.join(" | ")
        );
        if let Some(coverage) = &report.disassembly_coverage {
            println!(
                "coverage: total={} with_insn={} capstone={} objdump={} missing={}",
                coverage.total_functions,
                coverage.functions_with_instructions,
                coverage.capstone_functions,
                coverage.objdump_functions,
                coverage.missing_functions
            );
        }

        for warning in &report.warnings {
            println!("warning: {warning}");
        }

        for function in report.functions.iter().take(8) {
            println!(
                "  fn {} @ 0x{:x} size={} insn={}",
                function.name,
                function.address,
                function.size,
                function.instructions.len()
            );
        }

        println!();
    }

    if !run.issues.is_empty() {
        println!("skipped files (showing up to 20):");
        for issue in run.issues.iter().take(20) {
            println!("  - {} :: {}", issue.path.display(), issue.error);
        }
        if run.issues.len() > 20 {
            println!("  ... and {} more", run.issues.len() - 20);
        }
    }
}
