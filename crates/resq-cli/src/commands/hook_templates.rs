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

//! Canonical git-hook templates embedded via `include_str!`.
//!
//! Both `resq dev install-hooks` (scaffolding) and `resq hooks doctor`
//! (drift detection) read from this single source. The templates are kept
//! in sync with <https://github.com/resq-software/dev/tree/main/scripts/git-hooks>
//! and the repo-local `.git-hooks/` — CI workflows in both repos enforce
//! byte equality.

/// Canonical hook templates: `(hook_name, file_content)` tuples.
/// The hook names correspond to git hook trigger names.
pub const HOOK_TEMPLATES: &[(&str, &str)] = &[
    (
        "pre-commit",
        include_str!("../../templates/git-hooks/pre-commit"),
    ),
    (
        "commit-msg",
        include_str!("../../templates/git-hooks/commit-msg"),
    ),
    (
        "prepare-commit-msg",
        include_str!("../../templates/git-hooks/prepare-commit-msg"),
    ),
    (
        "pre-push",
        include_str!("../../templates/git-hooks/pre-push"),
    ),
    (
        "post-checkout",
        include_str!("../../templates/git-hooks/post-checkout"),
    ),
    (
        "post-merge",
        include_str!("../../templates/git-hooks/post-merge"),
    ),
];

/// Returns the canonical content for a given hook name, or `None` if the
/// hook is not one of the known canonical hooks.
#[must_use]
pub fn hook_content(name: &str) -> Option<&'static str> {
    HOOK_TEMPLATES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}
