#!/usr/bin/env bash
# Compile the demo C program and analyze it with resq-bin.
#
# Usage: ./run_demo.sh [--tui|--json|--plain]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEMO_C="$SCRIPT_DIR/demo.c"
DEMO_BIN="$SCRIPT_DIR/demo"

# --- Step 1: Compile ---
echo "=== Step 1: Compiling demo.c ==="
if command -v gcc &>/dev/null; then
    gcc -o "$DEMO_BIN" "$DEMO_C" -O2 -g
    echo "  Compiled with gcc: $DEMO_BIN"
elif command -v cc &>/dev/null; then
    cc -o "$DEMO_BIN" "$DEMO_C" -O2 -g
    echo "  Compiled with cc: $DEMO_BIN"
else
    echo "  ERROR: No C compiler found (gcc or cc required)"
    exit 1
fi

echo
echo "=== Step 2: Running the demo binary ==="
"$DEMO_BIN"

echo
echo "=== Step 3: Analyzing with resq-bin ==="

MODE="${1:---plain}"
echo "  Mode: $MODE"
echo

# Run from workspace root so cargo can find the package
cd "$SCRIPT_DIR/../.."
cargo run -p resq-bin -- --file "$DEMO_BIN" "$MODE"

echo
echo "=== Cleanup ==="
rm -f "$DEMO_BIN"
echo "  Removed $DEMO_BIN"
