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

// Gitignore parsing and utilities.
//
// Provides functions for parsing .gitignore files and matching
// paths against ignore patterns.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Fallback exclude dirs when `.gitignore` is missing or unreadable.
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

/// Parse `.gitignore` from `root` and return a list of simple directory/file
/// names to exclude during traversal.
///
/// Strategy (matches the TS `parseGitignore` in `sync-turbo-env.ts`):
/// - Read `.gitignore`, split into lines
/// - Strip comments (`#`) and blank lines
/// - Normalize: remove leading `/` and trailing `/`
/// - Drop negations (`!`) and wildcard patterns (`*`) — too complex for
///   simple component-based matching; these are already handled by git itself
/// - Always include `.git` and `node_modules` as safety nets
pub fn parse_gitignore(root: &Path) -> Vec<String> {
    let gitignore_path = root.join(".gitignore");

    let content = match fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => {
            return FALLBACK_EXCLUDES.iter().map(|s| (*s).to_string()).collect();
        }
    };

    let mut seen = HashSet::new();
    let mut excludes: Vec<String> = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Skip negation patterns and wildcard patterns
        if line.starts_with('!') || line.contains('*') {
            continue;
        }

        // Normalize: strip leading `/` and trailing `/`
        let normalized = line.trim_start_matches('/').trim_end_matches('/');

        if normalized.is_empty() {
            continue;
        }

        // Only keep simple names (no path separators) for component matching
        if !normalized.contains('/') && seen.insert(normalized.to_string()) {
            excludes.push(normalized.to_string());
        }
    }

    // Always ensure these are present
    for must_have in &[".git", "node_modules"] {
        if seen.insert((*must_have).to_string()) {
            excludes.push((*must_have).to_string());
        }
    }

    excludes
}

/// Check whether `path` should be skipped based on its directory components
/// matching any entry in `excludes`.
pub fn should_skip_path(path: &Path, excludes: &[String]) -> bool {
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if excludes.iter().any(|ex| name == *ex) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_parse_gitignore_basic() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".gitignore"),
            "# comment\nnode_modules/\ndist\n\n*.log\n!important\ntarget/\n",
        )
        .unwrap();

        let excludes = parse_gitignore(dir.path());
        assert!(excludes.contains(&"node_modules".to_string()));
        assert!(excludes.contains(&"dist".to_string()));
        assert!(excludes.contains(&"target".to_string()));
        assert!(excludes.contains(&".git".to_string())); // always present
                                                         // Wildcards and negations should be excluded
        assert!(!excludes.iter().any(|e| e.contains('*')));
        assert!(!excludes.iter().any(|e| e.starts_with('!')));
    }

    #[test]
    fn test_parse_gitignore_missing_file() {
        let dir = tempdir().unwrap();
        let excludes = parse_gitignore(dir.path());
        // Should return fallbacks
        assert!(excludes.contains(&"node_modules".to_string()));
        assert!(excludes.contains(&".git".to_string()));
        assert!(excludes.contains(&"dist".to_string()));
    }

    #[test]
    fn test_should_skip_path() {
        let excludes = vec!["node_modules".to_string(), ".git".to_string()];

        assert!(should_skip_path(
            &PathBuf::from("src/node_modules/foo.js"),
            &excludes
        ));
        assert!(should_skip_path(&PathBuf::from(".git/config"), &excludes));
        assert!(!should_skip_path(&PathBuf::from("src/main.rs"), &excludes));
    }

    #[test]
    fn test_deduplication() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join(".gitignore"),
            "node_modules\nnode_modules/\nnode_modules\n",
        )
        .unwrap();

        let excludes = parse_gitignore(dir.path());
        let count = excludes.iter().filter(|e| *e == "node_modules").count();
        assert_eq!(count, 1, "node_modules should appear exactly once");
    }
}
