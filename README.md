<!--
  Copyright 2026 ResQ

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# ResQ Crates

[![CI](https://img.shields.io/github/actions/workflow/status/resq-software/crates/ci.yml?branch=main&label=ci&style=flat-square)](https://github.com/resq-software/crates/actions)
[![crates.io](https://img.shields.io/crates/v/resq-dsa?style=flat-square)](https://crates.io/crates/resq-dsa)
[![crates.io](https://img.shields.io/crates/v/resq-cli?style=flat-square)](https://crates.io/crates/resq-cli)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=flat-square)](./LICENSE)

A Cargo workspace and crate registry for all ResQ Rust packages published to [crates.io](https://crates.io). Contains production-grade libraries and a suite of CLI/TUI developer tools for the ResQ autonomous drone platform.

---

## Packages

| Crate | Description | crates.io |
| :--- | :--- | :--- |
| [`resq-dsa`](resq-dsa/) | Data structures and algorithms -- zero dependencies | [![crates.io](https://img.shields.io/crates/v/resq-dsa?style=flat-square)](https://crates.io/crates/resq-dsa) |
| [`resq-cli`](cli/) | Unified CLI entry point (`resq` binary) | [![crates.io](https://img.shields.io/crates/v/resq-cli?style=flat-square)](https://crates.io/crates/resq-cli) |
| [`resq-tui`](resq-tui/) | Shared Ratatui component library for all TUI tools | [![crates.io](https://img.shields.io/crates/v/resq-tui?style=flat-square)](https://crates.io/crates/resq-tui) |
| [`resq-health`](health-checker/) | Service health monitoring dashboard | [![crates.io](https://img.shields.io/crates/v/resq-health?style=flat-square)](https://crates.io/crates/resq-health) |
| [`resq-deploy`](deploy-cli/) | Kubernetes and Docker Compose deployment TUI | [![crates.io](https://img.shields.io/crates/v/resq-deploy?style=flat-square)](https://crates.io/crates/resq-deploy) |
| [`resq-logs`](log-viewer/) | Log aggregator and stream viewer | [![crates.io](https://img.shields.io/crates/v/resq-logs?style=flat-square)](https://crates.io/crates/resq-logs) |
| [`resq-perf`](perf-monitor/) | Performance monitoring dashboard | [![crates.io](https://img.shields.io/crates/v/resq-perf?style=flat-square)](https://crates.io/crates/resq-perf) |
| [`resq-flame`](flame-graph/) | CPU profiler and flame graph generator | [![crates.io](https://img.shields.io/crates/v/resq-flame?style=flat-square)](https://crates.io/crates/resq-flame) |
| [`bin-explorer`](bin-explorer/) | Machine code and binary analyzer | [![crates.io](https://img.shields.io/crates/v/bin-explorer?style=flat-square)](https://crates.io/crates/bin-explorer) |
| [`resq-clean`](cleanup/) | Interactive workspace cleaner | [![crates.io](https://img.shields.io/crates/v/resq-clean?style=flat-square)](https://crates.io/crates/resq-clean) |

---

## resq-dsa

Production-grade data structures and algorithms with **zero external dependencies**. Supports `no_std` environments with the `alloc` crate.

### Installation

```sh
cargo add resq-dsa
```

### Features

| Feature | Default | Description |
| :--- | :--- | :--- |
| `std` | Yes | Enables standard library support |

For `no_std` environments, disable default features:

```toml
[dependencies]
resq-dsa = { version = "0.1", default-features = false }
```

The crate uses `alloc` internally, so a global allocator is required even in `no_std` mode.

### Bloom Filter

Space-efficient probabilistic set membership. False positives are possible; false negatives are not.

```rust
use resq_dsa::bloom::BloomFilter;

// Create a filter for ~1000 items with 1% false positive rate
let mut bf = BloomFilter::new(1000, 0.01);

// Add items
bf.add("drone-001");
bf.add("drone-002");

// Check membership
assert!(bf.has("drone-001"));   // definitely added
assert!(!bf.has("drone-999"));  // definitely NOT added
```

### Count-Min Sketch

Space-efficient probabilistic frequency estimation. May overcount but never undercounts.

```rust
use resq_dsa::count_min::CountMinSketch;

// Create a sketch with epsilon=0.01, delta=0.01 error bounds
let mut cms = CountMinSketch::new(0.01, 0.01);

// Increment frequency counts
cms.increment("sensor-a", 5);
cms.increment("sensor-b", 1);
cms.increment("sensor-a", 3);

// Estimate frequency
assert!(cms.estimate("sensor-a") >= 8);
```

### Graph

Weighted directed graph with BFS traversal, Dijkstra's shortest path, and A* pathfinding.

```rust
use resq_dsa::graph::Graph;

let mut g = Graph::<&str>::new();
g.add_edge("base", "waypoint-1", 100);
g.add_edge("waypoint-1", "target", 50);
g.add_edge("base", "target", 200);

// BFS traversal (unweighted)
let visited = g.bfs(&"base");
assert!(visited.contains(&"target"));

// Dijkstra's shortest path
let (path, cost) = g.dijkstra(&"base", &"target").unwrap();
assert_eq!(path, vec!["base", "waypoint-1", "target"]);
assert_eq!(cost, 150);

// A* with heuristic
let (path, cost) = g.astar(&"base", &"target", |_, _| 0).unwrap();
assert_eq!(cost, 150);
```

### Bounded Heap

A bounded max-heap for tracking the K smallest entries (K-nearest neighbors).

```rust
use resq_dsa::heap::BoundedHeap;

// Keep the 3 nearest neighbors, using distance function
let mut heap = BoundedHeap::new(3, |item: &(i32, i32)| {
    ((item.0 * item.0 + item.1 * item.1) as u64)
});

heap.insert((1, 2));
heap.insert((10, 10));
heap.insert((0, 1));
heap.insert((3, 3));  // evicts (10, 10) since heap is full

let sorted = heap.to_sorted();
assert_eq!(sorted.len(), 3);
```

### Trie

Prefix tree for efficient string storage, exact search, and autocomplete.

```rust
use resq_dsa::trie::Trie;

let mut t = Trie::new();
t.insert("drone");
t.insert("drone-001");
t.insert("drone-002");
t.insert("deploy");

// Exact search
assert!(t.search("drone"));
assert!(!t.search("dro"));

// Prefix-based autocomplete
let results = t.starts_with("drone-");
assert_eq!(results, vec!["drone-001", "drone-002"]);
```

### Rabin-Karp

Rolling-hash string pattern matching. Returns all starting indices of pattern occurrences.

```rust
use resq_dsa::trie::rabin_karp;

let indices = rabin_karp("the drone flew over the base", "the");
assert_eq!(indices, vec![0, 23]);
```

---

## CLI Tools

The workspace includes a suite of developer tools for the ResQ platform, all sharing a common TUI foundation via `resq-tui`.

| Command | Tool | Description |
| :--- | :--- | :--- |
| `resq audit` | resq-cli | Security audit (OSV/dependency scanning) |
| `resq health` | resq-health | Service health monitoring dashboard |
| `resq deploy` | resq-deploy | Kubernetes/Docker Compose deployment TUI |
| `resq logs` | resq-logs | Aggregate and stream service logs |
| `resq perf` | resq-perf | Real-time performance metrics |
| `resq flame` | resq-flame | CPU profiling and flame graph generation |
| `resq asm` | bin-explorer | Binary/machine code analysis |
| `resq clean` | resq-clean | Interactive workspace cleaner |
| `resq copyright` | resq-cli | Apache-2.0 license header enforcement |

### Quick Start

```sh
cargo install resq-cli
resq help
```

---

## Development

### Prerequisites

- **Rust:** Stable toolchain via `rustup` (pinned in `rust-toolchain.toml`).
- **Nix (optional):** For reproducible development environments, use `nix develop`.

### Build

```sh
git clone https://github.com/resq-software/crates.git
cd crates
cargo build --release --workspace
```

### Test

```sh
# Run all tests
cargo test --workspace

# Run only resq-dsa tests (including ignored complexity tests)
cargo test -p resq-dsa -- --include-ignored
```

### Lint

```sh
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

### Cargo Aliases

| Alias | Description |
| :--- | :--- |
| `cargo resq` | Run the ResQ CLI |
| `cargo health` | Launch health monitor |
| `cargo logs` | Launch log viewer |
| `cargo perf` | Launch performance dashboard |
| `cargo deploy` | Launch deployment TUI |
| `cargo flame` | Launch flame graph profiler |
| `cargo bin` | Launch binary explorer |
| `cargo cleanup` | Launch workspace cleaner |
| `cargo check-all` | Fastest correctness check |

---

## Contributing

We follow [Conventional Commits](https://www.conventionalcommits.org/).

1. **Branch:** Use `feat/`, `fix/`, or `refactor/` prefixes.
2. **Quality:** Run `cargo clippy --workspace -- -D warnings` before submitting.
3. **Tests:** All CI workflows must pass, including `osv-scan`.
4. **Headers:** Run `resq copyright` to enforce Apache-2.0 license headers on new source files.
5. **Agent Guides:** Run `./agent-sync.sh` if you modify `AGENTS.md` or `CLAUDE.md`.

---

## License

Copyright 2026 ResQ. Licensed under the [Apache License, Version 2.0](./LICENSE).
