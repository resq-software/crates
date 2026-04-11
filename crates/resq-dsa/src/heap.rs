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

use alloc::vec::Vec;

/// A bounded max-heap that keeps the K entries with the smallest "distance"
/// values.
///
/// This data structure is useful for tracking the K-nearest neighbors or
/// K-shortest paths. The root always contains the entry with the maximum
/// distance among the kept items. When the heap is full and a new entry has
/// a smaller distance than the root, the root is evicted and the new entry
/// takes its place.
///
/// # Algorithm
///
/// Uses a max-heap where:
/// - The root (index 0) always holds the entry with the *largest* distance
/// - When full, only entries with distance less than the current max can be inserted
/// - Insertion is O(log k) where k is the limit
///
/// # Use Cases
///
/// - K-nearest neighbor search in path planning
/// - Finding K shortest paths
/// - Maintaining top-K results in streaming scenarios
///
/// # Examples
///
/// ```
/// use resq_dsa::heap::BoundedHeap;
///
/// #[derive(Debug)]
/// struct Waypoint { id: u32, distance: f64 }
///
/// let mut h = BoundedHeap::new(3, |w: &Waypoint| w.distance);
///
/// h.insert(Waypoint { id: 1, distance: 10.0 });
/// h.insert(Waypoint { id: 2, distance: 2.0 });
/// h.insert(Waypoint { id: 3, distance: 7.0 });
/// h.insert(Waypoint { id: 4, distance: 1.0 }); // evicts id=1 (dist 10)
/// h.insert(Waypoint { id: 5, distance: 50.0 }); // rejected (too large)
///
/// // Returns the 3 nearest waypoints sorted by distance
/// let sorted: Vec<u32> = h.to_sorted().iter().map(|w| w.id).collect();
/// assert_eq!(sorted, vec![4, 2, 3]); // nearest first
/// ```
pub struct BoundedHeap<T, D: Fn(&T) -> f64> {
    data: Vec<T>,
    limit: usize,
    dist: D,
}

impl<T, D: Fn(&T) -> f64> BoundedHeap<T, D> {
    /// Creates a new bounded heap with the given limit and distance function.
    ///
    /// The distance function can be a function pointer or a closure.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of items to keep
    /// * `dist` - Function (or closure) that returns the "distance" value for ordering
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::heap::BoundedHeap;
    ///
    /// #[derive(Debug)]
    /// struct Node { id: u32, cost: f64 }
    ///
    /// // Works with closures too
    /// let offset = 1.0;
    /// let h = BoundedHeap::new(10, move |n: &Node| n.cost + offset);
    /// ```
    #[must_use]
    pub fn new(limit: usize, dist: D) -> Self {
        Self {
            data: Vec::with_capacity(limit),
            limit,
            dist,
        }
    }

    /// Inserts an entry into the heap.
    ///
    /// If the heap is not full, the entry is added.
    /// If the heap is full and the new entry has a smaller distance than
    /// the current maximum (root), the root is evicted and replaced.
    /// Otherwise, the entry is rejected.
    pub fn insert(&mut self, entry: T) {
        if self.data.len() < self.limit {
            self.data.push(entry);
            let n = self.data.len() - 1;
            self.sift_up(n);
        } else if !self.data.is_empty() && (self.dist)(&entry) < (self.dist)(&self.data[0]) {
            self.data[0] = entry;
            self.sift_down(0);
        }
    }

