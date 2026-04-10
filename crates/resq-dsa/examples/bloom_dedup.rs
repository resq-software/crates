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

//! # Bloom Filter — Web Crawler URL Deduplication
//!
//! Demonstrates using a `BloomFilter` to efficiently track which URLs a web
//! crawler has already visited. The Bloom filter uses a fraction of the memory
//! a `HashSet` would need, at the cost of occasional false positives (it may
//! say a URL was visited when it wasn't), but **never** false negatives.
//!
//! Run: `cargo run -p resq-dsa --example bloom_dedup`

use resq_dsa::bloom::BloomFilter;
use std::collections::HashSet;

fn main() {
    println!("=== Bloom Filter: Web Crawler URL Deduplication ===\n");

    // Configure: capacity for 10,000 URLs with a 1% false positive rate.
    let capacity = 10_000;
    let fp_rate = 0.01;
    let mut bf = BloomFilter::new(capacity, fp_rate);

    // --- Phase 1: Crawl 10,000 unique pages ---
    println!("Phase 1: Adding {capacity} unique URLs...");
    for i in 0..capacity {
        let url = format!("https://example.com/page/{i}");
        bf.add(&url);
    }
    println!("  Items stored: {}", bf.len());

    // --- Phase 2: Verify — every added URL must be found ---
    println!("\nPhase 2: Verifying all added URLs are found...");
    let mut missed = 0;
    for i in 0..capacity {
        let url = format!("https://example.com/page/{i}");
        if !bf.has(&url) {
            missed += 1;
        }
    }
    println!("  False negatives: {missed} (should always be 0)");

    // --- Phase 3: Measure false positive rate on unseen URLs ---
    println!("\nPhase 3: Testing {capacity} unseen URLs for false positives...");
    let mut false_positives = 0;
    for i in capacity..(capacity * 2) {
        let url = format!("https://example.com/page/{i}");
        if bf.has(&url) {
            false_positives += 1;
        }
    }
    let observed_fp_rate = false_positives as f64 / capacity as f64;
    println!("  False positives: {false_positives} / {capacity}");
    println!(
        "  Observed FP rate: {:.4}% (configured: {:.2}%)",
        observed_fp_rate * 100.0,
        fp_rate * 100.0
    );

    // --- Phase 4: Memory comparison ---
    println!("\nPhase 4: Memory comparison...");
    let mut hashset = HashSet::new();
    for i in 0..capacity {
        hashset.insert(format!("https://example.com/page/{i}"));
    }
    // Average URL is ~35 bytes + String overhead (~24 bytes) + HashSet entry (~8 bytes)
    let hashset_estimate = hashset.len() * 67;
    println!(
        "  HashSet estimated memory: ~{} KB ({} entries x ~67 bytes)",
        hashset_estimate / 1024,
        hashset.len()
    );
    println!("  BloomFilter: far smaller bit-array for the same task");

    // --- Phase 5: Clear and reuse ---
    println!("\nPhase 5: Clear and reuse...");
    bf.clear();
    println!("  After clear — len: {}, is_empty: {}", bf.len(), bf.is_empty());
    bf.add("https://new-crawl.com/start");
    println!("  After re-adding one URL — has it: {}", bf.has("https://new-crawl.com/start"));

    println!("\n=== Key Takeaway ===");
    println!("Bloom filters trade a small false positive rate for massive memory savings.");
    println!("Perfect for deduplication where occasional redundant work is acceptable.");
}
