#!/usr/bin/env python3
"""
Generate sample log files for resq-logs demo.

Creates log files in all three formats that resq-logs understands:
  1. JSON structured logs (api.log)
  2. RUST_LOG format (worker.log)
  3. Plain text (db.log)

Usage:
    python3 generate_logs.py                        # Generate to ./sample-logs/
    python3 generate_logs.py --output /tmp/logs     # Custom output dir
    python3 generate_logs.py --live                 # Continuously append (tail -f demo)

    # Then in another terminal:
    cargo run -p resq-logs -- --source file --path examples/resq-logs/sample-logs/
"""

import argparse
import json
import os
import random
import time
from datetime import datetime, timezone

random.seed(42)

API_ENDPOINTS = [
    ("GET", "/api/v1/users", 200, 12),
    ("GET", "/api/v1/feed", 200, 45),
    ("POST", "/api/v1/events", 201, 23),
    ("GET", "/api/v1/health", 200, 2),
    ("POST", "/api/v1/auth/login", 200, 89),
    ("POST", "/api/v1/auth/login", 401, 5),
    ("GET", "/api/v1/search", 200, 120),
    ("PUT", "/api/v1/users/settings", 200, 34),
    ("GET", "/api/v1/notifications", 200, 18),
    ("DELETE", "/api/v1/sessions", 204, 8),
    ("GET", "/api/v1/missing", 404, 3),
    ("POST", "/api/v1/upload", 500, 250),
]

WORKER_TASKS = [
    ("resq_worker::queue", "INFO", "Processing job #{job_id}"),
    ("resq_worker::queue", "INFO", "Job #{job_id} completed in {ms}ms"),
    ("resq_worker::retry", "WARN", "Job #{job_id} failed, retrying (attempt {attempt}/3)"),
    ("resq_worker::dlq", "ERROR", "Job #{job_id} moved to dead letter queue after 3 failures"),
    ("resq_worker::scheduler", "DEBUG", "Polling queue, {count} jobs pending"),
    ("resq_worker::health", "INFO", "Worker heartbeat: {count} jobs processed"),
    ("resq_worker::connection", "ERROR", "Redis connection lost, reconnecting..."),
    ("resq_worker::connection", "INFO", "Redis connection restored"),
]

DB_EVENTS = [
    ("INFO", "Query executed in {ms}ms: SELECT * FROM users WHERE id = $1"),
    ("INFO", "Query executed in {ms}ms: INSERT INTO events (type, data) VALUES ($1, $2)"),
    ("DEBUG", "Connection pool: {active}/20 active, {idle}/20 idle"),
    ("WARN", "Slow query detected ({ms}ms): SELECT * FROM events WHERE created_at > $1"),
    ("ERROR", "Connection timeout after 5000ms to postgres:5432"),
    ("INFO", "Query executed in {ms}ms: UPDATE users SET last_seen = NOW() WHERE id = $1"),
    ("ERROR", "Deadlock detected on table 'events', retrying transaction"),
    ("INFO", "Vacuum completed on table 'events': {count} dead tuples removed"),
    ("WARN", "Connection pool near capacity: {active}/20 active"),
]


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


def generate_json_log_line():
    """Generate one JSON structured log line (api.log format)."""
    endpoint = random.choice(API_ENDPOINTS)
    method, path, status, base_ms = endpoint
    ms = base_ms + random.randint(-5, 50)

    level = "info"
    if status >= 500:
        level = "error"
    elif status >= 400:
        level = "warn"

    return json.dumps({
        "timestamp": now_iso(),
        "level": level,
        "service": "api",
        "msg": f"{method} {path} → {status} ({ms}ms)",
        "method": method,
        "path": path,
        "status": status,
        "latency_ms": ms,
    })


