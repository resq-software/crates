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

//! # Combined Pipeline — Disaster Response Alert Processing
//!
//! Uses **all five** resq-dsa data structures together in a realistic scenario:
//! processing incoming sensor alerts for a disaster response system.
//!
//! 1. **BloomFilter** — deduplicate alert IDs from unreliable sensor streams
//! 2. **CountMinSketch** — track alert-type frequencies in fixed memory
//! 3. **Graph** — find optimal response routes between zones
//! 4. **BoundedHeap** — keep the top-5 most critical alerts for dispatch
//! 5. **Trie + rabin_karp** — match alerts to response protocols
//!
//! Run: `cargo run -p resq-dsa --example combined_pipeline`

use resq_dsa::bloom::BloomFilter;
use resq_dsa::count_min::CountMinSketch;
use resq_dsa::graph::Graph;
use resq_dsa::heap::BoundedHeap;
use resq_dsa::trie::{rabin_karp, Trie};

/// Simple LCG for deterministic "random" data.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        self.0
    }
    fn next_bound(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

#[derive(Debug)]
struct Alert {
    id: String,
    alert_type: &'static str,
    zone: &'static str,
    severity: u32,
    description: String,
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Disaster Response Alert Processing Pipeline           ║");
    println!("║   Using all 5 resq-dsa data structures                  ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    let alert_types = ["fire", "flood", "earthquake", "chemical", "structural", "medical"];
    let zones = ["Zone-A", "Zone-B", "Zone-C", "Zone-D", "Zone-E"];
    let mut rng = Lcg::new(2026);

    // Generate 5,000 alerts where ~20% are duplicates.
    let total_raw = 5_000u32;
    let mut alerts: Vec<Alert> = Vec::new();
    for i in 0..total_raw {
        // ~20% of alerts reuse an earlier ID (simulating sensor retransmission).
        let id = if i > 100 && rng.next_bound(5) == 0 {
            format!("ALERT-{:05}", rng.next_bound(i as u64))
        } else {
            format!("ALERT-{i:05}")
        };
        let atype = alert_types[rng.next_bound(alert_types.len() as u64) as usize];
        let zone = zones[rng.next_bound(zones.len() as u64) as usize];
        let severity = (rng.next_bound(10) + 1) as u32;
        let description = format!("{atype} detected in {zone} — severity {severity}");
        alerts.push(Alert { id, alert_type: atype, zone, severity, description });
    }

    // ══════════════════════════════════════════════════════════════
    // Stage 1: DEDUP with BloomFilter
    // ══════════════════════════════════════════════════════════════
    println!("━━━ Stage 1: Deduplication (BloomFilter) ━━━");
    let mut bloom = BloomFilter::new(total_raw as usize, 0.01);
    let mut unique_alerts: Vec<&Alert> = Vec::new();
    let mut duplicates = 0u32;

    for alert in &alerts {
        if bloom.has(&alert.id) {
            duplicates += 1;
        } else {
            bloom.add(&alert.id);
            unique_alerts.push(alert);
        }
    }
    println!("  Raw alerts:    {total_raw}");
    println!("  Duplicates:    {duplicates}");
    println!("  Unique alerts: {}\n", unique_alerts.len());

    // ══════════════════════════════════════════════════════════════
    // Stage 2: FREQUENCY with CountMinSketch
    // ══════════════════════════════════════════════════════════════
    println!("━━━ Stage 2: Frequency Analysis (CountMinSketch) ━━━");
    let mut cms = CountMinSketch::new(0.001, 0.01);
    let mut actual_counts: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();

    for alert in &unique_alerts {
        cms.increment(alert.alert_type, 1);
        *actual_counts.entry(alert.alert_type).or_default() += 1;
    }

    println!("  {:<15} {:>8} {:>8}", "Type", "Actual", "Estimate");
    println!("  {}", "-".repeat(33));
    let mut sorted_types: Vec<_> = actual_counts.iter().collect();
    sorted_types.sort_by(|a, b| b.1.cmp(a.1));
    for (atype, &count) in &sorted_types {
        let est = cms.estimate(atype);
        println!("  {:<15} {:>8} {:>8}", atype, count, est);
    }
    let most_common = sorted_types[0].0;
    println!("\n  Most common alert type: \"{most_common}\"\n");

    // ══════════════════════════════════════════════════════════════
    // Stage 3: ROUTING with Graph
    // ══════════════════════════════════════════════════════════════
    println!("━━━ Stage 3: Response Routing (Graph) ━━━");
    let mut graph = Graph::<&str>::new();

    // Build a response zone network.
    let routes = [
        ("HQ", "Zone-A", 10),
        ("HQ", "Zone-B", 15),
        ("Zone-A", "Zone-B", 5),
        ("Zone-A", "Zone-C", 12),
        ("Zone-B", "Zone-C", 8),
        ("Zone-B", "Zone-D", 20),
        ("Zone-C", "Zone-D", 6),
        ("Zone-C", "Zone-E", 14),
        ("Zone-D", "Zone-E", 4),
    ];
    for &(from, to, cost) in &routes {
        graph.add_edge(from, to, cost);
        graph.add_edge(to, from, cost);
    }

    // Route responders to the zone with the most common alert type.
    let target_zone = unique_alerts
        .iter()
        .filter(|a| a.alert_type == *most_common)
        .next()
        .map(|a| a.zone)
        .unwrap_or("Zone-A");

    println!("  Routing from HQ to {target_zone} (highest-frequency alert zone)...");
    match graph.dijkstra(&"HQ", &target_zone) {
        Some((path, cost)) => {
            let path_str: Vec<_> = path.iter().map(|s| *s).collect();
            println!("  Route: {}", path_str.join(" → "));
            println!("  Travel time: {cost} minutes");
        }
        None => println!("  No route found!"),
    }

    // BFS to show all reachable zones.
    let reachable = graph.bfs(&"HQ");
    println!("  All reachable zones from HQ: {}\n", reachable.join(", "));

    // ══════════════════════════════════════════════════════════════
    // Stage 4: PRIORITIZATION with BoundedHeap
    // ══════════════════════════════════════════════════════════════
    println!("━━━ Stage 4: Priority Dispatch (BoundedHeap) ━━━");
    let top_k = 5;

    // Score = severity * frequency_estimate (higher = more critical).
    // BoundedHeap keeps items with the *smallest* distance, so we use
    // negative score as the "distance" to keep the highest-scored alerts.
    let mut heap = BoundedHeap::new(top_k, |a: &&Alert| {
        let freq = cms.estimate(a.alert_type);
        let score = a.severity as f64 * freq as f64;
        -score // Negate so BoundedHeap keeps highest scores.
    });

    for alert in &unique_alerts {
        heap.insert(&alert);
    }

    println!("  Top {top_k} most critical alerts for dispatch:");
    let prioritized = heap.to_sorted();
    for (i, alert) in prioritized.iter().enumerate() {
        let freq = cms.estimate(alert.alert_type);
        let score = alert.severity as f64 * freq as f64;
        println!(
            "    #{}: [{}] {} (severity: {}, freq: ~{}, score: {:.0})",
            i + 1,
            alert.id,
            alert.description,
            alert.severity,
            freq,
            score,
        );
    }
    println!();

    // ══════════════════════════════════════════════════════════════
    // Stage 5: PROTOCOL MATCHING with Trie + Rabin-Karp
    // ══════════════════════════════════════════════════════════════
    println!("━━━ Stage 5: Protocol Matching (Trie + Rabin-Karp) ━━━");
    let mut protocols = Trie::new();
    let protocol_list = [
        "fire:evacuate",
        "fire:suppress",
        "fire:contain",
        "flood:sandbar",
        "flood:evacuate",
        "flood:pump",
        "earthquake:assess",
        "earthquake:rescue",
        "earthquake:evacuate",
        "chemical:hazmat",
        "chemical:evacuate",
        "chemical:contain",
        "structural:shore",
        "structural:evacuate",
        "medical:triage",
        "medical:transport",
    ];
    for p in &protocol_list {
        protocols.insert(p);
    }

    // Find protocols for the most common alert type.
    let prefix = format!("{most_common}:");
    let mut matching = protocols.starts_with(&prefix);
    matching.sort();
    println!("  Protocols for \"{most_common}\" alerts:");
    for p in &matching {
        println!("    • {p}");
    }

    // Use Rabin-Karp to scan the top alert's description for keywords.
    if let Some(top_alert) = prioritized.first() {
        println!("\n  Scanning top alert description for keywords...");
        println!("  Text: \"{}\"", top_alert.description);
        for keyword in &["fire", "flood", "earthquake", "chemical", "structural", "medical"] {
            let hits = rabin_karp(&top_alert.description, keyword);
            if !hits.is_empty() {
                println!("    Found \"{keyword}\" at positions: {hits:?}");
            }
        }
    }

    // ══════════════════════════════════════════════════════════════
    // Summary
    // ══════════════════════════════════════════════════════════════
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   Pipeline Summary                                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  BloomFilter:     {duplicates:>5} duplicates filtered              ║");
    println!("║  CountMinSketch:  {:>5} unique alerts tracked            ║", unique_alerts.len());
    println!("║  Graph:           {:>5} zones reachable from HQ          ║", reachable.len());
    println!("║  BoundedHeap:     {:>5} top alerts for dispatch          ║", top_k);
    println!("║  Trie:            {:>5} response protocols loaded        ║", protocol_list.len());
    println!("╚══════════════════════════════════════════════════════════╝");
}
