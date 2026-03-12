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

//! Copyright header command.
//!
//! Checks and updates copyright headers in source files to ensure
//! proper licensing and attribution.

use anyhow::{Context, Result};
use chrono::Datelike;
use glob::glob;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

// ── CLI Args ────────────────────────────────────────────────────────────────

/// CLI arguments for the copyright header management command.
#[derive(clap::Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct CopyrightArgs {
    /// License type (apache-2.0, mit, gpl-3.0, bsd-3-clause)
    #[arg(short, long, default_value = "apache-2.0")]
    pub license: String,

    /// Copyright holder name
    #[arg(short, long, default_value = "ResQ")]
    pub author: String,

    /// Copyright year (defaults to current year)
    #[arg(short, long)]
    pub year: Option<String>,

    /// Overwrite existing headers
    #[arg(long)]
    pub force: bool,

    /// Preview changes without writing files
    #[arg(long)]
    pub dry_run: bool,

    /// Check for missing headers (CI mode, exits non-zero if any missing)
    #[arg(long)]
    pub check: bool,

    /// Print detailed processing info
    #[arg(short, long)]
    pub verbose: bool,

    /// Glob patterns to match files (e.g. "src/**/*.rs")
    #[arg(long)]
    pub glob: Vec<String>,

    /// File extensions to include (e.g. --ext rs,js,py)
    #[arg(long, value_delimiter = ',')]
    pub ext: Vec<String>,

    /// Patterns to exclude from processing
    #[arg(short, long)]
    pub exclude: Vec<String>,
}

// ── License Templates ───────────────────────────────────────────────────────

const VALID_LICENSES: &[&str] = &["apache-2.0", "mit", "gpl-3.0", "bsd-3-clause"];

fn get_license_template(license: &str, author: &str, year: &str) -> Result<String> {
    let text = match license {
        "apache-2.0" => format!(
            "Copyright {year} {author}\n\n\
             Licensed under the Apache License, Version 2.0 (the \"License\");\n\
             you may not use this file except in compliance with the License.\n\
             You may obtain a copy of the License at\n\n\
             \x20   http://www.apache.org/licenses/LICENSE-2.0\n\n\
             Unless required by applicable law or agreed to in writing, software\n\
             distributed under the License is distributed on an \"AS IS\" BASIS,\n\
             WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\n\
             See the License for the specific language governing permissions and\n\
             limitations under the License."
        ),
        "mit" => format!(
            "Copyright (c) {year} {author}\n\n\
             Permission is hereby granted, free of charge, to any person obtaining a copy\n\
             of this software and associated documentation files (the \"Software\"), to deal\n\
             in the Software without restriction, including without limitation the rights\n\
             to use, copy, modify, merge, publish, distribute, sublicense, and/or sell\n\
             copies of the Software, and to permit persons to whom the Software is\n\
             furnished to do so, subject to the following conditions:\n\n\
             The above copyright notice and this permission notice shall be included in all\n\
             copies or substantial portions of the Software.\n\n\
             THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\n\
             IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\n\
             FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\n\
             AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\n\
             LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\n\
             OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\n\
             SOFTWARE."
        ),
        "gpl-3.0" => format!(
            "Copyright (C) {year} {author}\n\n\
             This program is free software: you can redistribute it and/or modify\n\
             it under the terms of the GNU General Public License as published by\n\
             the Free Software Foundation, either version 3 of the License, or\n\
             (at your option) any later version.\n\n\
             This program is distributed in the hope that it will be useful,\n\
             but WITHOUT ANY WARRANTY; without even the implied warranty of\n\
             MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the\n\
             GNU General Public License for more details.\n\n\
             You should have received a copy of the GNU General Public License\n\
             along with this program. If not, see <https://www.gnu.org/licenses/>."
        ),
        "bsd-3-clause" => format!(
            "Copyright (c) {year}, {author}\n\
             All rights reserved.\n\n\
             Redistribution and use in source and binary forms, with or without\n\
             modification, are permitted provided that the following conditions are met:\n\n\
             1. Redistributions of source code must retain the above copyright notice, this\n\
             \x20  list of conditions and the following disclaimer.\n\
             2. Redistributions in binary form must reproduce the above copyright notice,\n\
             \x20  this list of conditions and the following disclaimer in the documentation\n\
             \x20  and/or other materials provided with the distribution.\n\
             3. Neither the name of the copyright holder nor the names of its\n\
             \x20  contributors may be used to endorse or promote products derived from\n\
             \x20  this software without specific prior written permission.\n\n\
             THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\"\n\
             AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE\n\
             IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\n\
             DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE\n\
             FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL\n\
             DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR\n\
             SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER\n\
             CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,\n\
             OR (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE\n\
             OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE."
        ),
        _ => anyhow::bail!("Unsupported license: '{license}'. Valid options: {VALID_LICENSES:?}"),
    };
    Ok(text)
}

