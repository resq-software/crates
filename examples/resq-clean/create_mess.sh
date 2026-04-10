#!/usr/bin/env bash
# Create a fake workspace with build artifacts for resq-clean to clean up.
#
# Usage:
#   ./create_mess.sh                  # Create messy workspace in /tmp
#   ./create_mess.sh /path/to/dir     # Custom location
#
# Then run:
#   cd <workspace> && cargo run -p resq-clean
#   cd <workspace> && cargo run -p resq-clean -- --dry-run
set -euo pipefail

WORKSPACE="${1:-/tmp/resq-clean-demo}"

echo "=== Creating messy workspace at $WORKSPACE ==="
rm -rf "$WORKSPACE"
mkdir -p "$WORKSPACE"
cd "$WORKSPACE"

# --- Initialize as a git repo (resq-clean reads .gitignore) ---
git init -q
cat > .gitignore << 'GITIGNORE'
# Rust
target/
*.o
*.so
*.dylib

# Node
node_modules/
.next/
dist/

# Python
__pycache__/
*.pyc
.venv/

# IDE
.idea/
.vscode/

# Build outputs
build/
*.log
GITIGNORE

git add .gitignore
git commit -q -m "init"

# --- Create source files (these should NOT be cleaned) ---
mkdir -p src
cat > src/main.rs << 'RS'
fn main() {
    println!("Hello, world!");
}
RS

cat > Cargo.toml << 'TOML'
[package]
name = "demo"
version = "0.1.0"
edition = "2021"
TOML

git add -A
git commit -q -m "add source"

# --- Create build artifacts (these SHOULD be cleaned) ---
echo "Creating build artifacts..."

# Rust target/ — ~50MB of fake build output
mkdir -p target/debug target/release
dd if=/dev/urandom of=target/debug/demo bs=1M count=20 2>/dev/null
dd if=/dev/urandom of=target/release/demo bs=1M count=15 2>/dev/null
dd if=/dev/urandom of=target/debug/libdemo.so bs=1M count=8 2>/dev/null
echo "  target/         ~43 MB"

# node_modules/ — deep directory tree
mkdir -p node_modules/.package-lock
for pkg in react react-dom webpack babel-core lodash express; do
    mkdir -p "node_modules/$pkg"
    dd if=/dev/urandom of="node_modules/$pkg/index.js" bs=100K count=1 2>/dev/null
done
echo "  node_modules/   ~600 KB"

# .next/ build cache
mkdir -p .next/cache .next/static
dd if=/dev/urandom of=.next/cache/build.json bs=500K count=1 2>/dev/null
dd if=/dev/urandom of=.next/static/bundle.js bs=1M count=2 2>/dev/null
echo "  .next/          ~2.5 MB"

# Python bytecode
mkdir -p __pycache__
for mod in main utils config; do
    dd if=/dev/urandom of="__pycache__/${mod}.cpython-312.pyc" bs=10K count=1 2>/dev/null
done
echo "  __pycache__/    ~30 KB"

# .venv/
mkdir -p .venv/lib/python3.12/site-packages
dd if=/dev/urandom of=.venv/lib/python3.12/site-packages/numpy.so bs=1M count=5 2>/dev/null
echo "  .venv/          ~5 MB"

# IDE directories
mkdir -p .idea .vscode
echo '{}' > .idea/workspace.xml
echo '{}' > .vscode/settings.json
echo "  .idea/ .vscode/ ~1 KB"

# Build output and logs
mkdir -p build dist
dd if=/dev/urandom of=build/output.wasm bs=1M count=3 2>/dev/null
dd if=/dev/urandom of=dist/app.js bs=500K count=1 2>/dev/null
for i in $(seq 1 5); do
    dd if=/dev/urandom of="build-${i}.log" bs=50K count=1 2>/dev/null
done
echo "  build/ dist/    ~3.5 MB"
echo "  *.log           ~250 KB"

echo
echo "=== Workspace ready ==="
echo "  Location: $WORKSPACE"
echo "  Source files: src/main.rs, Cargo.toml (will be kept)"
echo "  Artifacts: ~55 MB across 7 gitignored categories"
echo
echo "Run resq-clean:"
echo "  cd $WORKSPACE && cargo run -p resq-clean --manifest-path $(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/Cargo.toml"
echo
echo "Or with dry-run:"
echo "  cd $WORKSPACE && cargo run -p resq-clean --manifest-path $(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/Cargo.toml -- --dry-run"
