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

//! Algorithmic complexity verification tests.
//!
//! These tests measure execution time at three input sizes (N, 2N, 4N) and
//! verify that the observed growth ratios are consistent with the expected
//! Big-O complexity class, ruling out worse-than-expected behavior (e.g.,
//! an accidentally quadratic algorithm).
//!
//! Approach: measure time for sizes N, 2N, and 4N; compute ratios.
//! For O(n) total work: T(2N)/T(N) ~ 2.0, T(4N)/T(N) ~ 4.0.
//! We assert T(4N)/T(N) < threshold to rule out quadratic behavior.
//!
//! We include a warmup pass before timing and take the median of multiple
//! trials to reduce noise from CPU caches, allocators, and scheduling.

use std::time::Instant;

use resq_dsa::bloom::BloomFilter;
use resq_dsa::count_min::CountMinSketch;
use resq_dsa::graph::Graph;
use resq_dsa::heap::BoundedHeap;
use resq_dsa::trie::Trie;

/// Number of timed trials per measurement (take median).
const TRIALS: usize = 7;

/// Measures the median execution time (in nanoseconds) of `f` over `TRIALS`
/// runs. Includes one warmup invocation that is discarded.
fn measure_median_ns<F: FnMut()>(mut f: F) -> f64 {
    // Warmup (discarded)
    f();

    let mut times = Vec::with_capacity(TRIALS);
    for _ in 0..TRIALS {
        let start = Instant::now();
        f();
        times.push(start.elapsed().as_nanos() as f64);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times[TRIALS / 2]
}

/// Asserts that doubling/quadrupling input does not cause super-linear blowup
/// beyond what the stated complexity class allows.
///
/// For an algorithm whose total work over n items is O(n) or O(n log n):
///   - T(4N)/T(N) should be well below 16 (which would indicate O(n^2)).
///   - We use a generous upper bound to tolerate system noise.
fn assert_not_quadratic(name: &str, t_n: f64, t_4n: f64, max_ratio_4x: f64) {
    let ratio = t_4n / t_n;
    assert!(
        ratio <= max_ratio_4x,
        "{name}: T(4N)/T(N) = {ratio:.2} exceeds max allowed {max_ratio_4x:.1} \
         (t_n={t_n:.0}ns, t_4n={t_4n:.0}ns) -- possible worse-than-expected complexity"
    );
}

// ---------------------------------------------------------------------------
// BloomFilter tests -- O(1) per add/has, total O(n) for n operations
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn bloom_filter_insert_linear_scaling() {
    let n = 20_000_usize;

    let t_n = measure_median_ns(|| {
        let mut bf = BloomFilter::new(n, 0.01);
        for i in 0..n {
            bf.add(i.to_string());
        }
    });

    let t_4n = measure_median_ns(|| {
        let mut bf = BloomFilter::new(4 * n, 0.01);
        for i in 0..(4 * n) {
            bf.add(i.to_string());
        }
    });

    // O(n) total: T(4N)/T(N) should be ~4, certainly < 10
    assert_not_quadratic("BloomFilter::add", t_n, t_4n, 10.0);
}

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn bloom_filter_lookup_linear_scaling() {
    let n = 20_000_usize;

    let mut bf_n = BloomFilter::new(n, 0.01);
    for i in 0..n {
        bf_n.add(i.to_string());
    }
    let mut bf_4n = BloomFilter::new(4 * n, 0.01);
    for i in 0..(4 * n) {
        bf_4n.add(i.to_string());
    }

    let t_n = measure_median_ns(|| {
        for i in 0..n {
            std::hint::black_box(bf_n.has(i.to_string()));
        }
    });

    let t_4n = measure_median_ns(|| {
        for i in 0..(4 * n) {
            std::hint::black_box(bf_4n.has(i.to_string()));
        }
    });

    assert_not_quadratic("BloomFilter::has", t_n, t_4n, 10.0);
}

// ---------------------------------------------------------------------------
// BoundedHeap -- n inserts each O(log n), total O(n log n)
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn bounded_heap_insert_nlogn_scaling() {
    let n = 20_000_usize;

    let t_n = measure_median_ns(|| {
        let mut heap = BoundedHeap::new(n, |x: &f64| *x);
        for i in 0..n {
            heap.insert(i as f64);
        }
    });

    let t_4n = measure_median_ns(|| {
        let mut heap = BoundedHeap::new(4 * n, |x: &f64| *x);
        for i in 0..(4 * n) {
            heap.insert(i as f64);
        }
    });

    // O(n log n) total: T(4N)/T(N) ~ 4 * log(4N)/log(N) ~ 4.5-5.5
    // Certainly < 12 (quadratic would be 16)
    assert_not_quadratic("BoundedHeap::insert", t_n, t_4n, 12.0);
}

// ---------------------------------------------------------------------------
// Graph Dijkstra -- O(E log V), chain graph: E = V-1
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn graph_dijkstra_nlogn_scaling() {
    let n = 2_000_usize;

    let build_chain = |size: usize| -> Graph<usize> {
        let mut g = Graph::new();
        for i in 0..(size - 1) {
            g.add_edge(i, i + 1, 1);
        }
        g
    };

    let g_n = build_chain(n);
    let g_4n = build_chain(4 * n);

    let t_n = measure_median_ns(|| {
        std::hint::black_box(g_n.dijkstra(&0, &(n - 1)));
    });

    let t_4n = measure_median_ns(|| {
        std::hint::black_box(g_4n.dijkstra(&0, &(4 * n - 1)));
    });

    // O(n log n) for chain: T(4N)/T(N) < 12
    assert_not_quadratic("Graph::dijkstra", t_n, t_4n, 12.0);
}

// ---------------------------------------------------------------------------
// Graph BFS -- O(V + E), chain graph: O(n)
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn graph_bfs_linear_scaling() {
    let n = 2_000_usize;

    let build_chain = |size: usize| -> Graph<usize> {
        let mut g = Graph::new();
        for i in 0..(size - 1) {
            g.add_edge(i, i + 1, 1);
        }
        g
    };

    let g_n = build_chain(n);
    let g_4n = build_chain(4 * n);

    let t_n = measure_median_ns(|| {
        std::hint::black_box(g_n.bfs(&0));
    });

    let t_4n = measure_median_ns(|| {
        std::hint::black_box(g_4n.bfs(&0));
    });

    // O(V + E) = O(n): T(4N)/T(N) < 12 (generous for allocation overhead)
    assert_not_quadratic("Graph::bfs", t_n, t_4n, 12.0);
}

// ---------------------------------------------------------------------------
// Trie insert/search -- O(L) per op, fixed L => total O(n)
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn trie_insert_linear_scaling() {
    let n = 20_000_usize;

    let t_n = measure_median_ns(|| {
        let mut trie = Trie::new();
        for i in 0..n {
            trie.insert(&format!("key-{i:08}"));
        }
    });

    let t_4n = measure_median_ns(|| {
        let mut trie = Trie::new();
        for i in 0..(4 * n) {
            trie.insert(&format!("key-{i:08}"));
        }
    });

    assert_not_quadratic("Trie::insert", t_n, t_4n, 10.0);
}

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn trie_search_linear_scaling() {
    let n = 20_000_usize;

    let mut trie_n = Trie::new();
    for i in 0..n {
        trie_n.insert(&format!("key-{i:08}"));
    }
    let mut trie_4n = Trie::new();
    for i in 0..(4 * n) {
        trie_4n.insert(&format!("key-{i:08}"));
    }

    let t_n = measure_median_ns(|| {
        for i in 0..n {
            std::hint::black_box(trie_n.search(&format!("key-{i:08}")));
        }
    });

    let t_4n = measure_median_ns(|| {
        for i in 0..(4 * n) {
            std::hint::black_box(trie_4n.search(&format!("key-{i:08}")));
        }
    });

    assert_not_quadratic("Trie::search", t_n, t_4n, 10.0);
}

// ---------------------------------------------------------------------------
// CountMinSketch increment/estimate -- O(d) per op, d constant => O(1) per op
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn count_min_increment_linear_scaling() {
    let n = 20_000_usize;

    let t_n = measure_median_ns(|| {
        let mut cms = CountMinSketch::new(0.01, 0.01);
        for i in 0..n {
            cms.increment(i.to_string(), 1);
        }
    });

    let t_4n = measure_median_ns(|| {
        let mut cms = CountMinSketch::new(0.01, 0.01);
        for i in 0..(4 * n) {
            cms.increment(i.to_string(), 1);
        }
    });

    assert_not_quadratic("CountMinSketch::increment", t_n, t_4n, 10.0);
}

#[test]
#[ignore] // Timing-sensitive — run locally with: cargo test -p resq-dsa -- --ignored
fn count_min_estimate_linear_scaling() {
    let n = 20_000_usize;

    let mut cms_n = CountMinSketch::new(0.01, 0.01);
    for i in 0..n {
        cms_n.increment(i.to_string(), 1);
    }
    let mut cms_4n = CountMinSketch::new(0.01, 0.01);
    for i in 0..(4 * n) {
        cms_4n.increment(i.to_string(), 1);
    }

    let t_n = measure_median_ns(|| {
        for i in 0..n {
            std::hint::black_box(cms_n.estimate(i.to_string()));
        }
    });

    let t_4n = measure_median_ns(|| {
        for i in 0..(4 * n) {
            std::hint::black_box(cms_4n.estimate(i.to_string()));
        }
    });

    assert_not_quadratic("CountMinSketch::estimate", t_n, t_4n, 10.0);
}
