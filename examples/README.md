# ResQ Examples

Hands-on, runnable examples for every crate in the ResQ workspace.

## Data Structures & Algorithms (`resq-dsa`)

Runnable Rust examples — no external services needed. Execute with:

```bash
cargo run -p resq-dsa --example <name>
```

| Example | Structures Used | Scenario |
|---------|----------------|----------|
| [`bloom_dedup`](../crates/resq-dsa/examples/bloom_dedup.rs) | BloomFilter | Web crawler URL deduplication |
| [`count_min_traffic`](../crates/resq-dsa/examples/count_min_traffic.rs) | CountMinSketch | API traffic frequency analysis |
| [`graph_routing`](../crates/resq-dsa/examples/graph_routing.rs) | Graph (BFS, Dijkstra, A*) | City transit network pathfinding |
| [`heap_knn`](../crates/resq-dsa/examples/heap_knn.rs) | BoundedHeap | K-nearest neighbor search |
| [`trie_autocomplete`](../crates/resq-dsa/examples/trie_autocomplete.rs) | Trie, rabin_karp | CLI command palette autocomplete |
| [`combined_pipeline`](../crates/resq-dsa/examples/combined_pipeline.rs) | **All 5 structures** | Disaster response alert processing pipeline |

### Quick Start

```bash
# Run all examples back-to-back
for ex in bloom_dedup count_min_traffic graph_routing heap_knn trie_autocomplete combined_pipeline; do
  echo "=== $ex ==="
  cargo run -p resq-dsa --example "$ex"
  echo
done
```

## CLI & TUI Tools

Each tool example includes **runnable demo infrastructure** — mock servers, sample data, scripts — so you can see the tools in action without production services.

### Prerequisites

- Rust toolchain (latest stable): `rustup update stable`
- Build all tools: `cargo build --release`
- Python 3 (for mock servers)
- Docker (for `resq-deploy` example)
- GCC (for `resq-bin` example)

### Tool Examples

| Tool | Demo Type | What You Run |
|------|-----------|-------------|
| [resq-cli](resq-cli/) | Sample project with missing headers + planted fake secrets | `resq copyright`, `resq secrets` |
| [resq-bin](resq-bin/) | C program to compile and analyze | `./run_demo.sh` |
| [resq-clean](resq-clean/) | Script that creates a messy workspace | `./create_mess.sh` then `resq-clean` |
| [resq-deploy](resq-deploy/) | Docker Compose with 4 mock services | `docker compose up`, then `resq-deploy` |
| [resq-flame](resq-flame/) | CPU-intensive Python workload | `python3 cpu_burner.py` then profile |
| [resq-health](resq-health/) | 5 mock HTTP services (Python) | `python3 mock_services.py` then `resq-health` |
| [resq-logs](resq-logs/) | Pre-generated log files in 3 formats | `resq-logs --source file --path sample-logs/` |
| [resq-perf](resq-perf/) | Mock `/status` endpoint with changing metrics | `python3 mock_status_server.py` then `resq-perf` |