    /// Returns a reference to the entry with the maximum distance (the root).
    ///
    /// Returns `None` if the heap is empty.
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        self.data.first()
    }

    /// Returns all entries sorted by distance (ascending, nearest first).
    ///
    /// Note: this allocates a new `Vec` and sorts it on every call.
    #[must_use]
    pub fn to_sorted(&self) -> Vec<&T> {
        let mut refs: Vec<&T> = self.data.iter().collect();
        refs.sort_by(|a, b| {
            (self.dist)(a)
                .partial_cmp(&(self.dist)(b))
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        refs
    }

    /// Returns the current number of entries in the heap.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the heap contains no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) / 2;
            if (self.dist)(&self.data[p]) >= (self.dist)(&self.data[i]) {
                break;
            }
            self.data.swap(p, i);
            i = p;
        }
    }

    fn sift_down(&mut self, mut i: usize) {
        let n = self.data.len();
        loop {
            let mut largest = i;
            let l = 2 * i + 1;
            let r = 2 * i + 2;
            if l < n && (self.dist)(&self.data[l]) > (self.dist)(&self.data[largest]) {
                largest = l;
            }
            if r < n && (self.dist)(&self.data[r]) > (self.dist)(&self.data[largest]) {
                largest = r;
            }
            if largest == i {
                break;
            }
            self.data.swap(i, largest);
            i = largest;
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct Item {
        id: u32,
        dist: f64,
    }

    #[test]
    fn keeps_k_nearest() {
        let mut h = BoundedHeap::new(3, |x: &Item| x.dist);
        h.insert(Item { id: 1, dist: 10.0 });
        h.insert(Item { id: 2, dist: 2.0 });
        h.insert(Item { id: 3, dist: 7.0 });
        h.insert(Item { id: 4, dist: 1.0 }); // evicts id=1 (dist 10)
        h.insert(Item { id: 5, dist: 50.0 }); // rejected
        assert_eq!(h.len(), 3);
        let sorted: Vec<u32> = h.to_sorted().iter().map(|x| x.id).collect();
        assert_eq!(sorted, vec![4, 2, 3]);
    }

    #[test]
    fn peek_is_max() {
        let mut h = BoundedHeap::new(2, |x: &Item| x.dist);
        h.insert(Item { id: 1, dist: 5.0 });
        h.insert(Item { id: 2, dist: 3.0 });
        assert_eq!(h.peek().expect("Heap should not be empty").id, 1); // max-distance is root
    }

    #[test]
    fn limit_one() {
        let mut h = BoundedHeap::new(1, |x: &Item| x.dist);
        h.insert(Item { id: 1, dist: 9.0 });
        h.insert(Item { id: 2, dist: 3.0 });
        assert_eq!(h.len(), 1);
        assert_eq!(h.peek().expect("Heap should not be empty").id, 2);
    }

    #[test]
    fn is_empty_check() {
        let h: BoundedHeap<Item, _> = BoundedHeap::new(5, |x: &Item| x.dist);
        assert!(h.is_empty());
    }

    #[test]
    fn works_with_closure() {
        let offset = 1.0;
        let mut h = BoundedHeap::new(2, move |x: &Item| x.dist + offset);
        h.insert(Item { id: 1, dist: 5.0 });
        h.insert(Item { id: 2, dist: 3.0 });
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn limit_zero_rejects_all() {
        let mut h = BoundedHeap::new(0, |x: &Item| x.dist);
        h.insert(Item { id: 1, dist: 1.0 });
        assert!(h.is_empty());
        assert!(h.peek().is_none());
    }

    #[test]
    fn to_sorted_empty() {
        let h: BoundedHeap<Item, _> = BoundedHeap::new(5, |x: &Item| x.dist);
        assert!(h.to_sorted().is_empty());
    }

    #[test]
    fn negative_distances() {
        let mut h = BoundedHeap::new(2, |x: &Item| x.dist);
        h.insert(Item { id: 1, dist: -10.0 });
        h.insert(Item { id: 2, dist: -20.0 });
        h.insert(Item { id: 3, dist: -5.0 });
        // Keeps the 2 smallest distances: -20 and -10
        assert_eq!(h.len(), 2);
        let sorted: Vec<u32> = h.to_sorted().iter().map(|x| x.id).collect();
        assert_eq!(sorted, vec![2, 1]);
    }

    #[test]
    fn identical_distances() {
        let mut h = BoundedHeap::new(3, |x: &Item| x.dist);
        for i in 0..5 {
            h.insert(Item { id: i, dist: 1.0 });
        }
        // All same distance — first 3 kept, rest rejected (not smaller than root).
        assert_eq!(h.len(), 3);
        let mut kept: Vec<u32> = h.to_sorted().iter().map(|x| x.id).collect();
        kept.sort_unstable();
        assert_eq!(kept, vec![0, 1, 2]);
    }
}
