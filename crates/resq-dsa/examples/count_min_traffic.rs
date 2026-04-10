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

//! # Count-Min Sketch — API Traffic Frequency Analysis
//!
//! Demonstrates using a `CountMinSketch` to track request frequencies across
//! API endpoints. The sketch uses fixed memory regardless of how many distinct
//! keys it sees, and **never undercounts** (estimates are always >= actual).
//!
//! Run: `cargo run -p resq-dsa --example count_min_traffic`

use resq_dsa::count_min::CountMinSketch;
use std::collections::HashMap;

/// Simple deterministic "random" number generator (LCG) to avoid needing
/// an external `rand` dependency.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        self.0
    }

    /// Return a value in [0, bound).
    fn next_bound(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

fn main() {
    println!("=== Count-Min Sketch: API Traffic Frequency Analysis ===\n");

    // Endpoints with a Zipf-like traffic distribution.
    let endpoints = [
        "POST /api/v1/auth/login",
        "GET  /api/v1/users/me",
        "GET  /api/v1/feed",
        "POST /api/v1/events",
        "GET  /api/v1/notifications",
        "PUT  /api/v1/users/settings",
        "GET  /api/v1/search",
        "POST /api/v1/upload",
        "GET  /api/v1/health",
        "DELETE /api/v1/sessions",
        "GET  /api/v1/metrics",
        "POST /api/v1/webhooks",
        "GET  /api/v1/config",
        "PUT  /api/v1/deploy",
        "GET  /api/v1/status",
    ];

    // Weights simulate a skewed distribution — first endpoints get more traffic.
    let weights: Vec<u64> = (0..endpoints.len())
        .map(|i| 1000 / (i as u64 + 1))
        .collect();
    let total_weight: u64 = weights.iter().sum();

    let total_requests: u64 = 100_000;
    let mut cms = CountMinSketch::new(0.001, 0.01);
    let mut actual: HashMap<&str, u64> = HashMap::new();
    let mut rng = Lcg::new(42);

    // --- Phase 1: Simulate traffic ---
    println!("Phase 1: Simulating {total_requests} API requests across {} endpoints...\n", endpoints.len());
    for _ in 0..total_requests {
        let r = rng.next_bound(total_weight);
        let mut cumulative = 0u64;
        for (i, &w) in weights.iter().enumerate() {
            cumulative += w;
            if r < cumulative {
                cms.increment(endpoints[i], 1);
                *actual.entry(endpoints[i]).or_default() += 1;
                break;
            }
        }
    }

    // --- Phase 2: Compare estimates vs actual ---
    println!("Phase 2: Top endpoints — actual vs estimated counts\n");
    let mut sorted: Vec<_> = actual.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    println!(
        "  {:<35} {:>8} {:>8} {:>8}",
        "Endpoint", "Actual", "Estimate", "Error"
    );
    println!("  {}", "-".repeat(63));

    let mut all_gte = true;
    for (endpoint, &count) in sorted.iter().take(10) {
        let estimate = cms.estimate(endpoint);
        let error = estimate - count;
        if estimate < count {
            all_gte = false;
        }
        println!(
            "  {:<35} {:>8} {:>8} {:>+8}",
            endpoint, count, estimate, error
        );
    }

    // --- Phase 3: Verify the "never undercounts" guarantee ---
    println!("\nPhase 3: Verifying guarantees...");
    println!(
        "  All estimates >= actual: {}",
        if all_gte { "YES (as guaranteed)" } else { "NO (unexpected!)" }
    );

    let max_error: u64 = sorted
        .iter()
        .map(|(ep, &count)| cms.estimate(ep) - count)
        .max()
        .unwrap_or(0);
    println!("  Maximum overcount: +{max_error}");
    println!("  Total requests tracked: {total_requests}");

    println!("\n=== Key Takeaway ===");
    println!("Count-Min sketches use fixed memory to track frequencies of arbitrarily many keys.");
    println!("Estimates may overcount slightly, but never undercount — ideal for rate limiting,");
    println!("hot-path detection, and abuse prevention.");
}