// ── Comment Styles ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum CommentKind {
    Block,
    Line,
}

#[derive(Debug)]
struct CommentStyle {
    kind: CommentKind,
    open: Option<&'static str>,
    line: &'static str,
    close: Option<&'static str>,
}

impl CommentStyle {
    const fn block(open: &'static str, line: &'static str, close: &'static str) -> Self {
        Self {
            kind: CommentKind::Block,
            open: Some(open),
            line,
            close: Some(close),
        }
    }

    const fn line(prefix: &'static str) -> Self {
        Self {
            kind: CommentKind::Line,
            open: None,
            line: prefix,
            close: None,
        }
    }
}

const C_STYLE_BLOCK: CommentStyle = CommentStyle::block("/**", " *", " */");
/// Rust uses `/* */` instead of `/** */` to avoid creating a doc comment that
/// conflicts with `//!` inner doc comments in `lib.rs` crate roots.
const RUST_BLOCK: CommentStyle = CommentStyle::block("/*", " *", " */");
const XML_BLOCK: CommentStyle = CommentStyle::block("<!--", " ", "-->");
const ASCIIDOC_BLOCK: CommentStyle = CommentStyle::block("////", "", "////");
const HASH_LINE: CommentStyle = CommentStyle::line("#");
const DASH_LINE: CommentStyle = CommentStyle::line("--");
const ELISP_LINE: CommentStyle = CommentStyle::line(";;");

/// Map file extension / filename to comment style.
/// Returns `None` for unsupported or binary file types.
fn get_comment_style(path: &Path, content: &str) -> Option<&'static CommentStyle> {
    // Agent/Claude instruction files are not source files — omit copyright headers.
    static SKIP_FILENAMES: &[&str] = &["AGENTS.md", "CLAUDE.md"];
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if SKIP_FILENAMES.contains(&name) {
            return None;
        }
    }

    // Shebang lines always get hash-style comments
    if content.starts_with("#!/") {
        return Some(&HASH_LINE);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    match ext.as_str() {
        // C-family block comments
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "css" | "scss" | "less" | "styl" | "c"
        | "cc" | "cpp" | "h" | "hpp" | "cs" | "java" | "kt" | "kts" | "swift" | "m" | "mm"
        | "go" | "php" | "dart" | "scala" | "groovy" | "gradle" | "proto" | "zig" | "v" | "sv" => {
            Some(&C_STYLE_BLOCK)
        }

        // Rust — non-doc block comment to avoid conflicting with //! inner docs
        "rs" => Some(&RUST_BLOCK),

        // Markup / XML
        "html" | "htm" | "xml" | "xhtml" | "svg" | "md" | "rst" | "xsl" | "xslt" | "vue"
        | "svelte" => Some(&XML_BLOCK),

        // AsciiDoc
        "adoc" | "asciidoc" => Some(&ASCIIDOC_BLOCK),

        // Hash-line comments
        "sh" | "bash" | "zsh" | "fish" | "py" | "pyi" | "rb" | "pl" | "pm" | "yml" | "yaml"
        | "toml" | "ini" | "cfg" | "conf" | "env" | "mk" | "make" | "r" | "jl" | "tf" | "hcl"
        | "nix" | "cmake" => Some(&HASH_LINE),

        // Double-dash comments
        "sql" | "lua" | "hs" | "elm" => Some(&DASH_LINE),

        // Elisp / Clojure
        "el" | "clj" | "cljs" | "cljc" | "edn" => Some(&ELISP_LINE),

        _ => {
            // Fallback: match well-known filenames
            static HASH_FILENAMES: &[&str] = &[
                "Makefile",
                "Dockerfile",
                "Containerfile",
                "Vagrantfile",
                ".env",
                ".gitignore",
                ".dockerignore",
                ".editorconfig",
                "Gemfile",
                "Rakefile",
                "Justfile",
                "CMakeLists.txt",
            ];
            if HASH_FILENAMES
                .iter()
                .any(|&name| name.eq_ignore_ascii_case(filename))
            {
                Some(&HASH_LINE)
            } else {
                None
            }
        }
    }
}

