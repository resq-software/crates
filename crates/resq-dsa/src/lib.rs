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

//! Production-grade data structures and algorithms — zero external dependencies.
//!
//! A collection of space-efficient probabilistic data structures and
//! graph algorithms for general-purpose use.
//!
//! # Modules
//!
//! - [`bloom`] - Bloom filter for approximate set membership
//! - [`count_min`] - Count-Min sketch for frequency estimation
//! - [`graph`] - Graph algorithms (BFS, Dijkstra, A*)
//! - [`heap`] - Bounded heap for K-nearest neighbor tracking
//! - [`trie`] - Trie prefix tree and Rabin-Karp string matching
//!
//! # Usage
//!
//! ```
//! use resq_dsa::bloom::BloomFilter;
//! use resq_dsa::count_min::CountMinSketch;
//! use resq_dsa::graph::Graph;
//!
//! // Bloom filter for deduplication
//! let mut bf = BloomFilter::new(1000, 0.01);
//! bf.add("drone-001");
//! assert!(bf.has("drone-001"));
//!
//! // Count-Min for frequency tracking
//! let mut cms = CountMinSketch::new(0.01, 0.01);
//! cms.increment("sensor-reading", 5);
//!
//! // Graph for pathfinding
//! let mut g = Graph::<&str>::new();
//! g.add_edge("base", "waypoint-1", 100);
//! g.add_edge("waypoint-1", "target", 50);
//! let (path, cost) = g.dijkstra(&"base", &"target").unwrap();
//! assert_eq!(path, vec!["base", "waypoint-1", "target"]);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

/// Bloom filter for approximate set membership.
pub mod bloom;
/// Count-Min sketch for frequency estimation.
pub mod count_min;
/// Graph data structure and pathfinding algorithms.
pub mod graph;
/// Bounded heap for K-nearest neighbor tracking.
pub mod heap;
/// Trie prefix tree and Rabin-Karp pattern matching.
pub mod trie;
