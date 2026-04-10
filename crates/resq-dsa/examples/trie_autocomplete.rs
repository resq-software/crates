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

//! # Trie — Command Palette Autocomplete
//!
//! Demonstrates using a `Trie` for autocomplete in a CLI command palette,
//! plus `rabin_karp` for pattern matching in command logs.
//!
//! Run: `cargo run -p resq-dsa --example trie_autocomplete`

#![allow(clippy::too_many_lines)]

use resq_dsa::trie::{rabin_karp, Trie};

fn main() {
    println!("=== Trie: Command Palette Autocomplete ===\n");

    let mut trie = Trie::new();

    // Insert realistic CLI commands.
    let commands = [
        "deploy",
        "deploy:dev",
        "deploy:staging",
        "deploy:prod",
        "deploy:rollback",
        "deploy:status",
        "health",
        "health:check",
        "health:monitor",
        "health:report",
        "logs",
        "logs:tail",
        "logs:search",
        "logs:export",
        "perf",
        "perf:dashboard",
        "perf:snapshot",
        "perf:compare",
        "clean",
        "clean:artifacts",
        "clean:cache",
        "clean:all",
        "flame",
        "flame:cpu",
        "flame:memory",
        "flame:compare",
        "bin",
        "bin:analyze",
        "bin:symbols",
        "bin:disasm",
        "audit",
        "audit:deps",
        "audit:secrets",
        "audit:licenses",
        "config",
        "config:get",
        "config:set",
        "config:list",
        "version",
        "version:bump",
        "version:changelog",
    ];

    println!(
        "Phase 1: Loading {} commands into trie...\n",
        commands.len()
    );
    for cmd in &commands {
        trie.insert(cmd);
    }

    // --- Phase 2: Exact search ---
    println!("Phase 2: Exact search");
    let tests = [
        ("deploy", true),
        ("dep", false),
        ("health:check", true),
        ("xyz", false),
    ];
    for (query, expected) in &tests {
        let found = trie.search(query);
        let status = if found == *expected {
            "OK"
        } else {
            "UNEXPECTED"
        };
        println!("  search(\"{query}\") → {found} [{status}]");
    }

    // --- Phase 3: Prefix autocomplete ---
    println!("\nPhase 3: Prefix autocomplete");

    let prefixes = ["deploy:", "health", "log", "clean:"];
    for prefix in &prefixes {
        let mut matches = trie.starts_with(prefix);
        matches.sort();
        println!("  starts_with(\"{prefix}\") → {} matches:", matches.len());
        for m in &matches {
            println!("    • {m}");
        }
    }

    // --- Phase 4: Non-existent prefix ---
    println!(
        "\n  starts_with(\"xyz\") → {} matches",
        trie.starts_with("xyz").len()
    );

    // --- Phase 5: Rabin-Karp pattern matching ---
    println!("\nPhase 4: Rabin-Karp pattern matching in command log");
    let log = "\
[10:01] user ran deploy:staging — success
[10:05] user ran health:check — 3/3 healthy
[10:12] user ran deploy:prod — success
[10:15] user ran logs:tail — streaming
[10:20] user ran deploy:rollback — success
[10:30] user ran health:check — 2/3 degraded";

    println!("  Searching for \"deploy\" in command log...");
    let positions = rabin_karp(log, "deploy");
    println!(
        "  Found {} occurrences at char positions: {:?}",
        positions.len(),
        positions
    );

    println!("\n  Searching for \"health:check\" in command log...");
    let positions = rabin_karp(log, "health:check");
    println!(
        "  Found {} occurrences at char positions: {:?}",
        positions.len(),
        positions
    );

    println!("\n=== Key Takeaway ===");
    println!("Tries give O(m) prefix search — ideal for autocomplete and routing tables.");
    println!("Rabin-Karp provides efficient substring search using rolling hashes,");
    println!("averaging O(n+m) for finding all occurrences of a pattern in text.");
}
