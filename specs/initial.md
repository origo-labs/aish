Below is a concrete, buildable MVP plan that gives you a “normal bash” experience while routing command output through a Rust runner that logs everything, prints a digest by default, and prints relevant excerpts on failure (or when detectors fire). It’s designed so you can extend detectors, policies, storage, and UX without rewriting the core.

---

## 0) What “fully functional MVP” means

MVP goals (high-quality, shippable):

* **Works in bash** as a drop-in init script (`source ~/.aish/init.bash`)
* **Provides a wrapper function** (e.g. `ai`) that:

  * runs a command in a **PTY** (so progress bars/colors behave normally)
  * **streams full output to log files**
  * returns **exact exit code**
  * prints:

    * a concise **digest** when successful
    * a **relevant excerpt** (stack trace / failing tests / error region) on failure
* **Configurable** policies (wrap/skip commands, verbosity, storage path, max excerpt lines)
* **Sensible defaults**: wrap test/build tools automatically; never break pipes/redirections.
* **zsh compatibility**: since the wrapping is invoked as a normal command (`ai cmd…`), it works in any shell. Bash-specific part is only the auto-wrapping hook (optional).

Stretch-but-still-MVP (optional, but very useful):

* “Auto-wrap safe commands” in bash via `DEBUG` trap + PROMPT_COMMAND, with clear fallbacks.

---

## 1) Architecture (minimal moving parts)

### Components

1. **Rust binary** `aish-run`
   Runs a command, captures output via PTY, writes logs + metadata, computes digest + excerpt via detectors, prints selected view, returns exit code.

2. **Shell integration script** `init.bash`
   Defines:

   * `ai()` function: explicit wrapper that calls `aish-run`
   * optional “auto mode”: attempts to wrap interactive commands automatically when safe

3. **Config file** `~/.config/aish/config.toml`

   * default policies
   * per-command overrides
   * detectors settings

4. **Log store** under `~/.local/state/aish/` (Linux-ish default)

   * each run → unique run directory
   * `meta.json`, `pty.log`, `digest.txt`, `relevant.txt`, etc.
   * `last` symlink for easy “aictx last” later

---

## 2) Rust runner: concrete design

### 2.1 Crate choices

* CLI parsing: `clap`
* Config: `serde`, `toml`
* Logging: `tracing`, `tracing-subscriber`
* Time + ids: `time`, `uuid`
* Files: `tempfile` (optional)
* JSON: `serde_json`
* PTY:

  * Linux/macOS: `portable-pty` (good cross-platform ergonomics)
  * Alternative: `nix` + manual `forkpty` (more control, more work)
* Regex detection: `regex`

### 2.2 Command execution model (PTY-first)

* Create a PTY pair.
* Spawn the child command attached to the PTY slave.
* In parent:

  * read from PTY master as bytes
  * write bytes to:

    * `pty.log` (raw)
    * detector pipeline (line splitting)
  * optionally “live preview” to terminal (MVP can skip; just show digest/excerpt at end)

**Why merged log?** In PTY mode, stdout/stderr are usually merged. That’s fine for MVP; keep one `pty.log`. Later you can add non-PTY mode with separate stdout/stderr when needed.

### 2.3 Metadata

Write `meta.json` with at least:

* `id`, `timestamp_start`, `timestamp_end`, `duration_ms`
* `cwd`, `command_argv`, `shell` (if provided), `tty` info
* `exit_code`, `signal` (if terminated)
* `env` subset: `TERM`, `COLORTERM`, maybe `CI`
* Git context (optional but great):

  * repo root, branch, HEAD sha (best-effort)
* Config hash/version so later you know what policy ran.

### 2.4 Digest + excerpt model

At end of run, runner produces:

* `digest` (1–3 lines)
* `relevant_excerpt` (0..N lines)

MVP default printing:

* exit=0: print digest only
* exit≠0: print digest + excerpt + “log saved to …”

Add flags:

* `--show=full|digest|excerpt|auto` (default `auto`)
* `--no-pty` fallback
* `--log-dir <path>` override
* `--label <string>` optional (helps grouping)

---

## 3) Detectors: concrete MVP set

Detectors run incrementally as output arrives. Each detector can:

* “recognize tool”
* extract summary counts
* identify failure region boundaries
* extract stack traces with file:line frames