// ── Header Construction ─────────────────────────────────────────────────────

fn build_header(style: &CommentStyle, license_text: &str) -> String {
    let lines: Vec<&str> = license_text.split('\n').collect();
    let mut header = String::with_capacity(license_text.len() + lines.len() * 4 + 32);

    match style.kind {
        CommentKind::Block => {
            if let Some(open) = style.open {
                header.push_str(open);
                header.push('\n');
            }
            for line in &lines {
                if line.is_empty() {
                    // Avoid trailing whitespace on blank comment lines
                    header.push_str(style.line.trim_end());
                } else {
                    header.push_str(style.line);
                    header.push(' ');
                    header.push_str(line);
                }
                header.push('\n');
            }
            if let Some(close) = style.close {
                header.push_str(close);
                header.push('\n');
            }
            header.push('\n');
        }
        CommentKind::Line => {
            for line in &lines {
                if line.is_empty() {
                    header.push_str(style.line.trim_end());
                } else {
                    header.push_str(style.line);
                    header.push(' ');
                    header.push_str(line);
                }
                header.push('\n');
            }
            header.push('\n');
        }
    }
    header
}

// ── Header Detection ────────────────────────────────────────────────────────

/// Compiled regexes, initialized once.
#[allow(clippy::expect_used)]
static HEADER_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)copyright\s*(\(c\)\s*)?\d{4}|SPDX-License-Identifier:")
        .expect("Static regex pattern is valid")
});

#[allow(clippy::expect_used)]
static COMMENT_START_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^\s*(#|--|//|;;)").expect("Static regex pattern is valid")
});

/// Check whether the first N lines of `content` contain a copyright header.
fn has_header(content: &str) -> bool {
    let head: String = content.lines().take(20).collect::<Vec<_>>().join("\n");
    HEADER_RE.is_match(&head)
}

// ── License Detection ───────────────────────────────────────────────────────

/// Fingerprints that uniquely identify each supported license inside a header.
#[allow(clippy::expect_used)]
static APACHE_FP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)Apache\s+License|apache\.org/licenses").expect("Static regex pattern is valid")
});
#[allow(clippy::expect_used)]
static MIT_FP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)Permission is hereby granted|\bMIT\s+License\b")
        .expect("Static regex pattern is valid")
});
#[allow(clippy::expect_used)]
static GPL3_FP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)GNU\s+General\s+Public\s+License|gnu\.org/licenses")
        .expect("Static regex pattern is valid")
});
#[allow(clippy::expect_used)]
static BSD3_FP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)Redistribution and use.*permitted|BSD.*3.*Clause")
        .expect("Static regex pattern is valid")
});

/// SPDX tag regex — captures the identifier value.
#[allow(clippy::expect_used)]
static SPDX_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"(?i)SPDX-License-Identifier:\s*([\w\-.]+)").expect("Static regex pattern is valid")
});

/// Detect which license the existing header uses.
/// Returns a SPDX-style identifier or `None` if unrecognised.
fn detect_header_license(content: &str) -> Option<&'static str> {
    let head: String = content.lines().take(30).collect::<Vec<_>>().join("\n");

    // Prefer an explicit SPDX tag if present.
    if let Some(caps) = SPDX_RE.captures(&head) {
        let id = caps.get(1).map_or("", |m| m.as_str());
        return match id.to_ascii_lowercase().as_str() {
            "apache-2.0" => Some("apache-2.0"),
            "mit" => Some("mit"),
            "gpl-3.0" | "gpl-3.0-only" | "gpl-3.0-or-later" => Some("gpl-3.0"),
            "bsd-3-clause" => Some("bsd-3-clause"),
            _ => None,
        };
    }

    // Fingerprint-based detection.
    if APACHE_FP.is_match(&head) {
        return Some("apache-2.0");
    }
    if MIT_FP.is_match(&head) {
        return Some("mit");
    }
    if GPL3_FP.is_match(&head) {
        return Some("gpl-3.0");
    }
    if BSD3_FP.is_match(&head) {
        return Some("bsd-3-clause");
    }
    None
}

