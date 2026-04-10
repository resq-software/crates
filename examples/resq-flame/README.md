# resq-flame Examples

CPU flame graph profiler that generates interactive SVG visualizations.

## Demo: CPU-Intensive Python Workload

[`cpu_burner.py`](cpu_burner.py) runs four distinct computation patterns that create clear, recognizable shapes in a flame graph:

| Function | Pattern | What it looks like in the flame graph |
|----------|---------|--------------------------------------|
| `fibonacci(28)` | Recursive | Deep, narrow stacks (many nested calls) |
| `matrix_multiply(50)` | Triple nested loop | Wide, shallow bars (hot inner loop) |
| `sorting_workload(50000)` | Comparison sort | Medium-depth timsort internals |
| `hashing_workload(10000)` | SHA-256 iterations | Narrow tower of crypto calls |

### Run it

```bash
# Terminal 1: Start the workload (runs until Ctrl+C)
python3 examples/resq-flame/cpu_burner.py
```

The script prints its PID and throughput stats. Use one of these methods to profile it:

**Method A: py-spy (recommended for Python)**

```bash
# Install py-spy
pip install py-spy

# Terminal 2: Record a flame graph
sudo py-spy record -o flamegraph.svg --pid $(pgrep -f cpu_burner) --duration 10

# Open the SVG
open flamegraph.svg    # macOS
xdg-open flamegraph.svg  # Linux
```

**Method B: resq-flame TUI**

```bash
# Launch the TUI and select a profiling target
cargo run -p resq-flame
```

**Method C: resq-flame HCE subcommand (for Node.js services)**

```bash
# If you have the docker-compose services running (from resq-deploy example):
cargo run -p resq-flame -- hce --url http://localhost:5000 --duration 5000 --open
```

### What you'll see in the flame graph

An interactive SVG where:
- **X-axis** = proportion of CPU time (wider = more CPU)
- **Y-axis** = call stack depth (deeper = more nesting)
- **Click** to zoom into any stack frame
- **Hover** for exact percentages

Look for:
- `fibonacci` — tall narrow tower on one side (recursive depth)
- `matrix_multiply` — wide band in the middle (inner loop dominates)
- `sorting_workload` — medium blocks from Python's timsort
- `hashing_workload` — narrow column of `hashlib` calls

### Before & After optimization example

```bash
# 1. Profile the original workload
sudo py-spy record -o before.svg --pid $(pgrep -f cpu_burner) --duration 10

# 2. Edit cpu_burner.py: change fibonacci(28) to fibonacci(20)
# 3. Restart cpu_burner.py

# 4. Profile again
sudo py-spy record -o after.svg --pid $(pgrep -f cpu_burner) --duration 10

# 5. Compare: fibonacci's bar should be dramatically smaller
open before.svg after.svg
```
