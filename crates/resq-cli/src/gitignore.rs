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

//! Gitignore matching for the file-mutating commands (`copyright`, `secrets`).
//!
//! Backed by the `ignore` crate's [`Gitignore`] matcher, which implements real
//! gitignore semantics — wildcards (`*.rs`), negations (`!keep`), anchoring
//! (`/build` vs `build`) and directory rules. The previous hand-rolled parser
//! dropped every wildcard and negation and matched bare names by substring, so
//! `copyright` would rewrite gitignored `generated/*.rs`, over-skip any path
//! merely *containing* an excluded name (e.g. `rebuild.rs` for `build`), and
//! ignore `!keep` re-includes entirely.

use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Fallback exclude dirs applied when the repo has no `.gitignore`.
const FALLBACK_EXCLUDES: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    ".turbo",
    "coverage",
];

/// A compiled gitignore matcher rooted at a project directory.
pub struct Matcher {
    gitignore: Gitignore,
    root: PathBuf,
}

/// Build a [`Matcher`] for `root`.
///
/// Loads `root/.gitignore` when present; otherwise seeds the matcher with
/// [`FALLBACK_EXCLUDES`]. `.git/` and `node_modules/` are always added as
/// safety nets so they are skipped even if a `.gitignore` omits them.
#[must_use]
pub fn load(root: &Path) -> Matcher {
    let mut builder = GitignoreBuilder::new(root);
    let gitignore_path = root.join(".gitignore");

    if gitignore_path.exists() {
        // `add` returns `Some(err)` on failure; a malformed .gitignore is
        // tolerated (git itself is lenient), so we ignore the error.
        let _ = builder.add(&gitignore_path);
    } else {
        for dir in FALLBACK_EXCLUDES {
            // Trailing slash → match the directory (and its contents) at any depth.
            let _ = builder.add_line(None, &format!("{dir}/"));
        }
    }

    // Safety nets, always present regardless of the .gitignore contents.
    let _ = builder.add_line(None, ".git/");
    let _ = builder.add_line(None, "node_modules/");

    Matcher {
        gitignore: builder.build().unwrap_or_else(|_| Gitignore::empty()),
        root: root.to_path_buf(),
    }
}

impl Matcher {
    /// Returns `true` if `path` is ignored, checking the path itself and every
    /// parent directory (so a file inside an ignored directory is skipped) and
    /// honoring negations. `is_dir` should reflect whether `path` is a directory.
    #[must_use]
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        // Anchor relative paths to the matcher root so the query shares the
        // builder's basis (matched_path_or_any_parents strips the root prefix).
        // Absolute paths are used as-is; we deliberately do NOT canonicalize,
        // since that follows symlinks — gitignore matches by name, not target.
        let anchored = if path.is_absolute() {
            std::borrow::Cow::Borrowed(path)
        } else {
            std::borrow::Cow::Owned(self.root.join(path))
        };
        self.gitignore
            .matched_path_or_any_parents(anchored.as_ref(), is_dir)
            .is_ignore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_gitignore(dir: &Path, contents: &str) {
        fs::write(dir.join(".gitignore"), contents).unwrap();
    }

    #[test]
    fn wildcard_patterns_are_honored() {
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "*.log\ngenerated/*.rs\n");
        let m = load(dir.path());

        assert!(m.is_ignored(&dir.path().join("app.log"), false));
        assert!(m.is_ignored(&dir.path().join("generated/api.rs"), false));
        // A real source file is not ignored.
        assert!(!m.is_ignored(&dir.path().join("src/main.rs"), false));
    }

    #[test]
    fn negations_re_include_files() {
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "*.log\n!keep.log\n");
        let m = load(dir.path());

        assert!(m.is_ignored(&dir.path().join("noise.log"), false));
        assert!(
            !m.is_ignored(&dir.path().join("keep.log"), false),
            "a `!keep.log` negation must re-include the file"
        );
    }

    #[test]
    fn substring_names_are_not_over_matched() {
        // The old parser skipped any path *containing* an excluded name; a
        // `build/` rule must NOT skip `rebuild.rs` or `src/buildings/x.rs`.
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "build/\n");
        let m = load(dir.path());

        assert!(m.is_ignored(&dir.path().join("build/out.o"), false));
        assert!(!m.is_ignored(&dir.path().join("rebuild.rs"), false));
        assert!(!m.is_ignored(&dir.path().join("src/buildings/plan.rs"), false));
    }

    #[test]
    fn files_inside_ignored_dirs_are_skipped() {
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "target/\n");
        let m = load(dir.path());
        assert!(m.is_ignored(&dir.path().join("target/debug/app"), false));
    }

    #[test]
    fn relative_paths_are_anchored_to_root() {
        // A caller passing a path relative to root must still match; is_ignored
        // anchors it to the matcher root before querying.
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "target/\n");
        let m = load(dir.path());
        assert!(m.is_ignored(Path::new("target/debug/app"), false));
        assert!(!m.is_ignored(Path::new("src/main.rs"), false));
    }

    #[test]
    fn git_dir_is_always_ignored() {
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "# nothing here\n");
        let m = load(dir.path());
        assert!(m.is_ignored(&dir.path().join(".git/config"), false));
    }

    #[test]
    fn missing_gitignore_uses_fallbacks() {
        let dir = tempdir().unwrap();
        let m = load(dir.path());
        assert!(m.is_ignored(&dir.path().join("node_modules/react/index.js"), false));
        assert!(m.is_ignored(&dir.path().join("target/debug/app"), false));
        assert!(m.is_ignored(&dir.path().join("dist/bundle.js"), false));
        assert!(!m.is_ignored(&dir.path().join("src/lib.rs"), false));
    }

    #[test]
    fn plain_source_file_is_not_ignored() {
        let dir = tempdir().unwrap();
        write_gitignore(dir.path(), "target/\n*.tmp\n");
        let m = load(dir.path());
        assert!(!m.is_ignored(
            &PathBuf::from(dir.path()).join("src/commands/mod.rs"),
            false
        ));
    }
}
