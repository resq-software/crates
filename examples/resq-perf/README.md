# resq-perf Examples

Real-time performance monitoring dashboard that polls `/status` endpoints for memory, heap, and latency metrics.

## Demo: Mock Status Server

[`mock_status_server.py`](mock_status_server.py) serves a `/admin/status` endpoint that returns the exact JSON schema resq-perf expects, with gradually changing metrics that simulate real service behavior.

### Simulated behaviors

- **Memory:** slow leak (~1MB every 10s) + sawtooth GC pattern (grows then drops every ~20s)
- **Latency:** ~45ms base with sinusoidal jitter + occasional spikes every ~37s
- **Objects:** gradually increasing count (simulates object accumulation)
- **Uptime:** real elapsed time since server start

### Run it

```bash
# Terminal 1: Start mock status server
python3 examples/resq-perf/mock_status_server.py

# Terminal 2: Connect resq-perf
cargo run -p resq-perf -- http://localhost:3000/admin/status

# High-frequency mode for detailed observation
cargo run -p resq-perf -- http://localhost:3000/admin/status --refresh-ms 200

# Custom port
python3 examples/resq-perf/mock_status_server.py --port 5000
cargo run -p resq-perf -- http://localhost:5000/admin/status
```

### What you'll see

A Ratatui dashboard with:
- **Process Memory panel:** RSS, heapUsed, heapTotal, external — watch RSS grow slowly
- **Heap Metrics panel:** object counts, capacity — counts increase over time
- **Latency sparkline:** a rolling chart showing request latency — look for the periodic spikes
- **Service info:** uptime ticking up, version "2.1.0", environment "development"

### What to look for

1. **Memory leak:** RSS and heapUsed grow over minutes — this is the simulated leak
2. **GC sawtooth:** heapUsed rises then drops every ~20s — simulated garbage collection
3. **Latency spikes:** the sparkline chart shows periodic bumps every ~37s
4. **Pause/resume:** press `p` to freeze updates, press again to resume

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `p` | Pause/resume polling |
| `q` | Quit |
