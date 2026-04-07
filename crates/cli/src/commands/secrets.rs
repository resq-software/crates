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

//! Secret scanning command for detecting hardcoded credentials.
//!
//! Scans source files for potential secrets like API keys, passwords, tokens,
//! and other sensitive information using pattern matching and entropy analysis.

use aho_corasick::AhoCorasick;
use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use walkdir::WalkDir;

// ── CLI Args ─────────────────────────────────────────────────────────────────

/// CLI arguments for the secrets scanning command.
#[derive(clap::Args, Debug)]
pub struct SecretsArgs {
    /// Root directory to scan (defaults to project root)
    #[arg(long, default_value = ".")]
    pub root: PathBuf,

    /// Only scan git-tracked files
    #[arg(long, default_value_t = true)]
    pub git_only: bool,

    /// Show verbose output (print matched content)
    #[arg(long, short)]
    pub verbose: bool,

    /// Path to allowlist file (one pattern per line)
    #[arg(long)]
    pub allowlist: Option<PathBuf>,

    /// Scan staged changes only (for pre-commit hook integration)
    #[arg(long)]
    pub staged: bool,

    /// Also scan git history (all commits reachable from HEAD)
    #[arg(long)]
    pub history: bool,

    /// Limit history scan to commits after this rev/date (e.g. "30 days ago", "v1.0.0")
    #[arg(long)]
    pub since: Option<String>,
}

// ── Rules ─────────────────────────────────────────────────────────────────────

/// Charset-specific entropy thresholds.
/// Hex max theoretical entropy = 4.0 bits/char (16 symbols)
/// Base64 max = ~6.0 bits/char (64 symbols)
/// Alphanumeric max = ~5.17 bits/char (62 symbols)
#[derive(Clone, Copy)]
enum EntropyCharset {
    Hex,
    Base64,
    Alphanumeric,
}

impl EntropyCharset {
    fn threshold(self) -> f64 {
        match self {
            EntropyCharset::Hex => 3.5,
            EntropyCharset::Base64 => 4.5,
            EntropyCharset::Alphanumeric => 4.0,
        }
    }

    fn min_len(self) -> usize {
        match self {
            EntropyCharset::Hex => 40,
            EntropyCharset::Base64 => 20,
            EntropyCharset::Alphanumeric => 20,
        }
    }
}

/// A secret pattern rule
struct Rule {
    name: &'static str,
    pattern: Regex,
    /// Optional entropy gate applied to the full regex match.
    /// A match that does not meet the entropy threshold is suppressed.
    entropy_gate: Option<EntropyCharset>,
}

/// A single finding
struct Finding {
    file: String,
    line: usize,
    rule: String,
    content: String,
}

// ── Entropy ───────────────────────────────────────────────────────────────────

/// Shannon entropy of a string, counting only bytes present in `charset`.
/// Bytes outside the charset are ignored so the score reflects the density
/// of the charset-relevant portion, not padding or delimiters.
fn charset_entropy(s: &str, charset: EntropyCharset) -> f64 {
    let is_member: fn(u8) -> bool = match charset {
        EntropyCharset::Hex => |b| b.is_ascii_hexdigit(),
        EntropyCharset::Base64 => {
            |b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=')
        }
        EntropyCharset::Alphanumeric => |b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'),
    };

    let mut freq = [0u32; 256];
    let mut count = 0usize;
    for byte in s.bytes() {
        if is_member(byte) {
            freq[byte as usize] += 1;
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    let len = count as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = f64::from(c) / len;
            -p * p.log2()
        })
        .sum()
}

fn passes_entropy_gate(matched: &str, gate: EntropyCharset) -> bool {
    matched.len() >= gate.min_len() && charset_entropy(matched, gate) >= gate.threshold()
}

// ── Pattern Registry ──────────────────────────────────────────────────────────