// ── Body License-Name Replacement ───────────────────────────────────────────

/// Human-readable display name for a SPDX identifier.
fn license_display_name(spdx: &str) -> &'static str {
    match spdx {
        "apache-2.0" => "Apache License, Version 2.0",
        "mit" => "MIT License",
        "gpl-3.0" => "GNU General Public License v3.0",
        "bsd-3-clause" => "BSD 3-Clause License",
        _ => "Unknown",
    }
}

/// Shields.io badge fragment for a license (used in `img.shields.io/badge/…`).
fn license_badge_fragment(spdx: &str) -> &'static str {
    match spdx {
        "apache-2.0" => "License-Apache%202.0-blue.svg",
        "mit" => "License-MIT-blue.svg",
        "gpl-3.0" => "License-GPL%20v3-blue.svg",
        "bsd-3-clause" => "License-BSD%203--Clause-blue.svg",
        _ => "License-Unknown-lightgrey.svg",
    }
}

/// Shields.io Markdown badge label for a license.
fn license_badge_label(spdx: &str) -> &'static str {
    match spdx {
        "apache-2.0" => "License: Apache 2.0",
        "mit" => "License: MIT",
        "gpl-3.0" => "License: GPL v3",
        "bsd-3-clause" => "License: BSD 3-Clause",
        _ => "License",
    }
}

/// SPDX identifier in the canonical casing expected by tooling.
fn license_spdx_canonical(spdx: &str) -> &str {
    match spdx {
        "apache-2.0" => "Apache-2.0",
        "mit" => "MIT",
        "gpl-3.0" => "GPL-3.0-only",
        "bsd-3-clause" => "BSD-3-Clause",
        _ => spdx,
    }
}

/// Replace targeted license-name patterns in the body of a file.
///
/// This only touches well-known patterns (SPDX tags, shield.io badges,
/// "licensed under" prose) — generic mentions of license names (e.g. in
/// dependency lists) are intentionally left alone.
fn replace_license_mentions(content: &str, from: &str, to: &str) -> Result<String> {
    let mut out = content.to_string();

    // 1. SPDX-License-Identifier tags.
    let spdx_from = license_spdx_canonical(from);
    let spdx_to = license_spdx_canonical(to);
    let spdx_pat = Regex::new(&format!(
        r"(?i)(SPDX-License-Identifier:\s*){}",
        regex::escape(spdx_from)
    ))
    .context("Invalid Regex pattern for SPDX-License-Identifier")?;
    out = spdx_pat
        .replace_all(&out, format!("${{1}}{spdx_to}"))
        .to_string();

    // 2. Shields.io badge URLs.
    let badge_from = license_badge_fragment(from);
    let badge_to = license_badge_fragment(to);
    out = out.replace(badge_from, badge_to);

    // 3. Shields.io badge alt-text / Markdown label.
    let label_from = license_badge_label(from);
    let label_to = license_badge_label(to);
    out = out.replace(label_from, label_to);

    // 4. "licensed under the <License Name>" prose (case-insensitive).
    let name_from = license_display_name(from);
    let name_to = license_display_name(to);
    if name_from != "Unknown" && name_to != "Unknown" {
        // Plain text.
        let prose_pat = Regex::new(&format!(r"(?i){}", regex::escape(name_from)))
            .context("Invalid Regex pattern for license prose")?;
        out = prose_pat.replace_all(&out, name_to).to_string();
        // Bold Markdown variant: **MIT License** → **Apache License, Version 2.0**
        let bold_from = format!("**{name_from}**");
        let bold_to = format!("**{name_to}**");
        out = out.replace(&bold_from, &bold_to);
    }

    Ok(out)
}

fn strip_existing_header(content: &str) -> String {
    let (shebang, rest) = split_shebang(content);

    // Try block-comment stripping first
    if let Some(stripped) = try_strip_block_header(rest) {
        return join_shebang(shebang, &stripped);
    }

    // Try line-comment stripping
    let lines: Vec<&str> = rest.lines().collect();
    let end_idx = find_line_header_end_index(&lines);
    if let Some(idx) = end_idx {
        let stripped = lines[(idx + 1)..]
            .join("\n")
            .trim_start_matches(['\r', '\n'])
            .to_string();
        return join_shebang(shebang, &stripped);
    }

    content.to_string()
}

