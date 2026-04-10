#!/usr/bin/env python3
"""
Mock /status endpoint for resq-perf demo.

Serves a /status JSON response that simulates realistic, gradually changing
performance metrics: growing memory, fluctuating latency, increasing object
counts. Matches the exact schema resq-perf expects.

Usage:
    python3 mock_status_server.py                   # Start on :3000
    python3 mock_status_server.py --port 5000       # Custom port

    # In another terminal:
    cargo run -p resq-perf -- http://localhost:3000/admin/status
"""

import argparse
import http.server
import json
import math
import socketserver
import time

START = time.time()

# Baseline metrics (bytes)
BASE_RSS = 200 * 1024 * 1024        # 200 MB
BASE_HEAP_USED = 60 * 1024 * 1024   # 60 MB
BASE_HEAP_TOTAL = 128 * 1024 * 1024  # 128 MB
BASE_EXTERNAL = 8 * 1024 * 1024     # 8 MB
BASE_OBJECTS = 10000


def build_status():
    """Generate a /status response with gradually changing metrics."""
    elapsed = time.time() - START
    t = elapsed

    # Simulate slow memory leak (~1MB every 10s)
    leak = int(t / 10) * 1024 * 1024

    # Sawtooth GC pattern: heap grows then drops every ~20s
    gc_cycle = (t % 20) / 20
    heap_delta = int(gc_cycle * 30 * 1024 * 1024)

    # Latency: base 45ms with sinusoidal jitter + occasional spikes
    latency_ms = 45 + 15 * math.sin(t / 5) + (80 if int(t) % 37 == 0 else 0)

    heap_used = BASE_HEAP_USED + heap_delta + leak
    heap_total = max(BASE_HEAP_TOTAL, heap_used + 20 * 1024 * 1024)

    uptime_s = int(elapsed)
    days = uptime_s // 86400
    hours = (uptime_s % 86400) // 3600
    mins = (uptime_s % 3600) // 60
    secs = uptime_s % 60

    parts = []
    if days:
        parts.append(f"{days}d")
    if hours:
        parts.append(f"{hours}h")
    parts.append(f"{mins}m")
    parts.append(f"{secs}s")
    uptime_str = " ".join(parts)

    return {
        "uptime": uptime_str,
        "uptimeNanoseconds": int(elapsed * 1e9),
        "memory": {
            "process": {
                "rss": BASE_RSS + leak + heap_delta,
                "heapUsed": heap_used,
                "heapTotal": heap_total,
                "external": BASE_EXTERNAL + int(t * 100),
                "arrayBuffers": int(t * 50),
            },
            "heap": {
                "heapSize": heap_used,
                "heapCapacity": heap_total,
                "extraMemorySize": int(t * 100),
                "objectCount": BASE_OBJECTS + int(t * 5),
                "protectedObjectCount": 150 + int(t * 0.2),
                "globalObjectCount": 42,
                "protectedGlobalObjectCount": 8,
                "objectTypeCounts": {
                    "Object": 4000 + int(t * 2),
                    "Array": 2500 + int(t),
                    "String": 2000 + int(t * 1.5),
                    "Function": 800,
                    "RegExp": 50,
                    "ArrayBuffer": 20 + int(t * 0.1),
                },
            },
        },
        "version": "2.1.0",
        "environment": "development",
    }


class StatusHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ("/admin/status", "/status"):
            # Check for bearer token (accept anything for demo)
            auth = self.headers.get("Authorization", "")
            if auth and not auth.startswith("Bearer "):
                self.send_response(401)
                self.end_headers()
                return

            body = json.dumps(build_status(), indent=2)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(body.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        pass


def main():
    parser = argparse.ArgumentParser(description="Mock /status server for resq-perf")
    parser.add_argument("--port", type=int, default=3000)
    args = parser.parse_args()

    socketserver.TCPServer.allow_reuse_address = True
    server = socketserver.TCPServer(("0.0.0.0", args.port), StatusHandler)

    print(f"Mock status server running on http://localhost:{args.port}")
    print(f"  Endpoint: http://localhost:{args.port}/admin/status")
    print()
    print("Simulated behaviors:")
    print("  - Memory: slow leak (~1MB/10s) + sawtooth GC pattern")
    print("  - Latency: ~45ms base with sinusoidal jitter + occasional spikes")
    print("  - Objects: gradually increasing count")
    print()
    print("Run in another terminal:")
    print(f"  cargo run -p resq-perf -- http://localhost:{args.port}/admin/status")
    print(f"  cargo run -p resq-perf -- http://localhost:{args.port}/admin/status --refresh-ms 200")
    print()
    print("Press Ctrl+C to stop.")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down...")
        server.shutdown()


if __name__ == "__main__":
    main()
