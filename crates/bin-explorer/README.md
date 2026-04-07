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

# resq-bin — Binary & Machine-Code Analyzer

Robust interactive binary analyzer for ResQ services. Provides deep visibility into ELF/object files, disassembly using Capstone and objdump, and a performant caching system for large-scale analysis.

## Build

```bash
# Build from workspace root
cargo build --release -p resq-bin
```

Binary: `target/release/resq-bin`

## Usage

```bash
# Interactive TUI for a single file (default)
resq-bin --file target/release/resq

# Analyze all binaries in a directory recursively
resq-bin --dir target/release --recursive

# Emit JSON report (for CI/tooling)
resq-bin --file my-service --json

# Emit human-readable plain text report
resq-bin --file my-service --plain
```

## Features

- **Multi-format Support**: Analyzes ELF, Mach-O, and PE files via the `object` crate.
- **Interactive TUI**: Visual exploration of sections, symbols, and disassembled functions.
- **Disassembly Backends**: Uses `Capstone` for high-quality instruction decoding, with fallback to `objdump` if needed.
- **Smart Caching**: Persistent analysis cache in `.cache/resq/bin-explorer` to avoid redundant heavy disassembly on unchanged files.
- **Section & Symbol Analysis**: detailed breakdown of binary layout, memory addresses, and entry points.

## TUI Layout

```
┌─ resq-bin ────────────────────────────────────────────────────────┐
│ file: target/release/resq [ELF64 x86_64]                           │
├────────────────────────────────────────────────────────────────────┤
│ SECTIONS             SYMBOLS                DISASSEMBLY            │
│ .text  [0x1000]      main    [0x1050]       push rbp               │
│ .data  [0x2000]      _start  [0x1020]       mov  rbp, rsp          │
│ .rodata[0x1500]      ...                    sub  rsp, 0x10         │
├────────────────────────────────────────────────────────────────────┤
│ [q] quit   [Tab] focus   [↑↓] select   [/] search   [Enter] detail │
└────────────────────────────────────────────────────────────────────┘
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Switch focus between Sections, Symbols, and Disassembly |
| `↑` / `↓` | Navigate focused list |
| `/` | Search symbols or functions |
| `Enter` | Toggle disassembly for selected symbol |

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--file <path>` | — | Analyze a single binary file |
| `--dir <path>` | — | Analyze all object-like files in a directory |
| `--recursive` | off | Include recursive traversal for directory mode |
| `--ext <ext>` | — | Filter files by extension (e.g. `.so`, `.o`) |
| `--no-disasm` | off | Disable disassembly and only collect metadata |
| `--max-functions` | `40` | Maximum functions to disassemble per binary |
| `--json` | off | Emit JSON instead of interactive TUI |
| `--plain` | off | Emit plain text instead of interactive TUI |
| `--no-cache` | off | Disable result cache reads/writes |
| `--rebuild-cache` | off | Force refresh cached reports |