/// Split optional shebang from rest of file content.
fn split_shebang(content: &str) -> (Option<&str>, &str) {
    if content.starts_with("#!") {
        content.find('\n').map_or((Some(content), ""), |newline| {
            let shebang = &content[..newline];
            let rest = content[newline + 1..].trim_start_matches(['\r', '\n']);
            (Some(shebang), rest)
        })
    } else {
        (None, content)
    }
}

fn join_shebang(shebang: Option<&str>, body: &str) -> String {
    shebang.map_or_else(|| body.to_string(), |s| format!("{s}\n{body}"))
}

fn try_strip_block_header(content: &str) -> Option<String> {
    // C-style block comments: /* ... */
    #[allow(clippy::expect_used)]
    static BLOCK_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^\s*/\*[\s\S]*?\*/\s*").expect("Static regex pattern is valid")
    });
    // XML-style comments: <!-- ... -->
    #[allow(clippy::expect_used)]
    static XML_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^\s*<!--[\s\S]*?-->\s*").expect("Static regex pattern is valid")
    });

    for re in [&*BLOCK_RE, &*XML_RE] {
        if let Some(mat) = re.find(content) {
            if HEADER_RE.is_match(mat.as_str()) {
                return Some(
                    content[mat.end()..]
                        .trim_start_matches(['\r', '\n'])
                        .to_string(),
                );
            }
        }
    }
    None
}

fn find_line_header_end_index(lines: &[&str]) -> Option<usize> {
    let mut header_end: Option<usize> = None;
    let mut in_header = false;
    let max_lines = 30.min(lines.len());

    for (i, line) in lines.iter().enumerate().take(max_lines) {
        if COMMENT_START_RE.is_match(line) {
            if HEADER_RE.is_match(line) {
                in_header = true;
            }
            if in_header {
                header_end = Some(i);
            }
        } else if line.trim().is_empty() && in_header {
            header_end = Some(i);
        } else {
            break;
        }
    }

    if in_header {
        header_end
    } else {
        None
    }
}

// ── Binary Detection ────────────────────────────────────────────────────────

fn is_binary(content: &str) -> bool {
    if content.contains('\0') {
        return true;
    }
    let mut limit = content.len().min(1024);
    while limit > 0 && !content.is_char_boundary(limit) {
        limit -= 1;
    }
    let sample = &content[..limit];
    if sample.is_empty() {
        return false;
    }
    let non_printable = sample
        .chars()
        .filter(|c| {
            let code = *c as u32;
            code < 9 || (code > 13 && code < 32) || (code > 126 && code < 160)
        })
        .count();
    #[allow(clippy::cast_precision_loss)]
    let ratio = non_printable as f64 / sample.len() as f64;
    ratio > 0.1
}

// ── File Discovery ──────────────────────────────────────────────────────────

// Directory excludes are now sourced from `.gitignore` via crate::gitignore.

fn collect_files_from_globs(patterns: &[String], verbose: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if verbose {
        eprintln!("Searching with glob patterns...");
    }
    for pattern in patterns {
        for entry in glob(pattern).context("Failed to read glob pattern")? {
            match entry {
                Ok(path) if path.is_file() => files.push(path),
                _ => {}
            }
        }
    }
    Ok(files)
}

fn collect_files_from_git(verbose: bool) -> Option<Vec<PathBuf>> {
    if verbose {
        eprintln!("Attempting git ls-files...");
    }
    let output = Command::new("git").arg("ls-files").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let mut files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect();

    // Also pick up untracked (but not ignored) files
    if let Ok(untracked) = Command::new("git")
        .args(["ls-files", "-o", "--exclude-standard"])
        .output()
    {
        if untracked.status.success() {
            files.extend(
                String::from_utf8_lossy(&untracked.stdout)
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(PathBuf::from),
            );
        }
    }

    Some(files)
}

fn collect_files_from_walk(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect()
}

