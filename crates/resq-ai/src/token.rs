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

//! Token estimation and budget-aware text truncation.

/// Estimate token count using the chars/4 heuristic.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

/// Truncate text to fit within a token budget.
/// Cuts at line boundaries to avoid broken diff hunks.
#[must_use]
pub fn truncate_to_budget(text: &str, max_tokens: usize) -> &str {
    let max_chars = max_tokens * 4;
    if text.len() <= max_chars {
        return text;
    }

    let slice = &text[..max_chars.min(text.len())];
    match slice.rfind('\n') {
        Some(pos) => &text[..=pos],
        None => slice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_short_text() {
        assert_eq!(estimate_tokens("hello"), 2);
    }

    #[test]
    fn estimate_longer_text() {
        let text = "a".repeat(400);
        assert_eq!(estimate_tokens(&text), 100);
    }

    #[test]
    fn truncate_within_budget() {
        let text = "line one\nline two\nline three\n";
        let result = truncate_to_budget(text, 1000);
        assert_eq!(result, text);
    }

    #[test]
    fn truncate_at_line_boundary() {
        let text = "line one\nline two\nline three\n";
        let result = truncate_to_budget(text, 5);
        assert_eq!(result, "line one\nline two\n");
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_to_budget("", 100), "");
    }

    #[test]
    fn truncate_single_long_line() {
        let text = "a".repeat(100);
        let result = truncate_to_budget(&text, 10);
        assert!(result.len() <= 40);
    }
}