### 3.1 Core detector interface

In Rust terms:

```rust
trait Detector {
  fn name(&self) -> &'static str;
  fn observe_line(&mut self, line: &str);
  fn finalize(&self, exit_code: i32) -> DetectorResult;
}

struct DetectorResult {
  tool: Option<String>,
  summary: Vec<String>,        // for digest
  relevant: Vec<String>,       // excerpt lines
  confidence: u8,              // for tie-breaking
}
```

Runner aggregates results, chooses “best” by confidence + whether exit != 0, merges:

* digest: status + duration + top summary lines
* excerpt: highest-confidence relevant excerpt (and possibly extra from generic stack trace extractor)

### 3.2 MVP detectors to implement

1. **GenericErrorDetector (always on)**

   * look for:

     * `panic:` (Rust), `thread '...' panicked`
     * `Traceback (most recent call last):` (Python)
     * `Exception in thread` / `Caused by:` (Java)
     * `Segmentation fault`, `Killed`, `Out of memory`
     * `error:` patterns from compilers
   * excerpt policy:

     * keep last N lines leading up to first strong error marker + next M lines
     * plus extracted stack frames (regex for `file:line`)

2. **PytestDetector**

   * markers:

     * `=+ FAILURES =+`
     * `collected X items`
     * `FAILED ...` lines
   * summary:

     * number failed/passed/skipped if found
   * excerpt:

     * section under `FAILURES` until next separator, capped

3. **JestDetector**

   * markers:

     * `FAIL ` lines
     * `Test Suites:` / `Tests:` summary
   * excerpt:

     * failing test blocks and final summary

4. **GradleDetector**

   * markers:

     * `BUILD FAILED`
     * `FAILURE: Build failed with an exception.`
   * excerpt:

     * “What went wrong” block + “Caused by” chain

5. **MavenDetector**

   * markers:

     * `[ERROR]` blocks
     * `BUILD FAILURE`
   * excerpt:

     * first `[ERROR]` block and summary

**Important:** Keep these detectors *strictly heuristic*; don’t overfit. Always store full logs anyway.

---

## 4) Config: sensible defaults + per-command policy

### 4.1 Config schema (TOML)

Example `~/.config/aish/config.toml`:

```toml
[store]
root = "~/.local/state/aish"
keep_days = 14
max_total_mb = 2000

[output]
mode = "auto"          # auto|digest|full|quiet
max_excerpt_lines = 200
max_digest_lines = 3
show_log_path = true

[wrap]
default = "off"        # off|on (auto-wrapping)
commands = ["pytest", "jest", "gradle", "mvn", "go", "cargo", "npm", "pnpm", "yarn"]
skip_commands = ["cat", "less", "more", "man", "ssh", "vim", "nano", "top", "htop"]

[detectors]
enabled = ["generic", "pytest", "jest", "gradle", "maven"]
```

Per-command overrides:

```toml
[[policy]]
match = "cargo"
show = "auto"
excerpt_on_success = false

[[policy]]
match = "pytest"
show = "auto"
max_excerpt_lines = 400

[[policy]]
match = "npm"
args_prefix = ["test"]        # only wrap `npm test ...`
```

### 4.2 Matching rules (MVP)

* match on argv[0] basename
* optional args_prefix match
* optional cwd match later

---

## 5) Bash integration (explicit + optional auto)

### 5.1 Explicit wrapper (MVP baseline; rock solid)

In `init.bash`:

* `ai()`:

  * if user runs `ai <cmd...>`, call `aish-run -- <cmd...>`
  * returns child exit code
  * does not mess with quoting: just pass `"$@"`

This alone gives you a “normal shell” because users opt in, and it works in zsh too.

### 5.2 Optional auto-wrapping (bash-specific; MVP v1.1)

Auto-wrap is where things get tricky. The safest approach for MVP:

* Auto-wrap **only** when:

  * command line is a *simple external command invocation* (no `|`, no `>`, no `<`, no `;`, no `&&`, no `||`, no `$(`, no backticks)
  * first token matches configured wrap commands
  * not a shell builtin, not an alias/function, not assignment-only

Implementation strategy:

