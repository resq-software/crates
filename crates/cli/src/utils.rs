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

// Utility functions for the ResQ CLI.
//
// Provides common functions for path resolution, project detection,
// and other shared functionality.

use std::env;
use std::path::PathBuf;

/// Markers that indicate the root of the project
const ROOT_MARKERS: &[&str] = &[
    "resQ.sln",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    ".git",
];

/// Finds the project root by climbing up the directory tree from the CWD.
/// Returns the current directory if no root marker is found.
pub fn find_project_root() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    for ancestor in cwd.ancestors() {
        for marker in ROOT_MARKERS {
            if ancestor.join(marker).exists() {
                return ancestor.to_path_buf();
            }
        }
    }

    cwd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_find_project_root() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let root = dir.path();

        // Create a marker file
        fs::write(root.join("resQ.sln"), "")?;

        // Create a subdirectory
        let sub = root.join("a/b/c");
        fs::create_dir_all(&sub)?;

        // Save current CWD to restore later
        let original_cwd = env::current_dir()?;

        // Change CWD to sub
        env::set_current_dir(&sub)?;

        let found_root = find_project_root();

        // Restore CWD
        env::set_current_dir(original_cwd)?;

        assert_eq!(found_root.canonicalize()?, root.canonicalize()?);
        Ok(())
    }
}
