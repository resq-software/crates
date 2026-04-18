// Copyright 2026 ResQ
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `resq format` and its underlying per-language
//! functions. Each test creates a misformatted fixture in a tempdir and
//! verifies the formatter fixes it (and `--check` detects the issue).
//!
//! Tests skip cleanly when the required formatter tool isn't on PATH,
//! because CI environments won't always have every toolchain installed.

#![allow(missing_docs)]

use std::process::Command;
use tempfile::TempDir;

const RESQ_BIN: &str = env!("CARGO_BIN_EXE_resq");

fn has(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .unwrap();
    tmp
}

#[test]
fn resq_format_check_on_clean_repo_with_no_languages_exits_zero() {
    // No language-specific files → every formatter is Skipped → exit 0.
    let tmp = init_repo();
    let out = Command::new(RESQ_BIN)
        .args(["format", "--check"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(
        out.status.success(),
        "expected exit 0 for empty repo; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn resq_format_rejects_unknown_language() {
    let tmp = init_repo();
    let out = Command::new(RESQ_BIN)
        .args(["format", "--language", "haskell"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    // clap emits its own error for the invalid choice.
    assert!(
        stderr.contains("invalid value") || stderr.contains("haskell"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn resq_format_rust_check_flags_unformatted_code() {
    if !has("cargo") {
        eprintln!("skip: cargo not on PATH");
        return;
    }
    let tmp = init_repo();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname=\"x\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    // Poorly-formatted main.rs — cargo fmt will rewrite to `fn main() {}`.
    std::fs::write(tmp.path().join("src/main.rs"), "fn   main(  ){\n}\n").unwrap();

    let check = Command::new(RESQ_BIN)
        .args(["format", "--language", "rust", "--check"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(
        !check.status.success(),
        "expected --check to fail on misformatted main.rs; stdout={}",
        String::from_utf8_lossy(&check.stdout)
    );

    // Now format and re-check → should pass.
    let fmt = Command::new(RESQ_BIN)
        .args(["format", "--language", "rust"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(
        fmt.status.success(),
        "format failed: {}",
        String::from_utf8_lossy(&fmt.stdout)
    );

    let recheck = Command::new(RESQ_BIN)
        .args(["format", "--language", "rust", "--check"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(recheck.status.success());
}

#[test]
fn resq_format_skips_when_no_files() {
    // Specify Python but put no .py file in the repo.
    let tmp = init_repo();
    let out = Command::new(RESQ_BIN)
        .args(["format", "--language", "python"])
        .current_dir(tmp.path())
        .output()
        .expect("run resq");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("skipped") || stdout.contains("⏭"),
        "expected skip output, got: {stdout}"
    );
}