fn discover_files(args: &CopyrightArgs) -> Result<Vec<PathBuf>> {
    let root = crate::utils::find_project_root();

    let raw = if !args.glob.is_empty() {
        // Adjust globs to be relative to root or handle them as is
        collect_files_from_globs(&args.glob, args.verbose)?
    } else if let Some(git_files) = collect_files_from_git(args.verbose) {
        git_files.into_iter().map(|p| root.join(p)).collect()
    } else {
        if args.verbose {
            eprintln!(
                "git not available, falling back to directory walk from {}.",
                root.display()
            );
        }
        collect_files_from_walk(&root)
    };

    // Build exclude set: user excludes + gitignore-derived dirs
    let gitignore_excludes = crate::gitignore::parse_gitignore(&root);
    let exclude_patterns: Vec<String> = args
        .exclude
        .iter()
        .cloned()
        .chain(gitignore_excludes)
        .collect();

    // Normalize extensions for filtering
    let ext_filter: HashSet<String> = args
        .ext
        .iter()
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
        .collect();

    let files: Vec<PathBuf> = raw
        .into_iter()
        .filter(|p| {
            let s = p.to_string_lossy();
            !exclude_patterns.iter().any(|ex| s.contains(ex.as_str()))
        })
        .filter(|p| {
            if ext_filter.is_empty() {
                return true;
            }
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ext_filter.contains(&e.to_ascii_lowercase()))
        })
        .collect();

    // Deduplicate (globs or git can return dupes)
    let mut seen = HashSet::with_capacity(files.len());
    Ok(files
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect())
}

// ── Processing ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct Stats {
    updated: usize,
    skipped: usize,
    missing: usize,
    mismatched: usize,
    errors: usize,
}

/// Process a single file to update its copyright header.
///
/// # Errors
/// Returns an error if reading from or writing to the file fails, or if license replacement fails.
fn process_file(
    path: &Path,
    license_body: &str,
    args: &CopyrightArgs,
    stats: &mut Stats,
) -> Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            if args.verbose {
                eprintln!("Skipping {}: {e}", path.display());
            }
            stats.skipped += 1;
            return Ok(());
        }
    };

    if content.trim().is_empty() || is_binary(&content) {
        stats.skipped += 1;
        return Ok(());
    }

    let Some(style) = get_comment_style(path, &content) else {
        if args.verbose {
            eprintln!("Skipping (unsupported type): {}", path.display());
        }
        stats.skipped += 1;
        return Ok(());
    };

    let already_has_header = has_header(&content);
    let detected_license = if already_has_header {
        detect_header_license(&content)
    } else {
        None
    };
    let is_mismatch = already_has_header && detected_license.is_some_and(|d| d != args.license);

    // --check mode: report missing *and* mismatched headers.
    if args.check {
        if !already_has_header {
            println!("Missing header: {}", path.display());
            stats.missing += 1;
        } else if is_mismatch {
            println!(
                "Mismatched license ({} → {}): {}",
                detected_license.unwrap_or("unknown"),
                args.license,
                path.display()
            );
            stats.mismatched += 1;
        }
        return Ok(());
    }

    // Decide whether we need to rewrite this file.
    let needs_rewrite = !already_has_header       // no header yet
        || args.force                              // explicit force
        || is_mismatch; // wrong license

    if !needs_rewrite {
        if args.verbose {
            eprintln!("Skipping (correct header): {}", path.display());
        }
        stats.skipped += 1;
        return Ok(());
    }

    // Strip old header when replacing.
    let base = if already_has_header {
        strip_existing_header(&content)
    } else {
        content.clone()
    };

    // Replace stale license-name mentions in the body when migrating.
    let base = if let Some(old_license) = detected_license {
        if old_license == args.license {
            base
        } else {
            replace_license_mentions(&base, old_license, &args.license)?
        }
    } else {
        base
    };

    let header = build_header(style, license_body);
    let new_content = prepend_header(&base, &header);

    if args.dry_run {
        if is_mismatch {
            println!(
                "Would migrate ({} → {}): {}",
                detected_license.unwrap_or("unknown"),
                args.license,
                path.display()
            );
        } else {
            println!("Would update: {}", path.display());
        }
        stats.updated += 1;
        return Ok(());
    }

    match fs::write(path, &new_content) {
        Ok(()) => {
            if is_mismatch {
                if args.verbose {
                    eprintln!(
                        "Migrated ({} → {}): {}",
                        detected_license.unwrap_or("unknown"),
                        args.license,
                        path.display()
                    );
                }
                stats.mismatched += 1;
            } else if args.verbose {
                eprintln!("Updated: {}", path.display());
            }
            stats.updated += 1;
        }
        Err(e) => {
            eprintln!("Error writing {}: {e}", path.display());
            stats.errors += 1;
        }
    }
    Ok(())
}

