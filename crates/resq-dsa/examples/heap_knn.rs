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

//! # Bounded Heap — K-Nearest Neighbor Search
//!
//! Demonstrates using a `BoundedHeap` to find the 5 closest 2D points to a
//! query point out of 1,000 candidates. The heap maintains only K items in
//! memory, making it efficient for streaming or large datasets.
//!
//! Run: `cargo run -p resq-dsa --example heap_knn`

#![allow(clippy::cast_precision_loss, clippy::too_many_lines)]

use resq_dsa::heap::BoundedHeap;

#[derive(Debug)]
struct Point {
    id: u32,
    x: f64,
    y: f64,
}

fn euclidean(a: &Point, b: &Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Simple LCG pseudo-random number generator (no external dependencies).
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Generate a float in [-range, +range].
    fn next_f64(&mut self, range: f64) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        // Map to [0, 1) then scale to [-range, +range].
        let normalized = (self.0 >> 11) as f64 / (1u64 << 53) as f64;
        (normalized * 2.0 - 1.0) * range
    }
}

fn main() {
    println!("=== Bounded Heap: K-Nearest Neighbor Search ===\n");

    let k = 5;
    let num_points = 1_000;
    let query = Point {
        id: 0,
        x: 0.0,
        y: 0.0,
    };
    let mut rng = Lcg::new(12345);

    // --- Phase 1: Generate random points ---
    println!("Phase 1: Generating {num_points} random 2D points...");
    let points: Vec<Point> = (1..=num_points)
        .map(|id| Point {
            id,
            x: rng.next_f64(100.0),
            y: rng.next_f64(100.0),
        })
        .collect();

    // --- Phase 2: Find K nearest using BoundedHeap ---
    println!(
        "Phase 2: Finding {k} nearest to query ({}, {})...\n",
        query.x, query.y
    );

    let mut heap = BoundedHeap::new(k, |p: &Point| euclidean(p, &query));
    for p in &points {
        heap.insert(Point {
            id: p.id,
            x: p.x,
            y: p.y,
        });
    }

    println!("  BoundedHeap results (nearest first):");
    let sorted = heap.to_sorted();
    for (rank, p) in sorted.iter().enumerate() {
        let dist = euclidean(p, &query);
        println!(
            "    #{}: Point #{:<4} ({:>7.2}, {:>7.2})  distance: {:.4}",
            rank + 1,
            p.id,
            p.x,
            p.y,
            dist,
        );
    }

    // --- Phase 3: Verify against brute-force sort ---
    println!("\n  Brute-force verification (sort all {num_points} points):");
    let mut all_distances: Vec<(u32, f64)> = points
        .iter()
        .map(|p| (p.id, euclidean(p, &query)))
        .collect();
    all_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    for (rank, (id, dist)) in all_distances.iter().take(k).enumerate() {
        println!("    #{}: Point #{:<4}  distance: {:.4}", rank + 1, id, dist);
    }

    let heap_ids: Vec<u32> = sorted.iter().map(|p| p.id).collect();
    let brute_ids: Vec<u32> = all_distances.iter().take(k).map(|(id, _)| *id).collect();
    let match_status = if heap_ids == brute_ids {
        "MATCH"
    } else {
        "MISMATCH"
    };
    println!("\n  Results: {match_status}");

    // --- Phase 4: Demonstrate streaming behavior ---
    println!("\nPhase 3: Streaming behavior...");
    println!("  Heap capacity: {k}");
    println!("  Items processed: {num_points}");
    println!("  Items kept: {}", heap.len());
    println!(
        "  The heap automatically evicted {} items with larger distances.",
        num_points as usize - heap.len()
    );

    println!("\n=== Key Takeaway ===");
    println!("BoundedHeap is O(n log k) vs O(n log n) for a full sort — significant when k << n.");
    println!("It processes items one at a time, making it ideal for streaming data where you");
    println!("can't hold all items in memory simultaneously.");
}
