# resq-cli Examples

The main `resq` CLI with copyright management, secret scanning, and pre-commit checks.

## Demo: Sample Project with Issues

[`sample-project/`](sample-project/) is a small project with intentional problems for resq-cli to find:
- **4 source files** missing copyright headers (`.rs` and `.py`)
- **7 planted fake secrets** across 2 files (AWS keys, database URLs, API tokens)

### Files included

| File | Issues |
|------|--------|
| `src/main.rs` | Missing copyright header |
| `src/lib.rs` | Missing copyright header |
| `src/config.rs` | Missing copyright header + 4 fake secrets (AWS key, DB URL, GitHub token, Slack webhook) |
| `src/server.py` | Missing copyright header + 3 fake secrets (OpenAI key, Stripe key, SendGrid key) |

All secrets are fake and clearly marked in comments. They follow real patterns so `resq secrets` can detect them.

### Run it

```bash
# Initialize it as a git repo first (resq needs git context)
cd examples/resq-cli/sample-project
git init && git add -A && git commit -m "init"

# --- Copyright Headers ---

# Check which files are missing headers
cargo run -p resq-cli -- copyright --check

# Add headers to all files
cargo run -p resq-cli -- copyright

# Verify they were added
cargo run -p resq-cli -- copyright --check

# --- Secret Scanning ---

# Scan for secrets
cargo run -p resq-cli -- secrets

# --- Pre-Commit (all checks at once) ---

cargo run -p resq-cli -- pre-commit
```

### What you'll see

**Copyright check (`--check`):**
```
Scanning for missing copyright headers...
  ✗ src/main.rs      missing header
  ✗ src/lib.rs       missing header
  ✗ src/config.rs    missing header
  ✗ src/server.py    missing header

4 files need copyright headers
```

**After running `copyright` (without `--check`):**
```
  ✓ src/main.rs      header added (Rust block comment)
  ✓ src/lib.rs       header added (Rust block comment)
  ✓ src/config.rs    header added (Rust block comment)
  ✓ src/server.py    header added (hash-style comment)
```

**Secret scanning:**
```
Scanning for secrets...
  src/config.rs:7     AWS Access Key ID (AKIA...)
  src/config.rs:8     Database connection string with credentials
  src/config.rs:9     GitHub Personal Access Token (ghp_...)
  src/config.rs:10    Slack Webhook URL
  src/server.py:6     OpenAI API Key (sk-proj-...)
  src/server.py:7     Stripe Secret Key (sk_live_...)
  src/server.py:8     SendGrid API Key (SG....)

7 potential secrets found
```

### Key takeaway

Run `resq pre-commit` before every commit to catch both missing headers and leaked secrets in one pass. Set it up as a git hook:

```bash
# .git/hooks/pre-commit
#!/bin/sh
cargo run -p resq-cli -- pre-commit
```
