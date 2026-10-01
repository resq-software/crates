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

/// A third-party licence must never be rewritten, and `--force` is not enough.
///
/// Before the `--relicense` gate, running with NO flags turned
/// `Copyright (c) 2019 Some Third Party ... MIT License` into an Apache-2.0
/// header attributed to `--author`, destroying the third party's notice. The
/// trigger was the AUTHOR mismatch, not the licence: MIT was recognised so the
/// licence check passed, but any mismatch rewrote, and the rebuild used
/// `--license`.
#[test]
fn a_third_party_licence_is_never_rewritten_without_relicense() {
    let tmp = init_repo();
    let mit = "# Copyright (c) 2019 Some Third Party\n#\n\
               # Permission is hereby granted, free of charge ... MIT License\n\
               def g(): pass\n";
    let path = tmp.path().join("vendored.py");

    for args in [
        vec!["copyright", "vendored.py"],
        vec!["copyright", "--force", "vendored.py"],
    ] {
        std::fs::write(&path, mit).expect("write fixture");
        let out = resq(tmp.path(), &args);
        assert!(out.status.success(), "resq copyright failed for {args:?}");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read back"),
            mit,
            "third-party header was rewritten by {args:?}"
        );
    }

    // The deliberate act still works.
    std::fs::write(&path, mit).expect("write fixture");
    let out = resq(tmp.path(), &["copyright", "--relicense", "vendored.py"]);
    assert!(out.status.success());
    let after = std::fs::read_to_string(&path).expect("read back");
    assert!(
        after.contains("Apache License"),
        "--relicense should have replaced the header, got:\n{after}"
    );
}

/// An existing header whose licence cannot be classified is left alone.
///
/// A proprietary notice matches the copyright regex but no licence
/// fingerprint, so `detect_header_license` returns `None`. That used to read
/// as "no licence to conflict with", and the rebuild fell back to
/// `--license apache-2.0`, relicensing proprietary files by default.
#[test]
fn an_unrecognised_header_is_left_alone() {
    let tmp = init_repo();
    let proprietary = "# Copyright (c) 2026 ResQ. All Rights Reserved.\n#\n\
                       # proprietary information. No license, express or implied.\n\
                       def f(): pass\n";
    let path = tmp.path().join("proprietary.py");
    std::fs::write(&path, proprietary).expect("write fixture");

    let out = resq(tmp.path(), &["copyright", "proprietary.py"]);
    assert!(out.status.success());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read back"),
        proprietary,
        "an unclassifiable header was overwritten"
    );
}

/// The legitimate case must keep working: same licence, stale author.
#[test]
fn author_is_still_normalised_when_the_licence_matches() {
    let tmp = init_repo();
    let ours = "# Copyright 2026 ResQ\n#\n\
                # Licensed under the Apache License, Version 2.0 (the \"License\");\n\
                def h(): pass\n";
    let path = tmp.path().join("ours.py");
    std::fs::write(&path, ours).expect("write fixture");

    let out = resq(tmp.path(), &["copyright", "ours.py"]);
    assert!(out.status.success());
    let after = std::fs::read_to_string(&path).expect("read back");
    assert!(
        after.contains("ResQ Systems, Inc."),
        "author normalisation regressed, got:\n{after}"
    );
    assert!(
        after.contains("Apache License"),
        "licence must be preserved, got:\n{after}"
    );
}

/// A header deep inside a long file must be found, not duplicated.
///
/// `has_header` once looked only at the first 20 lines. A `CHANGELOG.md` keeps
/// its header under the `# Changelog` title and every release inserts a
/// section above it, so after two releases the header sat past line 20, became
/// invisible, and a second one was prepended — then once per release after
/// that. Each rewrite is a change inside `crates/<pkg>/`, which release-plz
/// reads as releasable, so the duplication drove a release loop.
///
/// Line 40 of a ~190-line file is well past any plausible fixed window.
#[test]
fn header_deep_in_a_long_changelog_is_not_duplicated() {
    use std::fmt::Write as _;

    let tmp = init_repo();
    let mut content = String::from("# Changelog\n\n");
    for i in 0..37 {
        writeln!(content, "- entry {i}").expect("write to String");
    }
    content.push_str(
        "<!--\n  Copyright 2026 ResQ Systems, Inc.\n\n  \
         Licensed under the Apache License, Version 2.0 (the \"License\");\n-->\n\n",
    );
    for i in 0..150 {
        writeln!(content, "- older entry {i}").expect("write to String");
    }

    let path = tmp.path().join("CHANGELOG.md");
    std::fs::write(&path, &content).expect("write changelog");
    assert_eq!(
        content.matches("Copyright").count(),
        1,
        "fixture should start with exactly one header"
    );

    let out = resq(tmp.path(), &["copyright", "CHANGELOG.md"]);
    assert!(out.status.success(), "resq copyright failed");

    let after = std::fs::read_to_string(&path).expect("read back");
    assert_eq!(
        after.matches("Copyright").count(),
        1,
        "header at line 40 went unrecognised, so a second was prepended:\n{}",
        after.lines().take(8).collect::<Vec<_>>().join("\n")
    );
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
    assert!(resq(tmp.path(), &["copyright", "wanted.rs"])
        .status
        .success());

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

#[test]
fn a_force_added_ignored_file_is_still_stamped() {
    // `git add -f` makes "tracked and ignored" reachable, so a staged path can
    // match .gitignore. Dropping it here would let the pre-commit step report
    // success on a file it never gave a header.
    let tmp = init_repo();
    std::fs::write(tmp.path().join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(tmp.path().join("ignored.rs"), "fn ignored() {}\n").unwrap();
    git(tmp.path(), &["add", "-f", "ignored.rs", ".gitignore"])
        .status()
        .unwrap();

    let out = resq(tmp.path(), &["copyright", "--", "ignored.rs"]);
    assert!(out.status.success(), "copyright failed: {out:?}");
    assert!(
        has_header(tmp.path(), "ignored.rs"),
        "a named path outranks the ignore rules that discovery would apply"
    );
}

#[test]
fn discovery_still_honours_gitignore() {
    // The counterpart to the test above: only naming a file overrides the ignore
    // rules, so a sweep must leave an ignored file alone.
    let tmp = init_repo();
    std::fs::write(tmp.path().join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(tmp.path().join("ignored.rs"), "fn ignored() {}\n").unwrap();
    git(tmp.path(), &["add", "-f", "ignored.rs", ".gitignore"])
        .status()
        .unwrap();

    assert!(resq(tmp.path(), &["copyright"]).status.success());
    assert!(
        !has_header(tmp.path(), "ignored.rs"),
        "without an explicit path the ignore rules still apply"
    );
}

#[test]
fn a_leading_dash_filename_is_treated_as_a_path() {
    // Without the `--` terminator the CLI reads `-weird.rs` as a bundle of short
    // flags. The same hazard is why `restage` passes `--` to `git add`, where a
    // file named `-A` would otherwise stage the entire worktree.
    let tmp = init_repo();
    std::fs::write(tmp.path().join("-weird.rs"), "fn weird() {}\n").unwrap();
    git(tmp.path(), &["add", "--", "-weird.rs"])
        .status()
        .unwrap();

    let out = resq(tmp.path(), &["copyright", "--", "-weird.rs"]);
    assert!(out.status.success(), "copyright failed: {out:?}");
    assert!(has_header(tmp.path(), "-weird.rs"));
    assert!(
        !has_header(tmp.path(), "bystander.rs"),
        "the odd filename must not have widened the scope"
    );
}