/// Prepend header, preserving any shebang line at position 0.
fn prepend_header(content: &str, header: &str) -> String {
    if content.starts_with("#!") {
        let (shebang, rest) = split_shebang(content);
        match shebang {
            Some(s) => format!("{s}\n\n{header}{rest}"),
            None => format!("{header}{content}"),
        }
    } else {
        format!("{header}{content}")
    }
}

// ── Entry Point ─────────────────────────────────────────────────────────────

/// Run the copyright header management command.
/// Runs the copyright tool with the provided arguments.
///
/// # Errors
/// Returns an error if the license year is invalid, the license file cannot be read,
/// or if processing any of the files fails.
pub fn run(args: &CopyrightArgs) -> Result<()> {
    // Validate license upfront
    if !VALID_LICENSES.contains(&args.license.as_str()) {
        anyhow::bail!(
            "Invalid license type: '{}'. Valid options: {:?}",
            args.license,
            VALID_LICENSES
        );
    }

    let year = args
        .year
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().year().to_string());
    let license_body = get_license_template(&args.license, &args.author, &year)?;

    let files = discover_files(args)?;
    if args.verbose {
        eprintln!("Found {} files to process.", files.len());
    }

    let mut stats = Stats::default();

    for path in &files {
        process_file(path, &license_body, args, &mut stats)?;
    }

    // Report results
    if args.check {
        let problems = stats.missing + stats.mismatched;
        if problems > 0 {
            anyhow::bail!(
                "{} file(s) have issues ({} missing, {} mismatched).",
                problems,
                stats.missing,
                stats.mismatched
            );
        }
        if args.verbose {
            println!("All files have correct copyright headers.");
        }
    } else if stats.updated > 0 || stats.errors > 0 {
        println!(
            "Done. Updated: {}, Migrated: {}, Skipped: {}, Errors: {}",
            stats.updated, stats.mismatched, stats.skipped, stats.errors
        );
    }

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_has_header_positive() {
        assert!(has_header("// Copyright (c) 2024 Acme\nfn main() {}"));
        assert!(has_header("# copyright 2023 Foo\nimport os"));
        assert!(has_header("/* SPDX-License-Identifier: MIT */\n"));
    }

    #[test]
    fn test_has_header_negative() {
        assert!(!has_header("fn main() { println!(\"hello\"); }"));
        assert!(!has_header("#!/usr/bin/env python\nimport sys"));
    }

    #[test]
    fn test_is_binary() {
        assert!(is_binary("\0ELF binary content"));
        assert!(!is_binary("fn main() { println!(\"hello\"); }"));
    }

    #[test]
    fn test_is_binary_char_boundary() {
        // '─' is 3 bytes: [226, 148, 128]
        // We want to place it so it straddles the 1024 byte boundary.
        let mut content = "a".repeat(1023);
        content.push('─'); // bytes 1023, 1024, 1025
        content.push_str(" rest of content");

        // This should not panic
        assert!(!is_binary(&content));
    }

    #[test]
    fn test_build_header_block() {
        let header = build_header(&C_STYLE_BLOCK, "Copyright 2024 Test");
        assert!(header.starts_with("/**\n"));
        assert!(header.contains(" * Copyright 2024 Test"));
        assert!(header.contains(" */\n"));
    }

    #[test]
    fn test_build_header_line() {
        let header = build_header(&HASH_LINE, "Copyright 2024 Test");
        assert!(header.starts_with("# Copyright 2024 Test\n"));
        assert!(!header.contains("/**"));
    }

    #[test]
    fn test_shebang_preserved() {
        let content = "#!/usr/bin/env python\nimport os\n";
        let header = "# Copyright 2024 Test\n\n";
        let result = prepend_header(content, header);
        assert!(result.starts_with("#!/usr/bin/env python\n"));
        assert!(result.contains("# Copyright 2024 Test"));
        assert!(result.contains("import os"));
    }

    #[test]
    fn test_strip_existing_block_header() {
        let content = "/** Copyright (c) 2023 Old */\nfn main() {}";
        let stripped = strip_existing_header(content);
        assert_eq!(stripped.trim(), "fn main() {}");
    }

    #[test]
    fn test_get_comment_style() {
        let rs_path = Path::new("main.rs");
        let py_path = Path::new("script.py");
        let html_path = Path::new("index.html");

        assert_eq!(
            get_comment_style(rs_path, "fn main()")
                .expect("Should return comment style for .rs")
                .kind,
            CommentKind::Block
        );
        assert_eq!(
            get_comment_style(py_path, "import os")
                .expect("Should return comment style for .py")
                .kind,
            CommentKind::Line
        );
        assert_eq!(
            get_comment_style(html_path, "<html>")
                .expect("Should return comment style for .html")
                .kind,
            CommentKind::Block
        );
    }

    #[test]
    fn test_license_templates() {
        for license in VALID_LICENSES {
            let result = get_license_template(license, "Test", "2024");
            assert!(result.is_ok(), "Failed for license: {license}");
            assert!(
                result.expect("Should generate theme").contains("2024"),
                "Template should contain year"
            );
        }
        assert!(get_license_template("invalid", "Test", "2024").is_err());
    }

    // ── License Detection Tests ─────────────────────────────────────────

    #[test]
    fn test_detect_apache_header() {
        let content = "/*\n * Copyright 2024 Acme\n *\n * Licensed under the Apache License, Version 2.0\n */\nfn main() {}";
        assert_eq!(detect_header_license(content), Some("apache-2.0"));
    }

    #[test]
    fn test_detect_mit_header() {
        let content = "/*\n * Copyright (c) 2024 Acme\n *\n * Permission is hereby granted, free of charge\n */\nfn main() {}";
        assert_eq!(detect_header_license(content), Some("mit"));
    }

    #[test]
    fn test_detect_gpl_header() {
        let content = "# Copyright (C) 2024 Acme\n# GNU General Public License v3\nimport os";
        assert_eq!(detect_header_license(content), Some("gpl-3.0"));
    }

    #[test]
    fn test_detect_bsd_header() {
        let content = "/*\n * Copyright (c) 2024 Acme\n * Redistribution and use in source and binary forms, with or without modification, are permitted\n */\nint main() {}";
        assert_eq!(detect_header_license(content), Some("bsd-3-clause"));
    }

    #[test]
    fn test_detect_spdx_tag() {
        assert_eq!(
            detect_header_license("// SPDX-License-Identifier: MIT\nfn main() {}"),
            Some("mit")
        );
        assert_eq!(
            detect_header_license("// SPDX-License-Identifier: Apache-2.0\nfn main() {}"),
            Some("apache-2.0")
        );
    }

    #[test]
    fn test_detect_no_license() {
        assert_eq!(
            detect_header_license("// Copyright (c) 2024 Acme\nfn main() {}"),
            None
        );
    }

    // ── Body License-Name Replacement Tests ─────────────────────────────

    #[test]
    fn test_replace_spdx_identifier() {
        let input = "// SPDX-License-Identifier: MIT\nfn main() {}";
        let result = replace_license_mentions(input, "mit", "apache-2.0").unwrap();
        assert!(result.contains("SPDX-License-Identifier: Apache-2.0"));
        assert!(!result.contains("SPDX-License-Identifier: MIT"));
    }

    #[test]
    fn test_replace_badge_url() {
        let input = "[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)";
        let result = replace_license_mentions(input, "mit", "apache-2.0").unwrap();
        assert!(result.contains("License-Apache%202.0-blue.svg"));
        assert!(result.contains("License: Apache 2.0"));
        assert!(!result.contains("License-MIT-blue.svg"));
    }

    #[test]
    fn test_replace_prose_license_name() {
        let input = "This project is licensed under the **MIT License** - see LICENSE.";
        let result = replace_license_mentions(input, "mit", "apache-2.0").unwrap();
        assert!(result.contains("**Apache License, Version 2.0**"));
        assert!(!result.contains("MIT License"));
    }

    #[test]
    fn test_replace_no_false_positives() {
        // Should not change dependency-level mentions that don't match known patterns.
        let input = "dependencies:\n  some-lib: MIT\n  other-lib: BSD";
        let result = replace_license_mentions(input, "mit", "apache-2.0").unwrap();
        // "MIT" alone without "License" suffix should not be replaced
        // (the prose pattern matches "MIT License", not bare "MIT")
        assert!(result.contains("some-lib: MIT"));
    }
}
