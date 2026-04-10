#!/usr/bin/env python3
"""
Mock services for resq-health demo.

Starts 5 HTTP servers that simulate the ResQ service fleet:
  - coordination-hce    :5000  /health  (healthy, occasional degraded)
  - infrastructure-api  :8080  /health  (always healthy)
  - intelligence-pdie   :8000  /health  (unhealthy after 30s)
  - neo-n3-rpc          :20332         (JSON-RPC 2.0 getversion)
  - ipfs-gateway        :8081  /api/v0/version

Usage:
    python3 mock_services.py          # Start all 5 services
    resq-health                       # In another terminal

Press Ctrl+C to stop all servers.
"""

import http.server
import json
import socketserver
import threading
import time

START_TIME = time.time()


class HealthHandler(http.server.BaseHTTPRequestHandler):
    """Base handler for /health endpoints."""

    status_func = None
    service_name = "unknown"

    def do_GET(self):
        if self.path == "/health":
            status, body = self.status_func()
            self.send_response(200 if status != "error" else 503)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(body).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        pass  # Suppress request logs


class HceHandler(HealthHandler):
    """coordination-hce — healthy, occasionally degraded."""

    service_name = "coordination-hce"

    @staticmethod
    def status_func():
        elapsed = time.time() - START_TIME
        # Every 15 seconds, briefly report degraded for 3 seconds
        if int(elapsed) % 15 < 3 and elapsed > 5:
            return "degraded", {"status": "degraded", "uptime": f"{elapsed:.0f}s"}
        return "ok", {"status": "ok", "uptime": f"{elapsed:.0f}s"}


class InfraHandler(HealthHandler):
    """infrastructure-api — always healthy."""

    service_name = "infrastructure-api"

    @staticmethod
    def status_func():
        return "ok", {"status": "ok", "version": "2.4.1"}


class PdieHandler(HealthHandler):
    """intelligence-pdie — becomes unhealthy after 30 seconds."""

    service_name = "intelligence-pdie"

    @staticmethod
    def status_func():
        elapsed = time.time() - START_TIME
        if elapsed > 30:
            return "error", {"status": "error", "reason": "model OOM"}
        return "ok", {"status": "ok", "model": "resq-v3"}


class NeoHandler(http.server.BaseHTTPRequestHandler):
    """neo-n3-rpc — JSON-RPC 2.0 getversion endpoint."""

    def do_POST(self):
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len)
        try:
            req = json.loads(body)
        except json.JSONDecodeError:
            req = {}

        if req.get("method") == "getversion":
            resp = {
                "jsonrpc": "2.0",
                "id": req.get("id", 1),
                "result": {
                    "tcpport": 20333,
                    "wsport": 20334,
                    "nonce": 1234567890,
                    "useragent": "/Neo:3.6.0/",
                    "protocol": {"network": 860833102},
                },
            }
        else:
            resp = {
                "jsonrpc": "2.0",
                "id": req.get("id", 1),
                "error": {"code": -32601, "message": "Method not found"},
            }

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(resp).encode())

    def log_message(self, fmt, *args):
        pass


class IpfsHandler(http.server.BaseHTTPRequestHandler):
    """ipfs-gateway — /api/v0/version endpoint."""

    def do_GET(self):
        if self.path == "/api/v0/version":
            resp = {"Version": "0.18.1", "Commit": "abc123", "Repo": "14"}
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(resp).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, fmt, *args):
        pass


def start_server(handler_class, port, name):
    """Start an HTTP server in a daemon thread."""
    socketserver.TCPServer.allow_reuse_address = True
    server = socketserver.TCPServer(("0.0.0.0", port), handler_class)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    print(f"  {name:<25} http://localhost:{port}")
    return server


def main():
    print("Starting mock ResQ services...\n")

    servers = [
        start_server(HceHandler, 5000, "coordination-hce"),
        start_server(InfraHandler, 8080, "infrastructure-api"),
        start_server(PdieHandler, 8000, "intelligence-pdie"),
        start_server(NeoHandler, 20332, "neo-n3-rpc"),
        start_server(IpfsHandler, 8081, "ipfs-gateway"),
    ]

    print(f"\nAll {len(servers)} services running. Behaviors:")
    print("  - coordination-hce:   healthy, briefly degraded every 15s")
    print("  - infrastructure-api: always healthy")
    print("  - intelligence-pdie:  healthy for 30s, then unhealthy (OOM)")
    print("  - neo-n3-rpc:         JSON-RPC 2.0, always responds")
    print("  - ipfs-gateway:       always healthy")
    print("\nRun in another terminal:")
    print("  cargo run -p resq-health")
    print("  cargo run -p resq-health -- --check    # single check, CI mode")
    print("\nPress Ctrl+C to stop.")

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down...")
        for s in servers:
            s.shutdown()


if __name__ == "__main__":
    main()