fn build_rules() -> Vec<Rule> {
    // (name, pattern, entropy_gate)
    let specs: &[(&str, &str, Option<EntropyCharset>)] = &[
        // ── Cloud: AWS ──────────────────────────────────────────────────────
        ("AWS Access Key ID", r"AKIA[0-9A-Z]{16}", None),
        (
            "AWS Secret Access Key",
            r"(?i)aws_secret_access_key\s*[=:]\s*[A-Za-z0-9/+=]{40}",
            Some(EntropyCharset::Base64),
        ),
        (
            "AWS MWS Key",
            r"amzn\.mws\.[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            None,
        ),
        // ── Cloud: GCP ──────────────────────────────────────────────────────
        ("GCP API Key", r"AIza[0-9A-Za-z\-_]{35}", None),
        (
            "GCP Service Account",
            r#""type":\s*"service_account""#,
            None,
        ),
        // ── Cloud: Azure ────────────────────────────────────────────────────
        (
            "Azure Storage Account Key",
            r"(?i)(?:AccountKey|storageaccountkey|DefaultEndpointsProtocol)[=:\s]+[A-Za-z0-9+/]{86}==",
            Some(EntropyCharset::Base64),
        ),
        (
            "Azure SAS Token",
            r"(?i)sig=[A-Za-z0-9%+/]{43,}={0,2}",
            Some(EntropyCharset::Base64),
        ),
        (
            "Azure APIM Subscription Key",
            r"(?i)(?:ocp-apim-subscription-key|subscription.?key)\s*[=:]\s*[a-f0-9]{32}",
            Some(EntropyCharset::Hex),
        ),
        // ── GitHub ──────────────────────────────────────────────────────────
        ("GitHub PAT (classic)", r"ghp_[a-zA-Z0-9]{36}", None),
        ("GitHub OAuth Token", r"gho_[a-zA-Z0-9]{36}", None),
        (
            "GitHub Fine-Grained PAT",
            r"github_pat_[a-zA-Z0-9_]{82}",
            None,
        ),
        ("GitHub App Token", r"(?:ghu|ghs)_[a-zA-Z0-9]{36}", None),
        ("GitHub Refresh Token", r"ghr_[a-zA-Z0-9]{36}", None),
        // ── API Keys: AI ────────────────────────────────────────────────────
        (
            "OpenAI API Key",
            r"sk-[a-zA-Z0-9]{20}T3BlbkFJ[a-zA-Z0-9]{20}",
            None,
        ),
        (
            "OpenAI Project Key",
            r"sk-proj-[a-zA-Z0-9\-_]{80,}",
            Some(EntropyCharset::Alphanumeric),
        ),
        (
            "Anthropic API Key",
            r"sk-ant-[a-zA-Z0-9\-_]{80,}",
            Some(EntropyCharset::Alphanumeric),
        ),
        // ── API Keys: Payments ───────────────────────────────────────────────
        ("Stripe Live Secret Key", r"sk_live_[a-zA-Z0-9]{24,}", None),
        ("Stripe Test Secret Key", r"sk_test_[a-zA-Z0-9]{24,}", None),
        (
            "Stripe Restricted Key",
            r"rk_(?:live|test)_[a-zA-Z0-9]{24,}",
            None,
        ),
        ("Stripe Publishable Key", r"pk_live_[a-zA-Z0-9]{24,}", None),
        ("Square Access Token", r"sq0atp-[A-Za-z0-9_-]{22}", None),
        ("Square OAuth Token", r"sq0csp-[A-Za-z0-9_-]{43}", None),
        (
            "Braintree Access Token",
            r"access_token\$production\$[a-z0-9]{16}\$[a-f0-9]{32}",
            None,
        ),
        // ── API Keys: Communication ──────────────────────────────────────────
        (
            "Slack Token",
            r"xox[bpors]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*",
            None,
        ),
        (
            "Slack Webhook",
            r"https://hooks\.slack\.com/services/T[0-9A-Z]{8,}/B[0-9A-Z]{8,}/[a-zA-Z0-9]{24}",
            None,
        ),
        (
            "Twilio API Key",
            r"SK[a-f0-9]{32}",
            Some(EntropyCharset::Hex),
        ),
        (
            "SendGrid API Key",
            r"SG\.[a-zA-Z0-9_\-]{22}\.[a-zA-Z0-9_\-]{43}",
            None,
        ),
        (
            "Mailgun API Key",
            r"key-[a-zA-Z0-9]{32}",
            Some(EntropyCharset::Alphanumeric),
        ),
        // Mailchimp keys always end in -us## (unique format, no entropy gate needed)
        ("Mailchimp API Key", r"[a-f0-9]{32}-us\d{1,2}", None),
        // ── API Keys: Observability ──────────────────────────────────────────
        (
            "Datadog API Key",
            r"(?i)(?:datadog|dd)[_-]?(?:api[_-]?key|token)\s*[=:]\s*[a-f0-9]{32}",
            Some(EntropyCharset::Hex),
        ),
        (
            "Datadog App Key",
            r"(?i)(?:datadog|dd)[_-]?(?:app[_-]?key|application[_-]?key)\s*[=:]\s*[a-f0-9]{40}",
            Some(EntropyCharset::Hex),
        ),
        // ── API Keys: E-commerce ─────────────────────────────────────────────
        ("Shopify Private App Token", r"shppa_[a-fA-F0-9]{32}", None),
        ("Shopify Shared Secret", r"shpss_[a-fA-F0-9]{32}", None),
        ("Shopify Access Token", r"shpat_[a-fA-F0-9]{32}", None),
        ("Shopify Custom App Token", r"shpca_[a-fA-F0-9]{32}", None),
        // ── API Keys: CRM ────────────────────────────────────────────────────
        (
            "HubSpot Private App Token",
            r"pat-(?:na1|eu1)-[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
            None,
        ),
        // ── Infrastructure: HashiCorp ────────────────────────────────────────
        (
            "HashiCorp Vault Service Token",
            r"hvs\.[A-Za-z0-9_-]{90,}",
            Some(EntropyCharset::Base64),
        ),
        (
            "HashiCorp Vault Batch Token",
            r"hvb\.[A-Za-z0-9_-]{90,}",
            Some(EntropyCharset::Base64),
        ),
        (
            "Terraform Cloud Token",
            r"[A-Za-z0-9]{14}\.atlasv1\.[A-Za-z0-9_-]{60,}",
            Some(EntropyCharset::Alphanumeric),
        ),
        // ── Infrastructure: CI/CD & Hosting ──────────────────────────────────
        (
            "Heroku API Key",
            r"(?i)heroku[_-]?(?:api[_-]?key|token)\s*[=:]\s*[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}",
            None,
        ),
        (
            "Render API Key",
            r"rnd_[A-Za-z0-9]{32,}",
            Some(EntropyCharset::Alphanumeric),
        ),
        // ── Package Registries ───────────────────────────────────────────────
        (
            "NPM Registry Auth Token (legacy)",
            r"(?i)//registry\.npmjs\.org/:_authToken=[a-zA-Z0-9\-_]+",
            None,
        ),
        ("NPM Access Token", r"npm_[A-Za-z0-9]{36}", None),
        (
            "PyPI API Token",
            r"pypi-AgEIcHlwaS5vcmc[A-Za-z0-9_-]{50,}",
            None,
        ),
        // ── Database / Infrastructure ────────────────────────────────────────
        (
            "Generic Connection String",
            r#"(?i)(?:mongodb|postgres|mysql|redis)://[^\s"']+:[^\s"']+@"#,
            None,
        ),
        (
            "Database URL",
            r#"(?i)database_url\s*[=:]\s*["']?(?:postgres|mysql|mongodb)://[^\s"']+"#,
            None,
        ),
        // ── Private Keys ─────────────────────────────────────────────────────
        ("RSA Private Key", r"-----BEGIN RSA PRIVATE KEY-----", None),
        ("DSA Private Key", r"-----BEGIN DSA PRIVATE KEY-----", None),
        ("EC Private Key", r"-----BEGIN EC PRIVATE KEY-----", None),
        (
            "OpenSSH Private Key",
            r"-----BEGIN OPENSSH PRIVATE KEY-----",
            None,
        ),
        (
            "PGP Private Key",
            r"-----BEGIN PGP PRIVATE KEY BLOCK-----",
            None,
        ),
        ("Generic Private Key", r"-----BEGIN PRIVATE KEY-----", None),
        (
            "Encrypted Private Key",
            r"-----BEGIN ENCRYPTED PRIVATE KEY-----",
            None,
        ),
        // ── Blockchain / Crypto ──────────────────────────────────────────────
        // WIF format private keys (Bitcoin / Neo N3 WIF)
        (
            "WIF Private Key",
            r"\b[5KLc][1-9A-HJ-NP-Za-km-z]{50,51}\b",
            None,
        ),
        (
            "Ethereum/EVM Private Key",
            r"(?i)(?:private[_-]?key|eth[_-]?key)\s*[=:]\s*(?:0x)?[a-f0-9]{64}",
            Some(EntropyCharset::Hex),
        ),
        // ── Generic Patterns ─────────────────────────────────────────────────
        (
            "JWT Token",
            r"eyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}",
            None,
        ),
        (
            "Bearer Token",
            r#"(?i)(?:bearer|authorization)\s*[=:]\s*["']?[a-zA-Z0-9\-_.~+/]{20,}["']?"#,
            Some(EntropyCharset::Alphanumeric),
        ),
        (
            "Generic API Key Assignment",
            r#"(?i)(?:api[_-]?key|apikey|api[_-]?secret)\s*[=:]\s*["'][a-zA-Z0-9\-_.]{16,}["']"#,
            Some(EntropyCharset::Alphanumeric),
        ),
        (
            "Generic Secret Assignment",
            r#"(?i)(?:secret|password|passwd|token)\s*[=:]\s*["'][^\s"']{8,}["']"#,
            Some(EntropyCharset::Alphanumeric),
        ),
        // ── High-Entropy Fallback ─────────────────────────────────────────────
        // Catches secrets that don't match any specific rule above.
        // Entropy is always verified; threshold is 3.5 (hex max is 4.0).
        (
            "High-Entropy Hex (≥40 chars)",
            r"\b[a-f0-9]{40,}\b",
            Some(EntropyCharset::Hex),
        ),
    ];

    specs
        .iter()
        .filter_map(|&(name, pat, entropy_gate)| {
            Regex::new(pat).ok().map(|pattern| Rule {
                name,
                pattern,
                entropy_gate,
            })
        })
        .collect()
}

