# resq-clean Examples

Visual workspace cleaner that identifies build artifacts and gitignored files with selective deletion.

## Demo: Create a Messy Workspace

[`create_mess.sh`](create_mess.sh) initializes a git repo in `/tmp/resq-clean-demo` and fills it with ~55 MB of build artifacts across 7 categories, plus source files that should be kept.

### What it creates

| Artifact | Size | Type |
|----------|------|------|
| `target/` | ~43 MB | Rust build output (debug + release) |
| `.venv/` | ~5 MB | Python virtual environment |
| `.next/` | ~2.5 MB | Next.js build cache |
| `build/` + `dist/` | ~3.5 MB | Generic build outputs |
| `node_modules/` | ~600 KB | Node.js packages |
| `*.log` | ~250 KB | Build log files |
| `__pycache__/` | ~30 KB | Python bytecode |
| `.idea/` + `.vscode/` | ~1 KB | IDE configs |

**Source files preserved:** `src/main.rs`, `Cargo.toml`

### Run it

```bash
# Step 1: Create the messy workspace
./examples/resq-clean/create_mess.sh

# Step 2: Preview what would be cleaned (dry-run)
cd /tmp/resq-clean-demo
cargo run -p resq-clean --manifest-path /path/to/crates/Cargo.toml -- --dry-run

# Step 3: Interactive cleanup
cd /tmp/resq-clean-demo
cargo run -p resq-clean --manifest-path /path/to/crates/Cargo.toml
```

### What you'll see

**TUI showing:**
- List of gitignored directories and files, sorted largest first
- Each entry with a checkbox, path, and human-readable size
- Total reclaimable space at the bottom (~55 MB)
- Toggle items to select/deselect what to delete

### Before & After

**Before (after running `create_mess.sh`):**
```
/tmp/resq-clean-demo/
├── src/main.rs           (kept - tracked by git)
├── Cargo.toml            (kept - tracked by git)
├── target/          43 MB (gitignored)
├── .venv/            5 MB (gitignored)
├── .next/          2.5 MB (gitignored)
├── build/          3.0 MB (gitignored)
├── node_modules/   600 KB (gitignored)
├── *.log           250 KB (gitignored)
└── __pycache__/     30 KB (gitignored)
```

**After (selective cleanup — you chose target/ and .next/):**
```
Deleted: target/     (43 MB reclaimed)
Deleted: .next/     (2.5 MB reclaimed)
Kept:    .venv/, node_modules/, etc.

Total reclaimed: 45.5 MB
```

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate file list |
| `Space` | Toggle selection |
| `Enter` | Delete selected items |
| `a` | Select/deselect all |
| `q` | Quit without deleting |
