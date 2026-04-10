# resq-logs Examples

Multi-source log aggregator that streams and filters logs from Docker containers or local files.

## Demo: Sample Log Files

Pre-generated log files in all three formats resq-logs understands are included in [`sample-logs/`](sample-logs/). You can also regenerate them or run in live-streaming mode.

### Log formats included

| File | Format | What it simulates |
|------|--------|-------------------|
| `api.log` | JSON structured | API gateway request logs with method, path, status, latency |
| `worker.log` | RUST_LOG | Background job worker with module paths and log levels |
| `db.log` | Plain text | Database query logs with timing and connection pool stats |

### Run it

```bash
# View all logs from the sample files
cargo run -p resq-logs -- --source file --path examples/resq-logs/sample-logs/

# Filter to only errors
cargo run -p resq-logs -- --source file --path examples/resq-logs/sample-logs/ --level error

# View just one file
cargo run -p resq-logs -- --source file --path examples/resq-logs/sample-logs/api.log
```

### Regenerate or stream live

```bash
# Regenerate sample logs (fresh timestamps)
python3 examples/resq-logs/generate_logs.py

# Generate more lines
python3 examples/resq-logs/generate_logs.py --lines 1000

# Live mode: continuously append new log entries (tail -f style)
python3 examples/resq-logs/generate_logs.py --live
```

In live mode, the generator appends a new log line every 0.1-1.0 seconds across all three files, so you can watch resq-logs pick up new entries in real time.

### What you'll see

**TUI showing:**
- Color-coded service names (api, worker, db — each gets a distinct color)
- Columns: timestamp, log level, service name, message
- Levels color-coded: ERROR (red), WARN (yellow), INFO (blue), DEBUG (gray)
- Use `/` to search for specific text (e.g., search for "timeout" or "500")

### Sample log formats

**JSON (api.log):**
```json
{"timestamp":"2026-04-10T14:30:01.234Z","level":"error","service":"api","msg":"POST /api/v1/upload → 500 (267ms)"}
```

**RUST_LOG (worker.log):**
```
2026-04-10T14:30:02.567Z ERROR resq_worker::dlq: Job #1042 moved to dead letter queue after 3 failures
```

**Plain text (db.log):**
```
[2026-04-10 14:30:03] WARN Slow query detected (654ms): SELECT * FROM events WHERE created_at > $1
```

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `↑`/`↓` | Scroll through log history |
| `/` | Open search prompt |
| `Esc` | Close search |
| `q` | Quit |
