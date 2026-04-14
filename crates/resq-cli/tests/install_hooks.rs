// Copyright 2026 ResQ
// SPDX-License-Identifier: Apache-2.0
//
// Integration tests for `resq dev install-hooks`, `resq dev scaffold-local-hook`,
// and `resq hooks {doctor, update, status}`. Invokes the built `resq` binary
// against a fresh tempdir git repo to verify end-to-end behavior.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const RESQ_BIN: &str = env!("CARGO_BIN_EXE_resq");

const CANONICAL_HOOKS: &[&str] = &[
    "pre-commit",
    "commit-msg",
    "prepare-commit-msg",
    "pre-push",
    "post-checkout",
    "post-merge",
];

/// Initialize an empty git repo with one root commit so HEAD is valid.
fn init_repo() -> TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    git(tmp.path(), &["init", "-q"]).status().unwrap();
    git(
        tmp.path(),
        &[
            "-c",
            "user.email=t@t.io",
            "-c",
            "user.name=t",
            "commit",
            "--allow-empty",
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

fn read_hooks_path(dir: &Path) -> String {
    let out = git(dir, &["config", "--get", "core.hooksPath"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn install_hooks_scaffolds_into_empty_repo() {
    let tmp = init_repo();
    let out = resq(tmp.path(), &["dev", "install-hooks"]);
    assert!(
        out.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    for hook in CANONICAL_HOOKS {
        let path = tmp.path().join(".git-hooks").join(hook);
        assert!(path.exists(), "missing hook: {hook}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "{hook} mode = {mode:o}");
        }
    }

    assert_eq!(read_hooks_path(tmp.path()), ".git-hooks");
}

#[test]
fn install_hooks_does_not_overwrite_existing_files() {
    let tmp = init_repo();
    std::fs::create_dir(tmp.path().join(".git-hooks")).unwrap();
    let custom = "#!/bin/sh\necho custom\n";
    let pre_commit = tmp.path().join(".git-hooks").join("pre-commit");
    std::fs::write(&pre_commit, custom).unwrap();

    let out = resq(tmp.path(), &["dev", "install-hooks"]);
    assert!(out.status.success());

    let after = std::fs::read_to_string(&pre_commit).unwrap();
    assert_eq!(after, custom, "existing pre-commit was overwritten");

    // Other 5 hooks should still have been scaffolded.
    for hook in CANONICAL_HOOKS.iter().filter(|h| **h != "pre-commit") {
        assert!(tmp.path().join(".git-hooks").join(hook).exists(), "{hook}");
    }
}

#[test]
fn doctor_reports_clean_after_install() {
    let tmp = init_repo();
    resq(tmp.path(), &["dev", "install-hooks"]);
    let out = resq(tmp.path(), &["hooks", "doctor"]);
    assert!(
        out.status.success(),
        "doctor failed on clean install: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("All hooks healthy"), "stdout = {stdout}");
}

#[test]
fn doctor_detects_drift() {
    let tmp = init_repo();
    resq(tmp.path(), &["dev", "install-hooks"]);
    std::fs::write(tmp.path().join(".git-hooks/pre-commit"), "broken\n").unwrap();

    let out = resq(tmp.path(), &["hooks", "doctor"]);
    assert!(
        !out.status.success(),
        "doctor passed despite drift: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("drifts from embedded canonical"),
        "stdout = {stdout}"
    );
}

#[test]
fn update_restores_canonical_and_preserves_local() {
    let tmp = init_repo();
    resq(tmp.path(), &["dev", "install-hooks"]);

    let local_path = tmp.path().join(".git-hooks/local-pre-push");
    let local_body = "#!/bin/sh\necho local-marker\n";
    std::fs::write(&local_path, local_body).unwrap();

    // Corrupt one canonical hook.
    std::fs::write(tmp.path().join(".git-hooks/pre-commit"), "broken\n").unwrap();

    let out = resq(tmp.path(), &["hooks", "update"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Canonical content restored — pre-commit should reference resq pre-commit.
    let pc = std::fs::read_to_string(tmp.path().join(".git-hooks/pre-commit")).unwrap();
    assert!(
        pc.contains("resq pre-commit"),
        "pre-commit not restored from canonical: {pc}"
    );

    // local-pre-push must be untouched.
    let after_local = std::fs::read_to_string(&local_path).unwrap();
    assert_eq!(after_local, local_body, "local-pre-push was modified");
}

#[test]
fn status_reflects_drift() {
    let tmp = init_repo();
    resq(tmp.path(), &["dev", "install-hooks"]);

    let clean = resq(tmp.path(), &["hooks", "status"]);
    let s = String::from_utf8_lossy(&clean.stdout);
    assert!(s.contains("installed=clean"), "{s}");

    std::fs::write(tmp.path().join(".git-hooks/pre-commit"), "broken").unwrap();
    let drifted = resq(tmp.path(), &["hooks", "status"]);
    let s = String::from_utf8_lossy(&drifted.stdout);
    assert!(s.contains("installed=drift"), "{s}");
}

#[test]
fn scaffold_local_hook_writes_kind_template_and_refuses_overwrite() {
    let tmp = init_repo();
    // Touch a marker so --kind auto resolves to rust.
    std::fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

    let out = resq(tmp.path(), &["dev", "scaffold-local-hook"]);
    assert!(
        out.status.success(),
        "scaffold failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let local = tmp.path().join(".git-hooks/local-pre-push");
    let content = std::fs::read_to_string(&local).unwrap();
    assert!(
        content.contains("cargo check"),
        "rust template not written: {content}"
    );

    // Re-run without --force should refuse.
    let out = resq(tmp.path(), &["dev", "scaffold-local-hook"]);
    assert!(!out.status.success(), "scaffold succeeded without --force");

    // --force overwrites with a different kind.
    let out = resq(
        tmp.path(),
        &["dev", "scaffold-local-hook", "--kind", "python", "--force"],
    );
    assert!(out.status.success());
    let new = std::fs::read_to_string(&local).unwrap();
    assert!(new.contains("ruff"), "python template not written: {new}");
}

#[test]
fn scaffold_local_hook_auto_fails_on_unknown_kind() {
    let tmp = init_repo();
    let out = resq(tmp.path(), &["dev", "scaffold-local-hook"]);
    assert!(
        !out.status.success(),
        "scaffold succeeded in repo with no kind markers"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("auto-detect") || stderr.contains("kind"),
        "stderr = {stderr}"
    );
}

#[test]
fn scaffold_local_hook_rejects_unknown_explicit_kind() {
    let tmp = init_repo();
    let out = resq(
        tmp.path(),
        &["dev", "scaffold-local-hook", "--kind", "haskell"],
    );
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Unknown --kind"), "stderr = {stderr}");
}
