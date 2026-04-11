/*
 * Copyright 2026 ResQ Software
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

/// Trie (prefix tree) for efficient string operations and Rabin-Karp pattern matching.
///
/// # Modules
///
/// - [`Trie`] - Prefix tree for autocomplete, spell checking, IP routing
/// - [`rabin_karp`] - String pattern matching using rolling hash
use std::collections::HashMap;

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_end: bool,
}

/// A prefix tree (trie) for efficient string storage and retrieval.
///
/// Supports insertion, exact search, and prefix-based autocomplete.
/// All operations are O(m) where m is the length of the string.
///
/// # Use Cases
///
/// - Autocomplete suggestions
/// - IP address routing tables
/// - Spell checking
/// - Word frequency tracking
///
/// # Examples
///
/// ```
/// use resq_dsa::trie::Trie;
///
/// let mut t = Trie::new();
/// t.insert("drone");
/// t.insert("drone-001");
/// t.insert("drone-002");
///
/// assert!(t.search("drone"));
/// assert!(!t.search("dro"));
///
/// let suggestions = t.starts_with("drone-");
/// assert!(suggestions.contains(&"drone-001".to_string()));
/// ```
pub struct Trie {
    root: TrieNode,
}

impl Trie {
    /// Creates a new empty Trie.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
        }
    }

    /// Inserts a word into the trie.
    pub fn insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        node.is_end = true;
    }

    /// Returns `true` if the exact word exists in the trie.
    #[must_use]
    pub fn search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Returns all words in the trie that start with the given prefix.
    ///
    /// Uses a depth-first search with push/pop to avoid repeated
    /// string cloning during traversal.
    #[must_use]
    pub fn starts_with(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return vec![],
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::collect_words(node, &mut buf, &mut results);
        results
    }

    /// DFS helper that uses push/pop on a shared buffer to avoid cloning.
    fn collect_words(node: &TrieNode, buf: &mut String, results: &mut Vec<String>) {
        if node.is_end {
            results.push(buf.clone());
        }
        for (&ch, child) in &node.children {
            buf.push(ch);
            Self::collect_words(child, buf, results);
            buf.pop();
        }
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

/// Rabin-Karp string pattern matching using rolling hash.
///
/// Finds all occurrences of `pattern` in `text` using a polynomial
/// rolling hash with modular arithmetic. Average case O(n + m).
///
/// The algorithm operates on `char` boundaries, so it is
/// Unicode-aware (multi-byte characters are handled correctly).
///
/// # Arguments
///
/// * `text` - The text to search in
/// * `pattern` - The pattern to search for
///
/// # Returns
///
/// A vector of starting indices (in chars, not bytes) where the pattern matches.
///
/// # Examples
///
/// ```
/// use resq_dsa::trie::rabin_karp;
///
/// let matches = rabin_karp("ababab", "ab");
/// assert_eq!(matches, vec![0, 2, 4]);
///
/// let single = rabin_karp("hello world", "world");
/// assert_eq!(single, vec![6]);
///
/// let none = rabin_karp("hello", "xyz");
/// assert!(none.is_empty());
/// ```
#[must_use]
pub fn rabin_karp(text: &str, pattern: &str) -> Vec<usize> {
    const BASE: u64 = 31;
    const MOD: u64 = 1_000_000_007;

    let pat: Vec<char> = pattern.chars().collect();
    let m = pat.len();
    let mut matches = Vec::new();
    if m == 0 {
        return matches;
    }

    // We use a rolling window of chars since text can be large.
    // However, since we need to compare the window with the pattern,
    // and we also need to know the character that falls out of the window,
    // we use a full ring buffer of size `m`.
    let mut window: Vec<char> = Vec::with_capacity(m);
    let mut chars = text.chars();

    // Fill initial window
    for _ in 0..m {
        if let Some(c) = chars.next() {
            window.push(c);
        } else {
            return matches; // Text is shorter than pattern
        }
    }

    let mut pw = vec![1u64; m];
    for i in 1..m {
        pw[i] = pw[i - 1].wrapping_mul(BASE) % MOD;
    }

    let cv = |c: char| -> u64 { (c as u64).wrapping_add(1) };
    let (mut ph, mut wh) = (0u64, 0u64);

    for i in 0..m {
        ph = (ph + cv(pat[i]) * pw[m - 1 - i]) % MOD;
        wh = (wh + cv(window[i]) * pw[m - 1 - i]) % MOD;
    }

    // `window_idx` tracks the start index of the window in the ring buffer
    let mut window_idx = 0;

    let check_match = |window: &[char], start_idx: usize, pat: &[char]| -> bool {
        for i in 0..m {
            if window[(start_idx + i) % m] != pat[i] {
                return false;
            }
        }
        true
    };

    if wh == ph && check_match(&window, window_idx, &pat) {
        matches.push(0);
    }

    let mut i = 1;
    for next_char in chars {
        let old_char = window[window_idx];

        // Remove old char
        wh = (wh + MOD - cv(old_char) * pw[m - 1] % MOD) % MOD;
        // Shift left
        wh = (wh * BASE) % MOD;
        // Add new char
        wh = (wh + cv(next_char)) % MOD;

        // Update ring buffer
        window[window_idx] = next_char;
        window_idx = (window_idx + 1) % m;

        if wh == ph && check_match(&window, window_idx, &pat) {
            matches.push(i);
        }
        i += 1;
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trie_insert_search() {
        let mut t = Trie::new();
        t.insert("drone");
        t.insert("droning");
        assert!(t.search("drone"));
        assert!(!t.search("dron"));
        assert!(t.search("droning"));
    }

    #[test]
    fn trie_starts_with() {
        let mut t = Trie::new();
        for w in &["alert", "alerting", "alarm", "base"] {
            t.insert(w);
        }
        let mut r = t.starts_with("al");
        r.sort();
        assert_eq!(r, vec!["alarm", "alert", "alerting"]);
    }

    #[test]
    fn rabin_karp_multiple() {
        assert_eq!(rabin_karp("ababab", "ab"), vec![0, 2, 4]);
    }

    #[test]
    fn rabin_karp_single() {
        assert_eq!(rabin_karp("hello world", "world"), vec![6]);
    }

    #[test]
    fn rabin_karp_no_match() {
        assert!(rabin_karp("hello", "xyz").is_empty());
    }

    #[test]
    fn empty_string_insert_and_search() {
        let mut t = Trie::new();
        t.insert("");
        assert!(t.search(""));
        assert!(!t.search("a"));
    }

    #[test]
    fn search_not_inserted() {
        let t = Trie::new();
        assert!(!t.search("anything"));
    }

    #[test]
    fn starts_with_empty_prefix() {
        let mut t = Trie::new();
        t.insert("alpha");
        t.insert("beta");
        let mut r = t.starts_with("");
        r.sort();
        assert_eq!(r, vec!["alpha", "beta"]);
    }

    #[test]
    fn starts_with_no_matches() {
        let mut t = Trie::new();
        t.insert("hello");
        assert!(t.starts_with("xyz").is_empty());
    }

    #[test]
    fn unicode_support() {
        let mut t = Trie::new();
        t.insert("café");
        t.insert("naïve");
        assert!(t.search("café"));
        assert!(!t.search("cafe"));
        let r = t.starts_with("caf");
        assert_eq!(r, vec!["café"]);
    }

    #[test]
    fn rabin_karp_empty_pattern() {
        // Empty pattern should return empty results.
        assert!(rabin_karp("hello", "").is_empty());
    }

    #[test]
    fn rabin_karp_unicode() {
        assert_eq!(rabin_karp("aéaéa", "aé"), vec![0, 2]);
    }

    #[test]
    fn rabin_karp_pattern_longer_than_text() {
        assert!(rabin_karp("hi", "longer pattern").is_empty());
    }

    #[test]
    fn rabin_karp_full_match() {
        assert_eq!(rabin_karp("exact", "exact"), vec![0]);
    }
}
