<!--
  Copyright 2026 ResQ

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# cleanup — Build Artifact Cleaner

Removes gitignored build artifacts and generated files from the monorepo while preserving negated patterns and `.env` files. Uses the `ignore` crate for full `.gitignore` semantics including negation (`!pattern`).

## Build

```bash
cargo build --release --manifest-path tools/Cargo.toml -p resq-cleanup
```

Binary: `tools/cleanup/target/release/cleanup`

## Usage

```bash
# Always preview first
cleanup --dry-run

# Run cleanup with per-file output
cleanup --verbose

# Silent run (no output except errors)
cleanup
```

## How It Works

1. Walks the repository tree starting from the project root
2. Applies `.gitignore` rules via the `ignore` crate (full support for negated patterns, nested `.gitignore` files, `.git/info/exclude`)
3. Identifies files and directories that are gitignored
4. Processes entries depth-first (children before parents) so directories only get removed after their contents are gone
5. Always preserves `.env` files regardless of gitignore rules

**Safety rules**:
- `.env` files are never deleted (even if gitignored) — they contain local credentials
- Negated patterns (`!important-file`) keep matching files safe
- Dry-run is the recommended first step

## What Gets Deleted

Anything gitignored, which typically includes:

```
target/           # Rust build output
node_modules/     # npm/bun dependencies
.next/            # Next.js build cache
dist/             # TypeScript/Bun build output
build/            # C++ CMake build dirs
__pycache__/      # Python bytecode
*.pyc
.pytest_cache/
*.o *.a *.so      # C++ object files
*.nettrace        # .NET profiling traces
flamegraph.svg    # Generated profiles
scripts/out/      # Dependency cost reports
```

Files matching negated gitignore entries (e.g. `!dist/keep-this.json`) are preserved.

## Flags

| Flag | Description |
|------|-------------|
| `--dry-run` | Print what would be deleted without deleting anything |
| `--verbose` | Print each file/directory as it is deleted |

## Example Output

```
$ cleanup --dry-run
Would delete: services/infrastructure-api/target/
Would delete: services/coordination-hce/node_modules/
Would delete: services/web-dashboard/.next/
Would delete: libs/cpp/resq-common/build/
Would delete: services/intelligence-pdie/__pycache__/
5 items would be removed (2.4 GB)
```

```
$ cleanup --verbose
Deleted: services/infrastructure-api/target/debug/build/
Deleted: services/infrastructure-api/target/debug/deps/
...
Deleted: services/infrastructure-api/target/
Done. Removed 847 items (2.4 GB freed)
```

## When to Use

- Before committing to ensure no build artifacts slip through
- After switching branches to avoid stale build caches
- Periodic deep clean when disk space is low

For a lighter clean, use the individual ecosystem commands:
```bash
cargo clean                          # Rust only
rm -rf services/coordination-hce/node_modules  # Node only
find . -type d -name __pycache__ -exec rm -rf {} +  # Python only
```
