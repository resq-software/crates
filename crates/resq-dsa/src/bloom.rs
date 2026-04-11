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

/// A space-efficient probabilistic set membership data structure.
///
/// A Bloom filter can tell you if an element is *possibly* in the set
/// or *definitely* not in the set. False positives are possible (it may
/// say an element is present when it's not), but false negatives are not.
///
/// # Algorithm
///
/// Uses `k` independent FNV-1a hash functions (seeded variants) to set bits
/// in an `m`-bit array. The optimal number of hash functions and bit array
/// size are calculated from the desired capacity and false positive rate.
///
/// # Use Cases
///
/// - Deduplication of IDs, sensor readings, or events
/// - Caching layer to avoid expensive lookups
/// - Quick membership checks before database queries
///
/// # Examples
///
/// ```
/// use resq_dsa::bloom::BloomFilter;
///
/// let mut bf = BloomFilter::new(1000, 0.01); // 1% false positive rate
/// bf.add("drone-001");
/// bf.add("drone-002");
///
/// assert!(bf.has("drone-001")); // definitely present
/// assert!(!bf.has("drone-999")); // definitely not present
/// ```
pub struct BloomFilter {
    bits: Vec<u8>,
    k: usize,
    m: usize,
    count: usize,
}

impl BloomFilter {
    /// Creates a new Bloom filter with the given capacity and error rate.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Expected maximum number of elements to be added
    /// * `error_rate` - Desired false positive probability (must be in (0, 1))
    ///
    /// # Panics
    ///
    /// Panics if `error_rate` is not in `(0, 1)` or `capacity` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::bloom::BloomFilter;
    ///
    /// // Create a filter for 10000 items with 1% false positive rate
    /// let bf = BloomFilter::new(10000, 0.01);
    /// ```
    #[cfg(feature = "std")]
    #[must_use]
    pub fn new(capacity: usize, error_rate: f64) -> Self {
        assert!(
            error_rate > 0.0 && error_rate < 1.0,
            "error_rate must be in (0,1)"
        );
        assert!(capacity > 0, "capacity must be > 0");
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let m = (-(capacity as f64) * error_rate.ln()
            / (core::f64::consts::LN_2 * core::f64::consts::LN_2))
            .ceil() as usize;
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let k = ((m as f64 / capacity as f64) * core::f64::consts::LN_2).round() as usize;
        Self::from_raw_params(m, k.max(1))
    }

    /// Creates a new Bloom filter from pre-computed parameters.
    ///
    /// Use this in `no_std` environments where you pre-compute `m` and `k`
    /// externally. With `std`, prefer [`new`][Self::new] which computes
    /// optimal values automatically.
    ///
    /// # Arguments
    ///
    /// * `bit_count` - Number of bits in the filter (`m`)
    /// * `hash_count` - Number of hash functions (`k`)
    ///
    /// # Panics
    ///
    /// Panics if `bit_count` or `hash_count` is zero.
    #[must_use]
    pub fn from_raw_params(bit_count: usize, hash_count: usize) -> Self {
        assert!(bit_count > 0, "bit_count must be > 0");
        assert!(hash_count > 0, "hash_count must be > 0");
        Self {
            bits: vec![0u8; bit_count.div_ceil(8)],
            k: hash_count,
            m: bit_count,
            count: 0,
        }
    }

    /// FNV-1a hash variant with a seed for producing independent hash functions.
    fn hash(bytes: &[u8], seed: u32, m: usize) -> usize {
        let mut h: u32 = 0x811c_9dc5_u32 ^ seed;
        for &b in bytes {
            h ^= u32::from(b);
            h = h.wrapping_mul(0x0100_0193);
        }
        (h as usize) % m
    }

    /// Adds an element to the filter.
    ///
    /// Accepts any type that can be converted to a byte slice (e.g., `&str`,
    /// `String`, `&[u8]`, `Vec<u8>`).
    ///
    /// After calling `add`, the element will return `true` from `has`
    /// with probability 1 (no false negatives).
    pub fn add(&mut self, item: impl AsRef<[u8]>) {
        let bytes = item.as_ref();
        for i in 0..self.k {
            #[allow(clippy::cast_possible_truncation)]
            let idx = Self::hash(bytes, (i as u32).wrapping_mul(0x9e37_79b9), self.m);
            self.bits[idx >> 3] |= 1 << (idx & 7);
        }
        self.count = self.count.saturating_add(1);
    }

    /// Checks if an element might be in the set.
    ///
    /// Returns `false` if the element is definitely not in the set.
    /// Returns `true` if the element is possibly in the set (may be a false positive).
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::bloom::BloomFilter;
    ///
    /// let mut bf = BloomFilter::new(1000, 0.01);
    /// bf.add("drone-001");
    ///
    /// assert!(bf.has("drone-001")); // definitely present
    /// // bf.has("some-other-id") might return true (false positive)
    /// ```
    #[must_use]
    pub fn has(&self, item: impl AsRef<[u8]>) -> bool {
        let bytes = item.as_ref();
        (0..self.k).all(|i| {
            #[allow(clippy::cast_possible_truncation)]
            let idx = Self::hash(bytes, (i as u32).wrapping_mul(0x9e37_79b9), self.m);
            self.bits[idx >> 3] & (1 << (idx & 7)) != 0
        })
    }

    /// Returns the number of elements that have been added to the filter.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` if no elements have been added.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Resets the filter, removing all elements.
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_false_negatives() {
        let mut bf = BloomFilter::new(500, 0.01);
        let items: Vec<String> = (0..100).map(|i| format!("item-{i}")).collect();
        for x in &items {
            bf.add(x);
        }
        assert!(items.iter().all(|x| bf.has(x.as_str())));
    }

    #[test]
    fn absent_item_false() {
        let mut bf = BloomFilter::new(1000, 0.001);
        bf.add("seen");
        assert!(!bf.has("unseen"));
    }

    #[test]
    fn accepts_byte_slices() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.add(b"bytes" as &[u8]);
        assert!(bf.has(b"bytes" as &[u8]));
        assert!(!bf.has(b"other" as &[u8]));
    }

    #[test]
    fn len_and_clear() {
        let mut bf = BloomFilter::new(100, 0.01);
        assert!(bf.is_empty());
        bf.add("a");
        bf.add("b");
        assert_eq!(bf.len(), 2);
        bf.clear();
        assert!(bf.is_empty());
        assert!(!bf.has("a"));
    }
}
