// Copyright 2026 ResQ
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for how `resq copyright` chooses which files to touch.
//!
//! The scoping is what keeps the pre-commit hook honest. Without explicit paths
//! the command walks every tracked file in the repository, which is right for a
//! one-off sweep and wrong inside a hook, where rewriting outside the commit
//! being made shows up as unrelated files in someone's pull request.

#![allow(missing_docs)]

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const RESQ_BIN: &str = env!("CARGO_BIN_EXE_resq");

/// Initialize a git repo holding two header-less tracked sources.
fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    git(tmp.path(), &["init", "-q"]).status().unwrap();
    std::fs::write(tmp.path().join("wanted.rs"), "fn wanted() {}\n").unwrap();
    std::fs::write(tmp.path().join("bystander.rs"), "fn bystander() {}\n").unwrap();
    git(tmp.path(), &["add", "-A"]).status().unwrap();
    git(
        tmp.path(),
        &[
            "-c",
            "user.email=t@t.io",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    )
    .status()
    .unwrap();
    tmp
}

fn git(dir: &Path, args: &[&str]) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args);
    c
}

fn resq(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(RESQ_BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("resq invocation")
}

fn has_header(dir: &Path, name: &str) -> bool {
    std::fs::read_to_string(dir.join(name))
        .expect("read back")
        .contains("Copyright")
}

#[test]
fn named_paths_leave_every_other_file_alone() {
    let tmp = init_repo();
    let out = resq(tmp.path(), &["copyright", "wanted.rs"]);
    assert!(out.status.success(), "copyright failed: {out:?}");

    assert!(
        has_header(tmp.path(), "wanted.rs"),
        "the named file should have been stamped"
    );
    assert!(
        !has_header(tmp.path(), "bystander.rs"),
        "a file that was not named must not be rewritten — this is the whole point \
         of the pre-commit hook passing its staged set"
    );
}

#[test]
fn no_paths_still_sweeps_the_repository() {
    // `resq scan copyright` relies on this, so scoping stays opt-in.
    let tmp = init_repo();
    let out = resq(tmp.path(), &["copyright"]);
    assert!(out.status.success(), "copyright failed: {out:?}");

    assert!(has_header(tmp.path(), "wanted.rs"));
    assert!(has_header(tmp.path(), "bystander.rs"));
}

#[test]
fn check_reports_only_the_named_paths() {
    let tmp = init_repo();
    assert!(
        resq(tmp.path(), &["copyright", "wanted.rs"])
            .status
            .success()
    );

    // `wanted.rs` now has a header and `bystander.rs` does not. A check scoped to
    // the former has to pass despite the latter, or a hook that stamps only what
    // it is committing would fail on every pre-existing gap in the repository.
    assert!(
        resq(tmp.path(), &["copyright", "--check", "wanted.rs"])
            .status
            .success(),
        "a scoped check must ignore files it was not asked about"
    );
    assert!(
        !resq(tmp.path(), &["copyright", "--check", "bystander.rs"])
            .status
            .success(),
        "a scoped check must still fail for a named file that is missing a header"
    );
}

#[test]
fn named_paths_take_precedence_over_globs() {
    let tmp = init_repo();
    let out = resq(
        tmp.path(),
        &["copyright", "--glob", "bystander.rs", "wanted.rs"],
    );
    assert!(out.status.success(), "copyright failed: {out:?}");

    assert!(has_header(tmp.path(), "wanted.rs"));
    assert!(
        !has_header(tmp.path(), "bystander.rs"),
        "explicit paths are the most specific instruction and must win"
    );
}
