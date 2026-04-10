# resq-bin Examples

Terminal binary and machine code analyzer for inspecting compiled artifacts.

## Demo: Compile and Analyze a C Program

[`demo.c`](demo.c) is a small C program with multiple functions (fibonacci, matrix ops, string handling, heap allocation) that produces interesting disassembly output. [`run_demo.sh`](run_demo.sh) compiles it and runs resq-bin automatically.

### What demo.c includes

- **fibonacci()** — recursive, generates deep call stacks
- **sum_to()** — iterative loop, different instruction patterns
- **process_record()** — string formatting via snprintf, PLT entries
- **create_records()** — heap allocation with calloc/free
- **Global data** — string literals in .rodata, counter in .data
- **Struct type** — generates interesting symbol metadata

### Run it

```bash
# All-in-one: compile, run, and analyze
./examples/resq-bin/run_demo.sh

# Specific output modes
./examples/resq-bin/run_demo.sh --plain   # Human-readable text
./examples/resq-bin/run_demo.sh --json    # Machine-readable JSON
./examples/resq-bin/run_demo.sh --tui     # Interactive terminal UI

# Or manually
gcc -o /tmp/demo examples/resq-bin/demo.c -O2 -g
cargo run -p resq-bin -- --file /tmp/demo --plain
```

### What you'll see

**Plain text output:**
```
Binary: /tmp/demo
Format: ELF 64-bit
Architecture: x86_64
Endianness: Little
Entry Point: 0x1060

Sections (12):
  .text       0x1060  4521 bytes
  .rodata     0x2000   312 bytes
  .data       0x4000    16 bytes
  ...

Symbols (23):
  fibonacci   0x1180  FUNCTION  global
  sum_to      0x11c0  FUNCTION  global
  ...

Disassembly (6 functions):
  fibonacci:
    0x1180: push rbp
    0x1181: mov  rbp, rsp
    ...
```

**JSON output** — pipe to `jq` for specific fields:
```bash
./examples/resq-bin/run_demo.sh --json 2>/dev/null | jq '.metadata'
./examples/resq-bin/run_demo.sh --json 2>/dev/null | jq '.symbols[].name'
```

### Analyze your own Rust binaries

```bash
# Build a release binary and analyze it
cargo build --release
cargo run -p resq-bin -- --file ./target/release/resq --plain --no-disasm

# Batch scan all binaries in a directory
cargo run -p resq-bin -- --dir ./target/release --json
```