// ── AhoCorasick Prefilter ─────────────────────────────────────────────────────

/// Known literal prefixes present in at least one rule pattern.
/// Lines containing none of these can skip the full regex battery.
/// Entropy-only lines still need scanning — handled separately.
const KNOWN_PREFIXES: &[&str] = &[
    "AKIA",
    "AIza",
    "ghp_",
    "gho_",
    "github_pat_",
    "ghu_",
    "ghs_",
    "ghr_",
    "sk-",
    "sk_live_",
    "sk_test_",
    "pk_live_",
    "rk_live_",
    "rk_test_",
    "xox",
    "https://hooks.slack.com",
    "SG.",
    "key-",
    "hvs.",
    "hvb.",
    "npm_",
    "pypi-",
    "sq0atp-",
    "sq0csp-",
    "shppa_",
    "shpss_",
    "shpat_",
    "shpca_",
    "pat-na1-",
    "pat-eu1-",
    "rnd_",
    "-----BEGIN",
    "access_token$production$",
    "amzn.mws.",
    "eyJ",
    "AccountKey",
    "DefaultEndpointsProtocol",
    "sig=",
    "atlasv1.",
    "us1",
    "us2",
    "us3",
];

fn build_prefilter() -> AhoCorasick {
    AhoCorasick::new(KNOWN_PREFIXES).expect("prefilter patterns are valid")
}

