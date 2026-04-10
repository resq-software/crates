#!/usr/bin/env python3
"""
CPU-intensive workload for resq-flame profiling demo.

Runs multiple computation patterns so the flame graph shows distinct stacks:
  - fibonacci (recursive, deep stacks)
  - matrix multiply (loop-heavy, cache pressure)
  - sorting (comparison-heavy)
  - hashing (string processing)

Usage:
    # Terminal 1: Start the workload
    python3 cpu_burner.py

    # Terminal 2: Profile with py-spy (requires py-spy installed)
    #   pip install py-spy
    #   sudo py-spy record -o flamegraph.svg --pid $(pgrep -f cpu_burner)
    #
    # Or use resq-flame's TUI to select "Intelligence PDIE" target
    # (after configuring the PID).

The script runs until interrupted (Ctrl+C) and prints throughput stats.
"""

import hashlib
import time


def fibonacci(n):
    """Recursive fibonacci — creates deep call stacks in the flame graph."""
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


def matrix_multiply(size):
    """NxN matrix multiplication — loop-heavy, shows as wide bars."""
    a = [[i * j % 97 for j in range(size)] for i in range(size)]
    b = [[i * j % 89 for j in range(size)] for i in range(size)]
    c = [[0] * size for _ in range(size)]

    for i in range(size):
        for j in range(size):
            total = 0
            for k in range(size):
                total += a[i][k] * b[k][j]
            c[i][j] = total
    return c


def sorting_workload(size):
    """Generate and sort random-ish data — comparison-heavy."""
    data = [(i * 2654435761) % (size * 10) for i in range(size)]
    data.sort()
    return data


def hashing_workload(iterations):
    """Hash strings repeatedly — string processing + crypto."""
    result = b"seed"
    for _ in range(iterations):
        result = hashlib.sha256(result).digest()
    return result


def run_mixed_workload():
    """Run one iteration of mixed workload. Each function appears as
    a distinct flame in the graph."""
    fibonacci(28)
    matrix_multiply(50)
    sorting_workload(50000)
    hashing_workload(10000)


def main():
    print("CPU Burner — Flame Graph Demo Workload")
    print("=" * 45)
    print(f"PID: {__import__('os').getpid()}")
    print()
    print("Running: fibonacci, matrix multiply, sorting, hashing")
    print("Each function creates distinct patterns in flame graphs.")
    print()
    print("To profile:")
    print(f"  sudo py-spy record -o flamegraph.svg --pid {__import__('os').getpid()}")
    print()
    print("Press Ctrl+C to stop.")
    print()

    iteration = 0
    start = time.time()

    try:
        while True:
            iter_start = time.time()
            run_mixed_workload()
            iter_time = time.time() - iter_start
            iteration += 1

            if iteration % 5 == 0:
                elapsed = time.time() - start
                rate = iteration / elapsed
                print(f"  Iteration {iteration:>4} | {iter_time:.2f}s/iter | {rate:.1f} iter/s total")
    except KeyboardInterrupt:
        elapsed = time.time() - start
        print(f"\nCompleted {iteration} iterations in {elapsed:.1f}s ({iteration/elapsed:.1f} iter/s)")


if __name__ == "__main__":
    main()