* Use `trap 'aish__preexec' DEBUG` to capture the raw `$BASH_COMMAND` before it runs.
* But you cannot easily replace execution mid-flight in bash without hacks.
* So the workable pattern is: **PROMPT_COMMAND-based “last command replay” is a non-starter** (it reruns commands).
* Therefore: for MVP, keep auto-wrapping as an **alias injection** approach for selected commands:

  * define shell functions for `pytest`, `jest`, `gradle`, `mvn`, etc. that call `aish-run -- pytest "$@"`.
  * This is robust, reversible, and predictable.

Example:

```bash
pytest() { command aish-run -- pytest "$@"; }
jest()   { command aish-run -- jest "$@"; }
gradle() { command aish-run -- gradle "$@"; }
mvn()    { command aish-run -- mvn "$@"; }
```

This is “bash primitives” and “works like normal shell”:

* `command pytest` bypasses function if needed
* exit codes preserved
* works in zsh too if sourced there

Then “configurable what commands to wrap” simply generates these function shims at shell init time based on config.

**MVP recommendation:** ship with explicit `ai` + optional “shim mode” that wraps a small curated list by defining functions.

---

## 6) Storage management (MVP but not sloppy)

### 6.1 Run directory layout

`~/.local/state/aish/runs/2026-03-02/20260302T153012Z_4f3c.../`

Files:

* `meta.json`
* `pty.log`
* `digest.txt`
* `relevant.txt`

Symlinks:

* `~/.local/state/aish/last` → latest run dir

### 6.2 Retention

On each run (or once per shell session):

* delete runs older than `keep_days`
* enforce `max_total_mb` by deleting oldest

Keep it simple; no database needed.

---

## 7) UX details that make it feel “high quality”

* Preserve colors: don’t strip ANSI in `pty.log`; store raw. For excerpt printing, you may optionally strip ANSI for readability (configurable).
* Always print where logs are:

  * `… (full log: ~/.local/state/aish/last/pty.log)`
* Provide helper CLI (MVP minimal):

  * `aish last` → prints `relevant.txt` if exists else `digest.txt`
  * `aish open` → `$PAGER` `pty.log` of last run

This keeps the workflow tight without needing an agent.

---

## 8) Implementation breakdown (weekend-to-1-week MVP)

### Step 1 — Repo scaffolding

* `crates/aish-run` binary
* `shell/init.bash` script
* `examples/config.toml`

### Step 2 — Runner can execute and log (no detectors yet)

* PTY spawn
* log file streaming
* meta.json
* correct exit code propagation
* basic digest: `OK/FAIL + duration + cmd`

### Step 3 — Generic detector + excerpt

* line splitter
* GenericErrorDetector:

  * keep ring buffer of last ~200 lines
  * on strong error markers, mark “start”
* excerpt printed on failure

### Step 4 — Add tool-specific detectors

* pytest/jest/gradle/maven
* confidence scoring and selection logic

### Step 5 — Config + shell shims

* load config in Rust (runner needs it)
* `init.bash` reads config minimally (or just relies on user listing shims manually)

  * best MVP: `init.bash` calls `aish-run --print-shims` which outputs shell function definitions for commands to wrap; then `eval` it
  * that keeps config parsing in Rust (one source of truth)

### Step 6 — Polish + tests

* unit tests for detectors with fixture logs
* integration test: spawn runner on `bash -lc '...'` for a failing command
* ensure signals behave sanely (Ctrl-C):

  * forward SIGINT to child
  * still write meta + partial logs

---

## 9) Extension points (so you don’t paint yourself into a corner)

Design these seams early:

* **Detector registry**: `Vec<Box<dyn Detector>>` built from config
* **Policy engine**: a small matcher that decides:

  * whether to run PTY
  * output mode
  * max excerpt
* **Renderer**: `render_digest`, `render_excerpt` (later can output JSON for agents)
* **Backends**:

  * PTY backend (default)
  * “pipe backend” for non-interactive contexts or when PTY fails

---

## 10) Answering your compatibility question

* The **explicit** wrapper (`ai cmd…`) is shell-agnostic → works in bash and zsh.
* The **shim mode** (defining functions named `pytest`, `mvn`, etc.) also works in bash and zsh.
* If later you want “transparent auto-wrap of arbitrary commands”, zsh hooks are much nicer than bash. But you can get 90% of the value with shims + `ai`.
