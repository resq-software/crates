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

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::hash::Hash;

/// Reconstructs a path from a predecessor map, walking backwards from `end`.
fn reconstruct_path<Id: Eq + Hash + Clone>(prev: &HashMap<Id, Id>, end: &Id) -> Vec<Id> {
    let mut path = Vec::new();
    let mut cur = end.clone();
    loop {
        path.push(cur.clone());
        match prev.get(&cur) {
            Some(p) => cur = p.clone(),
            None => break,
        }
    }
    path.reverse();
    path
}

/// Graph data structure with pathfinding algorithms.
///
/// Provides BFS, Dijkstra's algorithm, and A* for finding shortest paths
/// in weighted directed graphs.
///
/// # Type Parameters
///
/// - `Id`: Node identifier (must be hashable, clonable, comparable)
///
/// # Use Cases
///
/// - Flight path planning
/// - Network routing
/// - Game pathfinding
///
/// # Examples
///
/// ```
/// use resq_dsa::graph::Graph;
///
/// let mut g = Graph::<&str>::new();
///
/// // Add edges (from, to, weight)
/// g.add_edge("base", "waypoint-1", 100);
/// g.add_edge("waypoint-1", "waypoint-2", 50);
/// g.add_edge("base", "waypoint-2", 150);
/// g.add_edge("waypoint-2", "target", 25);
///
/// // BFS - visit all reachable nodes
/// let nodes = g.bfs(&"base");
/// assert!(nodes.contains(&"target"));
///
/// // Dijkstra - shortest path (175 via waypoint-2)
/// let (path, cost) = g.dijkstra(&"base", &"target").unwrap();
/// assert_eq!(cost, 175);
/// ```
pub struct Graph<Id: Eq + Hash + Clone> {
    adj: HashMap<Id, Vec<(Id, u64)>>,
}

impl<Id: Eq + Hash + Clone> Graph<Id> {
    /// Creates a new empty directed graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adj: HashMap::new(),
        }
    }

    /// Adds a directed edge from `from` to `to` with the given weight.
    ///
    /// Note: This creates a directed graph. For undirected edges,
    /// call `add_edge` twice with the nodes reversed.
    pub fn add_edge(&mut self, from: Id, to: Id, weight: u64) {
        self.adj.entry(from).or_default().push((to, weight));
    }

    /// Performs breadth-first search starting from `start`.
    ///
    /// Returns all nodes reachable from `start` in BFS order.
    /// Time complexity: O(V + E) where V is vertices and E is edges.
    ///
    /// Note: only nodes with outgoing edges (or explicitly added as edge
    /// sources/targets) will appear in the result.
    pub fn bfs(&self, start: &Id) -> Vec<Id> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();
        visited.insert(start.clone());
        queue.push_back(start.clone());
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            if let Some(neighbors) = self.adj.get(&node) {
                for (to, _) in neighbors {
                    if visited.insert(to.clone()) {
                        queue.push_back(to.clone());
                    }
                }
            }
        }
        result
    }
}

impl<Id: Eq + Hash + Clone + Ord> Graph<Id> {
    /// Finds the shortest path using Dijkstra's algorithm.
    ///
    /// Returns `Some((path, cost))` if a path exists, or `None` if unreachable.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting node
    /// * `end` - Target node
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::graph::Graph;
    ///
    /// let mut g = Graph::<&str>::new();
    /// g.add_edge("A", "B", 1);
    /// g.add_edge("A", "C", 4);
    /// g.add_edge("B", "C", 2);
    /// g.add_edge("C", "D", 1);
    ///
    /// let (path, cost) = g.dijkstra(&"A", &"D").unwrap();
    /// assert_eq!(path, vec!["A", "B", "C", "D"]);
    /// assert_eq!(cost, 4);
    /// ```
    pub fn dijkstra(&self, start: &Id, end: &Id) -> Option<(Vec<Id>, u64)> {
        let mut dist: HashMap<Id, u64> = HashMap::new();
        let mut prev: HashMap<Id, Id> = HashMap::new();
        let mut pq: BinaryHeap<Reverse<(u64, Id)>> = BinaryHeap::new();
        dist.insert(start.clone(), 0);
        pq.push(Reverse((0, start.clone())));
        while let Some(Reverse((d, u))) = pq.pop() {
            if &u == end {
                break;
            }
            if d > *dist.get(&u).unwrap_or(&u64::MAX) {
                continue;
            }
            if let Some(neighbors) = self.adj.get(&u) {
                for (v, w) in neighbors {
                    let alt = d.saturating_add(*w);
                    if alt < *dist.get(v).unwrap_or(&u64::MAX) {
                        dist.insert(v.clone(), alt);
                        prev.insert(v.clone(), u.clone());
                        pq.push(Reverse((alt, v.clone())));
                    }
                }
            }
        }
        let cost = *dist.get(end)?;
        Some((reconstruct_path(&prev, end), cost))
    }

