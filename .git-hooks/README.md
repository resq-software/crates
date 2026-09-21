# ResQ Git Hooks

This directory contains custom Git hooks for the ResQ platform.

## Hooks

### `pre-commit`
Runs a comprehensive set of checks before allowing a commit:
- **Copyright Verification**: Ensures all source files have appropriate Apache-2.0 headers.
- **Secret Scanning**: Scans for accidental inclusion of API keys, private keys, and credentials.
- **Security Audit**: Runs `cargo audit` and other security checks.
- **Formatting**: Verifies code formatting.
- **Versioning**: Prompts for a changeset if a version bump is needed.

This hook delegates to the `resq pre-commit` command in the project's own CLI.

### `commit-msg`
Ensures commit messages follow the **Conventional Commits** specification:
- Format: `type(scope): subject`
- Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- Prevents `fixup!`, `squash!`, and `WIP` commits on the `master` branch.

## Setup

To enable these hooks, run the following command from the repository root:

```bash
resq dev install-hooks
```

If you are using the debug build:
```bash
./target/debug/resq dev install-hooks
```

This configures `git` to use this directory for hooks:
```bash
git config core.hooksPath .git-hooks
```

## Skipping Checks (`GIT_HOOKS_SKIP`)

`GIT_HOOKS_SKIP` takes a **list of named tokens**, not a boolean. Every skip is
announced on stderr naming exactly which checks were skipped and which ran, so
a hook can never quietly do less than you think it did.

| Token | Hook | Disables |
|---|---|---|
| `all`, `1`, `true`, `yes`, `on` | every hook | the whole hook (loud warning) |
| `0`, `false`, `no`, `off`, `none` | every hook | nothing — explicit no-op |
| `audit` | pre-commit | security audit (osv-scanner / audit-ci) |
| `format` | pre-commit | code formatters |
| `versioning` | pre-commit | changeset / version prompt |
| `msg-format` | commit-msg | Conventional Commits subject check |
| `wip-guard` | commit-msg | fixup!/squash!/WIP block on main |
| `force-push` | pre-push | force-push guard on main/master |
| `branch-name` | pre-push | branch naming convention |
| `notify` | post-merge, post-checkout | lockfile drift notices |
| `local` | every hook | the repo-local `local-<hook>` override |

Separators are comma, colon or whitespace, and tokens are case-insensitive:

```bash
GIT_HOOKS_SKIP=audit        git commit   # skips ONLY the security audit
GIT_HOOKS_SKIP=audit,format git commit   # skips two checks
GIT_HOOKS_SKIP=all          git commit   # skips everything, and says so loudly
```

The vocabulary is global, so a token exported for a shell session means the
same thing in every hook; tokens belonging to another hook are inert there
rather than an error.

### The secrets scan has no token of its own

The secret scan is the compensating control for not licensing GitHub Secret
Protection, so it is deliberately **not** individually skippable. It can only
be turned off by `GIT_HOOKS_SKIP=all` — which prints an explicit
`THE SECRETS SCAN DID NOT RUN` warning — or by git's own per-invocation bypass
flag. `GIT_HOOKS_SKIP=secrets` is rejected with an explanation rather than
silently honoured.

### Unknown values fail closed

An unrecognised token makes the **gating** hooks (`pre-commit`, `commit-msg`,
`pre-push`) refuse to run and exit 1, listing the valid tokens. This is
deliberate. Previously *any* non-empty value — including a typo, and including
`0` and `false` — silently disabled the entire hook. That is how a
`GIT_HOOKS_SKIP=audit` invocation once ran with the secret scanner switched
off while appearing to skip only the audit.

The **non-gating** hooks (`post-merge`, `post-checkout`) print the same error
but exit 0 and run every check, because they execute after the operation has
already completed and refusing there could not protect anything.

## Bypassing Hooks Entirely

Git's own per-invocation bypass flag still works and skips every hook at once:

```bash
git commit --no-verify
```

Prefer a named token where one exists: it keeps the remaining checks running.
