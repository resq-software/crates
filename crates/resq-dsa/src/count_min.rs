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

use alloc::vec;
use alloc::vec::Vec;

/// A space-efficient probabilistic data structure for frequency estimation.
///
/// The Count-Min sketch can estimate the frequency of events with guaranteed
/// error bounds. It may overcount but never undercounts (like Bloom filters,
/// this is a property of the algorithm).
///
/// # Algorithm
///
/// Uses `depth` independent FNV-1a hash functions (seeded variants) to map
/// elements to positions in `width` columns. The estimated count is the
/// minimum across all hash table rows.
///
/// # Error Bounds
///
/// - Error is at most `epsilon * total_count` with probability `1 - delta`
/// - Smaller epsilon/delta values require more memory
///
/// # Use Cases
///
/// - Tracking event frequencies
/// - Network traffic analysis
/// - Distributed counting without central aggregation
///
/// # Examples
///
/// ```
/// use resq_dsa::count_min::CountMinSketch;
///
/// let mut cms = CountMinSketch::new(0.01, 0.01);
/// cms.increment("sensor-temp-high", 5);
/// cms.increment("sensor-temp-high", 3);
/// cms.increment("sensor-humidity", 1);
///
/// let temp_estimate = cms.estimate("sensor-temp-high");
/// assert!(temp_estimate >= 8); // never undercounts
/// ```
pub struct CountMinSketch {
    table: Vec<Vec<u64>>,
    width: usize,
    depth: usize,
}

impl CountMinSketch {
    /// Creates a new Count-Min sketch with the given error parameters.
    ///
    /// # Arguments
    ///
    /// * `epsilon` - Error parameter (estimates are within epsilon * N with high probability)
    /// * `delta` - Failure probability (1 - delta is the success probability)
    ///
    /// # Panics
    ///
    /// Panics if epsilon or delta are not in `(0, 1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::count_min::CountMinSketch;
    ///
    /// // Creates a sketch where estimates are within 1% of true count
    /// // with 99% probability
    /// let cms = CountMinSketch::new(0.01, 0.01);
    /// ```
    /// Creates a new Count-Min sketch with the given error bounds.
    ///
    /// Requires the `std` feature for floating-point math. In `no_std`
    /// environments, use [`from_raw_params`][Self::from_raw_params].
    #[cfg(feature = "std")]
    #[must_use]
    pub fn new(epsilon: f64, delta: f64) -> Self {
        assert!(epsilon > 0.0 && epsilon < 1.0, "epsilon must be in (0,1)");
        assert!(delta > 0.0 && delta < 1.0, "delta must be in (0,1)");
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let width = (core::f64::consts::E / epsilon).ceil() as usize;
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let depth = (1.0_f64 / delta).ln().ceil() as usize;
        Self::from_raw_params(width, depth)
    }

    /// Creates a new Count-Min sketch from pre-computed dimensions.
    ///
    /// Use this in `no_std` environments where you pre-compute `width`
    /// and `depth` externally. With `std`, prefer [`new`][Self::new].
    ///
    /// # Arguments
    ///
    /// * `width` - Number of columns (derived from epsilon: `ceil(e / epsilon)`)
    /// * `depth` - Number of rows / hash functions (derived from delta: `ceil(ln(1 / delta))`)
    ///
    /// # Panics
    ///
    /// Panics if `width` or `depth` is zero.
    #[must_use]
    pub fn from_raw_params(width: usize, depth: usize) -> Self {
        assert!(width > 0, "width must be > 0");
        assert!(depth > 0, "depth must be > 0");
        Self {
            table: vec![vec![0u64; width]; depth],
            width,
            depth,
        }
    }

    /// FNV-1a hash variant with a seed for producing independent hash functions.
    fn hash(bytes: &[u8], seed: u32, width: usize) -> usize {
        let mut h: u32 = 0x811c_9dc5_u32 ^ seed;
        for &b in bytes {
            h ^= u32::from(b);
            h = h.wrapping_mul(0x0100_0193);
        }
        (h as usize) % width
    }

    /// Increments the count for a key by the given amount.
    ///
    /// Accepts any type that can be converted to a byte slice (e.g., `&str`,
    /// `String`, `&[u8]`).
    ///
    /// The estimate returned by [`estimate`][Self::estimate] will never
    /// be less than the true count, but may be higher due to hash collisions.
    ///
    /// Note: counts are stored as `u64` and will saturate on overflow.
    pub fn increment(&mut self, key: impl AsRef<[u8]>, count: u64) {
        let bytes = key.as_ref();
        for i in 0..self.depth {
            #[allow(clippy::cast_possible_truncation)]
            let idx = Self::hash(bytes, (i as u32).wrapping_mul(0x9e37_79b9), self.width);
            self.table[i][idx] = self.table[i][idx].saturating_add(count);
        }
    }

    /// Estimates the count for a key.
    ///
    /// Accepts any type that can be converted to a byte slice.
    ///
    /// Returns the minimum value across all hash table rows.
    /// The estimate is guaranteed to be at least the true count,
    /// but may be higher due to hash collisions from other keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::count_min::CountMinSketch;
    ///
    /// let mut cms = CountMinSketch::new(0.01, 0.01);
    /// cms.increment("drone-001", 10);
    /// cms.increment("drone-001", 5);
    ///
    /// let estimate = cms.estimate("drone-001");
    /// assert!(estimate >= 15); // never undercounts
    /// ```
    #[must_use]
    pub fn estimate(&self, key: impl AsRef<[u8]>) -> u64 {
        let bytes = key.as_ref();
        (0..self.depth)
            .map(|i| {
                #[allow(clippy::cast_possible_truncation)]
                let idx = Self::hash(bytes, (i as u32).wrapping_mul(0x9e37_79b9), self.width);
                self.table[i][idx]
            })
            .min()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_frequency() {
        let mut cms = CountMinSketch::new(0.01, 0.01);
        cms.increment("a", 5);
        cms.increment("a", 3);
        cms.increment("b", 1);
        assert!(cms.estimate("a") >= 8);
        assert!(cms.estimate("b") >= 1);
        assert_eq!(cms.estimate("ghost"), 0);
    }

    #[test]
    fn default_increment_one() {
        let mut cms = CountMinSketch::new(0.01, 0.01);
        cms.increment("k", 1);
        cms.increment("k", 1);
        assert!(cms.estimate("k") >= 2);
    }

    #[test]
    fn accepts_byte_slices() {
        let mut cms = CountMinSketch::new(0.01, 0.01);
        cms.increment(b"sensor" as &[u8], 3);
        assert!(cms.estimate(b"sensor" as &[u8]) >= 3);
    }
}