    /// Finds the shortest path using A* algorithm.
    ///
    /// A* uses a heuristic to guide the search and can be faster than Dijkstra
    /// when a good heuristic is available.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting node
    /// * `end` - Target node
    /// * `h` - Heuristic function estimating cost from node to end
    ///
    /// # Heuristic Requirements
    ///
    /// For correct results, the heuristic must be:
    /// - Admissible: never overestimate the true cost
    /// - Consistent: for all edges, h(u) <= cost(u,v) + h(v)
    ///
    /// # Examples
    ///
    /// ```
    /// use resq_dsa::graph::Graph;
    ///
    /// let mut g = Graph::<u64>::new();
    /// g.add_edge(0, 1, 1);
    /// g.add_edge(1, 2, 1);
    /// g.add_edge(0, 3, 10);
    /// g.add_edge(3, 2, 1);
    ///
    /// // Euclidean distance as heuristic
    /// let (path, cost) = g.astar(&0, &2, |a, b| a.abs_diff(*b)).unwrap();
    /// assert_eq!(path, vec![0, 1, 2]); // faster route via 1
    /// assert_eq!(cost, 2);
    /// ```
    pub fn astar<H: Fn(&Id, &Id) -> u64>(
        &self,
        start: &Id,
        end: &Id,
        h: H,
    ) -> Option<(Vec<Id>, u64)> {
        let mut g_score: HashMap<Id, u64> = HashMap::new();
        let mut prev: HashMap<Id, Id> = HashMap::new();
        let mut pq: BinaryHeap<Reverse<(u64, Id)>> = BinaryHeap::new();
        g_score.insert(start.clone(), 0);
        pq.push(Reverse((h(start, end), start.clone())));
        while let Some(Reverse((_, u))) = pq.pop() {
            if &u == end {
                return Some((reconstruct_path(&prev, end), *g_score.get(end)?));
            }
            let cost = *g_score.get(&u).unwrap_or(&u64::MAX);
            if let Some(neighbors) = self.adj.get(&u) {
                for (v, w) in neighbors {
                    let alt = cost.saturating_add(*w);
                    if alt < *g_score.get(v).unwrap_or(&u64::MAX) {
                        g_score.insert(v.clone(), alt);
                        prev.insert(v.clone(), u.clone());
                        pq.push(Reverse((alt.saturating_add(h(v, end)), v.clone())));
                    }
                }
            }
        }
        None
    }
}

impl<Id: Eq + Hash + Clone> Default for Graph<Id> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn bfs_visits_all() {
        let mut g = Graph::<&str>::new();
        g.add_edge("A", "B", 1);
        g.add_edge("A", "C", 1);
        g.add_edge("B", "D", 1);
        assert_eq!(g.bfs(&"A"), vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn dijkstra_shortest_path() {
        let mut g = Graph::<&str>::new();
        g.add_edge("A", "B", 1);
        g.add_edge("A", "C", 4);
        g.add_edge("B", "C", 2);
        g.add_edge("C", "D", 1);
        let (path, cost) = g.dijkstra(&"A", &"D").expect("Path A->D should exist");
        assert_eq!(path, vec!["A", "B", "C", "D"]);
        assert_eq!(cost, 4);
    }

    #[test]
    fn dijkstra_unreachable() {
        let mut g = Graph::<&str>::new();
        g.add_edge("A", "B", 1);
        assert!(g.dijkstra(&"A", &"Z").is_none());
    }

    #[test]
    fn astar_finds_path() {
        let mut g = Graph::<u64>::new();
        g.add_edge(0, 1, 1);
        g.add_edge(1, 2, 1);
        g.add_edge(0, 3, 10);
        g.add_edge(3, 2, 1);
        let (path, cost) = g
            .astar(&0, &2, |a, b| a.abs_diff(*b))
            .expect("Path 0->2 should exist");
        assert_eq!(path, vec![0, 1, 2]);
        assert_eq!(cost, 2);
    }
}