// ── False-Positive Reduction ──────────────────────────────────────────────────

/// Returns true if the value-part of an assignment is a variable reference or
/// obvious placeholder, meaning the line should not be reported as a finding.
fn is_variable_reference(line: &str) -> bool {
    // Extract value after first = or :
    let value = line.split_once(['=', ':']).map_or(line, |x| x.1);
    let v = value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`');

    v.starts_with("${")
        || v.starts_with("$(")
        || v.starts_with("#{")  // Ruby interpolation
        || v.starts_with('%')   // Python/Ruby template
        || v.starts_with('<')   // <placeholder>
        || v.starts_with("process.env.")
        || v.starts_with("os.environ")
        || v.starts_with("env(")
        || v.starts_with("vault(")
        || v.starts_with("secret(")
        || v == "null"
        || v == "undefined"
        || v == "None"
        || v == "false"
        || v == "true"
        || v.is_empty()
        // ALL_CAPS with underscores = environment variable name, not a value
        || (v.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
            && v.len() > 2)
}

const TEST_KEYWORDS: &[&str] = &[
    "example",
    "placeholder",
    "changeme",
    "replace_me",
    "insert_",
    "dummy",
    "fake",
    "mock",
    "stub",
    "fixture",
    "demo",
    "invalid",
    "xxx",
    "000000",
    "aaaaaa",
    "test-key",
    "sample",
];

/// Returns true if the line looks like it contains a test/example value
/// rather than a real credential.
fn has_test_marker(line: &str) -> bool {
    let lower = line.to_lowercase();
    TEST_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns true if the matched hex string is a well-known non-secret
/// (git SHA context, integrity hashes, checksums).
fn is_known_non_secret_hex(line: &str) -> bool {
    let lower = line.trim().to_lowercase();
    lower.starts_with("commit ")
        || lower.contains("sha256")
        || lower.contains("integrity")
        || lower.contains("checksum")
        || lower.contains("srchash")
        || lower.contains("filehash")
}

// ── Git File Collection ───────────────────────────────────────────────────────

fn get_git_files(root: &Path, staged: bool) -> Vec<PathBuf> {
    let args = if staged {
        vec!["diff", "--cached", "--name-only", "--diff-filter=ACM"]
    } else {
        vec!["ls-files", "--cached", "--others", "--exclude-standard"]
    };

    let output = Command::new("git").args(&args).current_dir(root).output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| root.join(l.trim()))
            .filter(|p| p.is_file())
            .collect(),
        _ => vec![],
    }
}

/// Collect (`commit_hash`, `addition_lines`) pairs from git log -p for history scanning.
fn get_history_diffs(root: &Path, since: Option<&str>) -> Vec<(String, Vec<String>)> {
    let mut cmd = Command::new("git");
    cmd.args(["log", "--all", "--format=%H", "-p", "--diff-filter=ACM"]);
    if let Some(s) = since {
        cmd.arg(format!("--since={s}"));
    }
    cmd.current_dir(root);

    let output = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    let mut current_hash = String::new();
    let mut additions: Vec<String> = Vec::new();

    for line in text.lines() {
        if line.len() == 40 && line.chars().all(|c| c.is_ascii_hexdigit()) {
            if !current_hash.is_empty() && !additions.is_empty() {
                result.push((current_hash.clone(), additions.clone()));
                additions.clear();
            }
            current_hash = line.to_string();
        } else if let Some(rest) = line.strip_prefix('+') {
            if !rest.starts_with("++") {
                additions.push(rest.to_string());
            }
        }
    }
    if !current_hash.is_empty() && !additions.is_empty() {
        result.push((current_hash, additions));
    }
    result
}

// ── Skip Logic ────────────────────────────────────────────────────────────────

fn should_skip(path: &Path, gitignore_excludes: &[String]) -> bool {
    const SKIP_EXT: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "mp3",
        "mp4", "wav", "avi", "mov", "pdf", "zip", "gz", "tar", "bz2", "7z", "rar", "exe", "dll",
        "so", "dylib", "o", "a", "wasm", "lock",
    ];

    if crate::gitignore::should_skip_path(path, gitignore_excludes) {
        return true;
    }

    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if SKIP_EXT.iter().any(|e| ext == *e) {
            return true;
        }
    }

    let path_str = path.to_string_lossy();
    if path_str.contains(".env.example") || path_str.contains(".env.sample") {
        return true;
    }

    false
}

fn is_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(512);
    content[..check_len].contains(&0)
}

// ── Allowlist ─────────────────────────────────────────────────────────────────

fn load_allowlist(path: &Path) -> Vec<String> {
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn is_allowlisted(finding: &Finding, allowlist: &[String]) -> bool {
    allowlist
        .iter()
        .any(|pattern| finding.content.contains(pattern) || finding.file.contains(pattern))
}

// ── Line Scanning ─────────────────────────────────────────────────────────────

/// Scan a single line against all rules. Appends any findings to `out`.
fn scan_line(
    line: &str,
    line_num: usize,
    rel_path: &str,
    rules: &[Rule],
    prefilter: &AhoCorasick,
    allowlist: &[String],
    out: &mut Vec<Finding>,
) {
    let trimmed = line.trim();

    // Skip comment lines that contain obvious example markers
    let is_comment = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("<!--")
        || trimmed.starts_with('*')
        || trimmed.starts_with("/*");
    if is_comment
        && (trimmed.contains("example")
            || trimmed.contains("EXAMPLE")
            || trimmed.contains("xxx")
            || trimmed.contains("your-")
            || trimmed.contains("placeholder"))
    {
        return;
    }

    // Fast path: skip lines with no known literal prefix AND no assignment context
    let has_known_prefix = prefilter.is_match(line);
    let has_assignment = line.contains('=') || line.contains(':');
    if !has_known_prefix && !has_assignment {
        return;
    }

    for rule in rules {
        let Some(mat) = rule.pattern.find(line) else {
            continue;
        };
        let matched = mat.as_str();

        // Entropy gate
        if let Some(charset) = rule.entropy_gate {
            if !passes_entropy_gate(matched, charset) {
                continue;
            }
        }

        // Hex-specific non-secret exclusions
        if rule.name.contains("Hex") && is_known_non_secret_hex(line) {
            continue;
        }

        // Variable reference / placeholder exclusions for assignment rules
        if (rule.name.contains("Assignment") || rule.name.contains("Generic"))
            && (is_variable_reference(line) || has_test_marker(line))
        {
            continue;
        }

        let finding = Finding {
            file: rel_path.to_string(),
            line: line_num + 1,
            rule: rule.name.to_string(),
            content: redact_line(line),
        };

        if !is_allowlisted(&finding, allowlist) {
            out.push(finding);
        }
    }
}

// ── Entry Point ───────────────────────────────────────────────────────────────

/// Run the secrets scan.
pub async fn run(args: SecretsArgs) -> Result<()> {
    let root = if args.root == std::path::Path::new(".") {
        crate::utils::find_project_root()
    } else {
        args.root
    };

    let rules = build_rules();
    let prefilter = build_prefilter();
    let allowlist_path = args
        .allowlist
        .unwrap_or_else(|| root.join(".secretsignore"));
    let allowlist = load_allowlist(&allowlist_path);
    let gitignore_excludes = crate::gitignore::parse_gitignore(&root);

    if args.verbose {
        println!("🔍 Scanning for secrets in: {}", root.display());
        if !allowlist.is_empty() {
            println!("📋 Loaded {} allowlist entries", allowlist.len());
        }
    }

    // ── History scanning ──────────────────────────────────────────────────────
    if args.history {
        println!("📜 Scanning git history{}...", {
            args.since
                .as_deref()
                .map(|s| format!(" since {s}"))
                .unwrap_or_default()
        });

        let diffs = get_history_diffs(&root, args.since.as_deref());
        println!("   {} commits to check", diffs.len());

        let all_findings: Mutex<Vec<(String, Finding)>> = Mutex::new(Vec::new());

        diffs.par_iter().for_each(|(hash, lines)| {
            let mut local: Vec<(String, Finding)> = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                let mut findings: Vec<Finding> = Vec::new();
                scan_line(
                    line,
                    i,
                    &format!("commit:{}", &hash[..8]),
                    &rules,
                    &prefilter,
                    &allowlist,
                    &mut findings,
                );
                for f in findings {
                    local.push((hash.clone(), f));
                }
            }
            if !local.is_empty() {
                all_findings.lock().unwrap().extend(local);
            }
        });

        let history_findings = all_findings.into_inner().unwrap();
        if history_findings.is_empty() {
            println!("✅ No secrets found in git history.");
        } else {
            println!("\n🚨 History findings:");
            for (hash, f) in &history_findings {
                println!("   {} L{}: [{}]", &hash[..8], f.line, f.rule);
                if args.verbose {
                    println!("      {}", f.content);
                }
            }
            println!(
                "\n⚠️  {} secret(s) found in git history. Rotate exposed credentials and consider a history rewrite.",
                history_findings.len()
            );
        }
        println!();
    }

    // ── Current tree scanning ─────────────────────────────────────────────────
    let files: Vec<PathBuf> = if args.staged {
        get_git_files(&root, true)
    } else if args.git_only {
        get_git_files(&root, false)
    } else {
        WalkDir::new(&root)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .collect()
    };

    let files: Vec<PathBuf> = files
        .into_iter()
        .filter(|p| !should_skip(p, &gitignore_excludes))
        .collect();

    if args.verbose {
        println!("📂 Scanning {} files...", files.len());
    }

    let all_findings: Mutex<Vec<Finding>> = Mutex::new(Vec::new());

    files.par_iter().for_each(|file_path| {
        let content_bytes = match fs::read(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        if is_binary(&content_bytes) {
            return;
        }

        let content = match std::str::from_utf8(&content_bytes) {
            Ok(s) => s,
            Err(_) => return,
        };

        let rel_path = file_path
            .strip_prefix(&root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let mut local: Vec<Finding> = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            scan_line(
                line, line_num, &rel_path, &rules, &prefilter, &allowlist, &mut local,
            );
        }

        if !local.is_empty() {
            all_findings.lock().unwrap().extend(local);
        }
    });

    let mut all_findings = all_findings.into_inner().unwrap();

    if all_findings.is_empty() {
        println!("✅ No secrets detected");
        return Ok(());
    }

    // Group by file, sorted
    all_findings.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    let mut current_file = String::new();
    for finding in &all_findings {
        if finding.file != current_file {
            current_file = finding.file.clone();
            println!("❌ {current_file}");
        }
        print!("   L{}: [{}]", finding.line, finding.rule);
        if args.verbose {
            print!("  {}", finding.content);
        }
        println!();
    }

    let file_count = {
        let mut seen: Vec<&str> = all_findings.iter().map(|f| f.file.as_str()).collect();
        seen.dedup();
        seen.len()
    };

    println!(
        "\n🚨 Found {} potential secret(s) across {} file(s)",
        all_findings.len(),
        file_count
    );
    println!("   Rotate any exposed credentials immediately.");
    println!("   Add false positives to .secretsignore");

    std::process::exit(1);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn redact_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 20 {
        return trimmed.to_string();
    }
    let visible_prefix = 10.min(trimmed.len() / 4);
    let visible_suffix = 6.min(trimmed.len() / 6);
    format!(
        "{}...REDACTED...{}",
        &trimmed[..visible_prefix],
        &trimmed[trimmed.len() - visible_suffix..]
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── charset_entropy ───────────────────────────────────────────────────────

    #[test]
    fn entropy_single_char_is_zero() {
        // Repeating same char → zero entropy
        assert_eq!(
            charset_entropy("aaaaaaaaaa", EntropyCharset::Alphanumeric),
            0.0
        );
    }

    #[test]
    fn entropy_empty_string_is_zero() {
        assert_eq!(charset_entropy("", EntropyCharset::Hex), 0.0);
    }

    #[test]
    fn entropy_no_matching_chars_is_zero() {
        // No hex chars in string of special chars
        assert_eq!(charset_entropy("!@#$%^&*()", EntropyCharset::Hex), 0.0);
    }

    #[test]
    fn entropy_uniform_hex_is_high() {
        // All 16 hex symbols used equally → max entropy ~4.0
        let s = "0123456789abcdef";
        let e = charset_entropy(s, EntropyCharset::Hex);
        assert!(e > 3.9, "Expected entropy > 3.9, got {e}");
    }

    #[test]
    fn entropy_uniform_base64_is_high() {
        let s = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let e = charset_entropy(s, EntropyCharset::Base64);
        assert!(e > 5.0, "Expected entropy > 5.0, got {e}");
    }

    #[test]
    fn entropy_hex_ignores_non_hex() {
        // 'xyz' chars are not hex, only 'aabb' counted
        let e = charset_entropy("aabbxyz", EntropyCharset::Hex);
        let e_pure = charset_entropy("aabb", EntropyCharset::Hex);
        assert!((e - e_pure).abs() < f64::EPSILON);
    }

    // ── passes_entropy_gate ───────────────────────────────────────────────────

    #[test]
    fn entropy_gate_too_short_fails() {
        // Hex min_len is 40, this is only 10
        assert!(!passes_entropy_gate("0123456789", EntropyCharset::Hex));
    }

    #[test]
    fn entropy_gate_low_entropy_fails() {
        // Long but all same char — entropy = 0
        let s = "a".repeat(50);
        assert!(!passes_entropy_gate(&s, EntropyCharset::Hex));
    }

    #[test]
    fn entropy_gate_high_entropy_passes() {
        // 40+ chars, repeating full hex alphabet → high entropy
        let s = "0123456789abcdef".repeat(3); // 48 chars
        assert!(passes_entropy_gate(&s, EntropyCharset::Hex));
    }

    // ── is_variable_reference ────────────────────────────────────────────────

    #[test]
    fn variable_ref_shell_expansion() {
        assert!(is_variable_reference("API_KEY=${SECRET_VALUE}"));
    }

    #[test]
    fn variable_ref_command_substitution() {
        assert!(is_variable_reference("TOKEN=$(vault read secret/key)"));
    }

    #[test]
    fn variable_ref_process_env() {
        assert!(is_variable_reference("key = process.env.API_KEY"));
    }

    #[test]
    fn variable_ref_null_values() {
        assert!(is_variable_reference("secret = null"));
        assert!(is_variable_reference("secret = undefined"));
        assert!(is_variable_reference("secret = None"));
    }

    #[test]
    fn variable_ref_boolean_values() {
        assert!(is_variable_reference("debug = true"));
        assert!(is_variable_reference("debug = false"));
    }

    #[test]
    fn variable_ref_empty_value() {
        assert!(is_variable_reference("key = "));
        assert!(is_variable_reference("key = \"\""));
    }

    #[test]
    fn variable_ref_env_var_name() {
        // ALL_CAPS_ENV is an environment variable name, not a value
        assert!(is_variable_reference("key = MY_SECRET_KEY"));
    }

    #[test]
    fn variable_ref_ruby_interpolation() {
        assert!(is_variable_reference("secret = #{ENV['KEY']}"));
    }

    #[test]
    fn variable_ref_vault_function() {
        assert!(is_variable_reference("token: vault(secret/data/key)"));
    }

    #[test]
    fn variable_ref_real_secret_is_not_ref() {
        assert!(!is_variable_reference(
            "api_key = sk_live_<YOUR-STRIPE-KEY>"
        ));
    }

    // ── has_test_marker ──────────────────────────────────────────────────────

    #[test]
    fn test_marker_example() {
        assert!(has_test_marker("api_key = 'example_key_12345'"));
    }

    #[test]
    fn test_marker_placeholder() {
        assert!(has_test_marker("token: placeholder-token"));
    }

    #[test]
    fn test_marker_changeme() {
        assert!(has_test_marker("password = changeme"));
    }

    #[test]
    fn test_marker_dummy() {
        assert!(has_test_marker("SECRET=dummy_secret_value"));
    }

    #[test]
    fn test_marker_real_secret_no_match() {
        assert!(!has_test_marker("sk_live_<YOUR-STRIPE-KEY>"));
    }

    #[test]
    fn test_marker_case_insensitive() {
        assert!(has_test_marker("API_KEY=EXAMPLE_VALUE"));
    }

    // ── is_known_non_secret_hex ──────────────────────────────────────────────

    #[test]
    fn non_secret_git_commit() {
        assert!(is_known_non_secret_hex(
            "commit abc123def456789012345678901234567890abcd"
        ));
    }

    #[test]
    fn non_secret_sha256() {
        assert!(is_known_non_secret_hex("sha256-abc123def456"));
    }

    #[test]
    fn non_secret_integrity() {
        assert!(is_known_non_secret_hex("integrity: sha384-abc123"));
    }

    #[test]
    fn non_secret_checksum() {
        assert!(is_known_non_secret_hex("checksum = abc123def456"));
    }

    #[test]
    fn non_secret_plain_hex_is_secret() {
        assert!(!is_known_non_secret_hex(
            "abc123def456789012345678901234567890abcd"
        ));
    }

    // ── is_binary ────────────────────────────────────────────────────────────

    #[test]
    fn binary_null_byte() {
        assert!(is_binary(b"\x00ELF binary content"));
    }

    #[test]
    fn binary_text_is_not_binary() {
        assert!(!is_binary(b"fn main() { println!(\"hello\"); }"));
    }

    #[test]
    fn test_charset_entropy() {
        // Use a small epsilon for float comparisons to satisfy clippy
        let epsilon = 1e-10;

        assert!(
            (charset_entropy("aaaaaaaaaa", EntropyCharset::Alphanumeric) - 0.0).abs() < epsilon
        );
        assert!(
            (charset_entropy("abcde", EntropyCharset::Hex) - 2.321928094887362).abs() < epsilon
        );
        assert!((charset_entropy("", EntropyCharset::Hex) - 0.0).abs() < epsilon);

        // Mix of valid and invalid chars
        assert!((charset_entropy("!@#$%^&*()", EntropyCharset::Hex) - 0.0).abs() < epsilon);
    }

    #[test]
    fn test_load_allowlist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join(".secretsignore");
        let mut f = std::fs::File::create(&path).expect("failed to create temp file");
        writeln!(f, "# This is a comment").expect("failed to write");
        writeln!(f).expect("failed to write");
        writeln!(f, "some-pattern").expect("failed to write");
        writeln!(f, "  another-pattern  ").expect("failed to write");
        writeln!(f, "# another comment").expect("failed to write");

        let allowlist = load_allowlist(&path);
        assert_eq!(allowlist.len(), 2);
        assert_eq!(allowlist[0], "some-pattern");
        assert_eq!(allowlist[1], "another-pattern");
    }

    // ── is_allowlisted ───────────────────────────────────────────────────────

    #[test]
    fn allowlisted_content_match() {
        let finding = Finding {
            file: "src/config.rs".to_string(),
            line: 10,
            rule: "test".to_string(),
            content: "api_key = AKIA1234567890ABCDEF".to_string(),
        };
        let allowlist = vec!["AKIA1234567890ABCDEF".to_string()];
        assert!(is_allowlisted(&finding, &allowlist));
    }

    #[test]
    fn allowlisted_file_match() {
        let finding = Finding {
            file: "tests/fixtures/secrets.txt".to_string(),
            line: 1,
            rule: "test".to_string(),
            content: "secret here".to_string(),
        };
        let allowlist = vec!["tests/fixtures".to_string()];
        assert!(is_allowlisted(&finding, &allowlist));
    }

    #[test]
    fn allowlisted_no_match() {
        let finding = Finding {
            file: "src/main.rs".to_string(),
            line: 5,
            rule: "test".to_string(),
            content: "ghp_abcdef1234567890abcdef1234567890abcd".to_string(),
        };
        let allowlist = vec!["unrelated-pattern".to_string()];
        assert!(!is_allowlisted(&finding, &allowlist));
    }

    // ── redact_line ──────────────────────────────────────────────────────────

    #[test]
    fn redact_short_line_unchanged() {
        assert_eq!(redact_line("short"), "short");
    }

    #[test]
    fn redact_long_line_is_redacted() {
        let line = "api_key = sk_live_<YOUR-STRIPE-KEY>_very_long_secret";
        let result = redact_line(line);
        assert!(result.contains("REDACTED"));
        assert!(result.len() < line.len());
    }
}
