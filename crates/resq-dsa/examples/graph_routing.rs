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

//! # Graph — City Transit Network Pathfinding
//!
//! Demonstrates BFS (reachability), Dijkstra (shortest weighted path), and
//! A* (heuristic-guided shortest path) on a small city transit network.
//!
//! Run: `cargo run -p resq-dsa --example graph_routing`

use resq_dsa::graph::Graph;

/// Station with a name and (x, y) grid coordinates for the A* heuristic.
struct Station {
    name: &'static str,
    x: i64,
    y: i64,
}

fn main() {
    println!("=== Graph: City Transit Network Pathfinding ===\n");

    // --- Build the transit network ---
    // Stations laid out on a rough grid for the A* heuristic.
    let stations = [
        Station { name: "Airport",       x: 0,  y: 0 },
        Station { name: "Terminal",      x: 2,  y: 0 },
        Station { name: "Downtown",      x: 4,  y: 1 },
        Station { name: "Central",       x: 5,  y: 3 },
        Station { name: "Harbor",        x: 3,  y: 0 },
        Station { name: "Market",        x: 6,  y: 1 },
        Station { name: "Park",          x: 4,  y: 4 },
        Station { name: "University",    x: 8,  y: 4 },
        Station { name: "Stadium",       x: 7,  y: 2 },
        Station { name: "Hospital",      x: 6,  y: 5 },
        Station { name: "Museum",        x: 3,  y: 3 },
        Station { name: "Library",       x: 5,  y: 5 },
        Station { name: "Tech Hub",      x: 9,  y: 3 },
        Station { name: "Suburbs",       x: 10, y: 5 },
        // Disconnected station — no edges lead here.
        Station { name: "Ghost Station", x: 20, y: 20 },
    ];

    // Build coordinate lookup for A* heuristic.
    let coords: std::collections::HashMap<&str, (i64, i64)> = stations
        .iter()
        .map(|s| (s.name, (s.x, s.y)))
        .collect();

    let mut g = Graph::<&str>::new();

    // Edges: (from, to, travel time in minutes). All bidirectional.
    let edges = [
        ("Airport", "Terminal", 5),
        ("Terminal", "Harbor", 3),
        ("Terminal", "Downtown", 7),
        ("Harbor", "Downtown", 6),
        ("Downtown", "Central", 4),
        ("Downtown", "Market", 5),
        ("Central", "Park", 3),
        ("Central", "Museum", 4),
        ("Market", "Stadium", 4),
        ("Market", "University", 8),
        ("Stadium", "University", 3),
        ("Stadium", "Tech Hub", 5),
        ("Park", "Library", 3),
        ("Park", "Hospital", 4),
        ("Library", "Hospital", 2),
        ("University", "Tech Hub", 3),
        ("University", "Suburbs", 6),
        ("Tech Hub", "Suburbs", 4),
        ("Hospital", "University", 5),
        ("Museum", "Park", 2),
    ];

    for &(from, to, weight) in &edges {
        g.add_edge(from, to, weight);
        g.add_edge(to, from, weight);
    }

    println!(
        "Built transit network: {} stations, {} bidirectional routes\n",
        stations.len(),
        edges.len()
    );

    // --- 1. BFS: Discover all reachable stations from Central ---
    println!("--- 1. BFS from \"Central\" ---");
    let visited = g.bfs(&"Central");
    println!("  Reachable stations ({} total):", visited.len());
    for (i, station) in visited.iter().enumerate() {
        print!("  {station}");
        if i < visited.len() - 1 {
            print!(" →");
        }
    }
    println!("\n  Note: \"Ghost Station\" is NOT reachable (no connecting edges)\n");

    // --- 2. Dijkstra: Shortest path from Airport to University ---
    println!("--- 2. Dijkstra: Airport → University ---");
    match g.dijkstra(&"Airport", &"University") {
        Some((path, cost)) => {
            print!("  Path: ");
            for (i, station) in path.iter().enumerate() {
                print!("{station}");
                if i < path.len() - 1 {
                    print!(" → ");
                }
            }
            println!("\n  Total travel time: {cost} minutes");
        }
        None => println!("  No path found!"),
    }

    // --- 3. A*: Same route with Manhattan distance heuristic ---
    println!("\n--- 3. A*: Airport → University (with distance heuristic) ---");
    let goal_coords = coords[&"University"];
    match g.astar(&"Airport", &"University", |node, _goal| {
        let (nx, ny) = coords[node];
        let (gx, gy) = goal_coords;
        ((nx - gx).unsigned_abs() + (ny - gy).unsigned_abs())
    }) {
        Some((path, cost)) => {
            print!("  Path: ");
            for (i, station) in path.iter().enumerate() {
                print!("{station}");
                if i < path.len() - 1 {
                    print!(" → ");
                }
            }
            println!("\n  Total travel time: {cost} minutes");
            println!("  A* finds the same optimal path but explores fewer nodes.");
        }
        None => println!("  No path found!"),
    }

    // --- 4. Unreachable destination ---
    println!("\n--- 4. Dijkstra: Central → Ghost Station ---");
    match g.dijkstra(&"Central", &"Ghost Station") {
        Some((path, cost)) => {
            println!("  Path found: {path:?} (cost: {cost})");
        }
        None => {
            println!("  No path found — Ghost Station is disconnected from the network.");
            println!("  Dijkstra correctly returns None for unreachable destinations.");
        }
    }

    println!("\n=== Key Takeaway ===");
    println!("BFS finds all reachable nodes. Dijkstra finds the shortest weighted path.");
    println!("A* uses a heuristic to guide the search, reaching the goal faster in");
    println!("geographic networks while still guaranteeing optimality.");
}