def generate_rustlog_line(job_counter):
    """Generate one RUST_LOG format line (worker.log format)."""
    template = random.choice(WORKER_TASKS)
    module, level, msg_template = template

    msg = msg_template.format(
        job_id=job_counter,
        ms=random.randint(10, 500),
        attempt=random.randint(1, 3),
        count=random.randint(0, 50),
    )

    return f"{now_iso()} {level} {module}: {msg}"


def generate_plain_line():
    """Generate one plain text log line (db.log format)."""
    template = random.choice(DB_EVENTS)
    level, msg_template = template

    msg = msg_template.format(
        ms=random.randint(1, 800),
        active=random.randint(3, 18),
        idle=random.randint(2, 17),
        count=random.randint(100, 5000),
    )

    ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    return f"[{ts}] {level} {msg}"


def generate_batch(output_dir, num_lines=200):
    """Generate a static batch of log files."""
    os.makedirs(output_dir, exist_ok=True)

    api_path = os.path.join(output_dir, "api.log")
    worker_path = os.path.join(output_dir, "worker.log")
    db_path = os.path.join(output_dir, "db.log")

    job_counter = 1000

    with open(api_path, "w") as api_f, \
         open(worker_path, "w") as worker_f, \
         open(db_path, "w") as db_f:

        for i in range(num_lines):
            # Weight: more API logs than worker/db
            r = random.random()
            if r < 0.5:
                api_f.write(generate_json_log_line() + "\n")
            elif r < 0.8:
                worker_f.write(generate_rustlog_line(job_counter) + "\n")
                if random.random() < 0.3:
                    job_counter += 1
            else:
                db_f.write(generate_plain_line() + "\n")

    print(f"Generated {num_lines} log entries across 3 files:")
    for name in ["api.log", "worker.log", "db.log"]:
        path = os.path.join(output_dir, name)
        lines = sum(1 for _ in open(path))
        size = os.path.getsize(path)
        print(f"  {name:<12} {lines:>4} lines  ({size:>6} bytes)")


def generate_live(output_dir):
    """Continuously append log entries (for tail -f style demos)."""
    os.makedirs(output_dir, exist_ok=True)

    api_path = os.path.join(output_dir, "api.log")
    worker_path = os.path.join(output_dir, "worker.log")
    db_path = os.path.join(output_dir, "db.log")

    job_counter = 1000

    print(f"Writing live logs to {output_dir}/")
    print("Press Ctrl+C to stop.\n")

    try:
        while True:
            r = random.random()
            if r < 0.5:
                with open(api_path, "a") as f:
                    line = generate_json_log_line()
                    f.write(line + "\n")
                    print(f"  [api]    {line[:80]}...")
            elif r < 0.8:
                with open(worker_path, "a") as f:
                    line = generate_rustlog_line(job_counter)
                    f.write(line + "\n")
                    print(f"  [worker] {line[:80]}")
                    if random.random() < 0.3:
                        job_counter += 1
            else:
                with open(db_path, "a") as f:
                    line = generate_plain_line()
                    f.write(line + "\n")
                    print(f"  [db]     {line[:80]}")

            time.sleep(random.uniform(0.1, 1.0))
    except KeyboardInterrupt:
        print("\nStopped.")


def main():
    parser = argparse.ArgumentParser(description="Generate sample logs for resq-logs")
    parser.add_argument(
        "--output", default=os.path.join(os.path.dirname(__file__), "sample-logs"),
        help="Output directory (default: ./sample-logs/)"
    )
    parser.add_argument("--lines", type=int, default=200, help="Number of log entries")
    parser.add_argument("--live", action="store_true", help="Continuously append logs")
    args = parser.parse_args()

    if args.live:
        generate_live(args.output)
    else:
        generate_batch(args.output, args.lines)
        print(f"\nRun resq-logs:")
        print(f"  cargo run -p resq-logs -- --source file --path {args.output}")
        print(f"  cargo run -p resq-logs -- --source file --path {args.output} --level error")


if __name__ == "__main__":
    main()
