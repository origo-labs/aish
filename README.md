# aish

AISH is a shell-friendly command runner that executes commands in a PTY, logs full output, and prints concise summaries by default.

Current implementation lives in `crates/aish-run` and is integrated through `shell/init.bash` and `shell/init.zsh`.

## What It Does
- Runs commands through `aish-run` (PTY by default).
- Streams and stores full command output to disk.
- Writes per-run metadata and digest files.
- Shows digest/excerpt/full/quiet output modes.
- Extracts relevant failure excerpts with detectors:
  - generic
  - pytest
  - jest
  - vitest
  - cargo
  - go
  - tsc
  - eslint
  - ruff
  - mypy
  - gradle
  - maven
  - dotnet
  - cmake/ctest
  - terraform
  - docker
  - kubectl
- Supports config-driven shell shim generation.
- Provides quick access to last run output.

## Project Layout
- `crates/aish-run`: Rust CLI runner.
- `shell/init.bash`: shell function setup (`ai` and optional shims).
- `shell/init.zsh`: zsh function setup (`ai` and optional shims).
- `examples/config.toml`: sample config.
- `docs/PLAN.md`: implementation plan.
- `docs/NEXT_ITERATION.md`: detector/zsh iteration plan.
- `specs/initial.md`: original MVP spec.

## Build
```bash
cargo build
```

## Install
Install the binary into Cargo's bin directory (typically `~/.cargo/bin`):
```bash
cargo install --path crates/aish-run --locked
```

For local iteration, replace existing install:
```bash
cargo install --path crates/aish-run --locked --force
```

## Basic Usage
Run an explicit wrapped command:
```bash
aish-run -- bash -lc 'echo hello'
```

Or source shell integration and use `ai` (bash):
```bash
source shell/init.bash
ai bash -lc 'echo hello'
```

zsh setup:
```zsh
source shell/init.zsh
ai bash -lc 'echo hello'
```

### Output Modes
```bash
aish-run --show auto -- bash -lc 'echo ok'
aish-run --show digest -- bash -lc 'echo ok'
aish-run --show excerpt -- bash -lc 'exit 1'
aish-run --show full -- bash -lc 'echo ok'
aish-run --show quiet -- bash -lc 'echo ok'
```

### Last Run Helpers
```bash
aish-run --last
aish-run --open
```

`--last` prints `relevant.txt` if available, otherwise `digest.txt`.
`--open` uses `$PAGER` to open `pty.log` (falls back to `cat`).

## Shell Integration
`ai` wrapper is always defined by `shell/init.bash`:
```bash
ai() { command aish-run -- "$@"; }
```

Optional shims are loaded from config when `wrap.default = "on"`:
```bash
aish-run --print-shims
```

`shell/init.bash` evaluates generated shims unless disabled:
```bash
AISH_ENABLE_SHIMS=0 source shell/init.bash
```

`shell/init.zsh` supports the same behavior:
```zsh
AISH_ENABLE_SHIMS=0 source shell/init.zsh
```

## Configuration
Default config path:
- `~/.config/aish/config.toml`

Override config path with:
- `AISH_CONFIG=/path/to/config.toml`

Sample config is provided in [`examples/config.toml`](examples/config.toml).

### Key Config Areas
- `[store]`: log root, retention days, max total size.
- `[output]`: default show mode and line limits.
- `[wrap]`: shim behavior and command lists.
- `[detectors]`: enabled detectors.
- `[[policy]]`: per-command overrides, including `args_prefix`.

## Run Artifacts
By default, runs are stored under:
- `~/.local/state/aish/runs/<date>/<run-id>/`

Each run directory contains:
- `meta.json`
- `pty.log`
- `digest.txt`
- `relevant.txt` (on failure or policy-driven)

A `last` symlink points to the most recent run.

## Retention
Retention is enforced after each run:
- Deletes runs older than `store.keep_days`.
- Enforces `store.max_total_mb` by deleting oldest runs first.
- Preserves the current run being written.

## Status
- MVP phases 0-8 from `docs/PLAN.md` are implemented.
- Next iteration work from `docs/NEXT_ITERATION.md` includes command-aware detector routing, expanded detector coverage, fixture tests, and zsh init support.
