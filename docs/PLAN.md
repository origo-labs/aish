# AISH MVP Implementation Plan

## 1. Objective and Scope
Build a shippable MVP that provides a normal shell workflow while routing command execution through a Rust runner that:
- executes commands in a PTY,
- stores full output and metadata for every run,
- prints concise digest output on success,
- prints relevant failure excerpts on error,
- returns exact child exit codes,
- supports explicit wrapping (`ai <cmd...>`) and optional command shim auto-wrap.

This plan implements the baseline MVP first, then optional stretch behavior without compromising reliability.

## 2. Non-Goals (MVP)
- No transparent interception of arbitrary shell commands via fragile DEBUG/PROMPT replay tricks.
- No heavy persistence layer (no DB).
- No agent orchestration or remote execution.
- No full stdout/stderr split while in PTY mode.

## 3. Deliverables
1. Rust binary `aish-run` with PTY execution, logging, metadata, digest/excerpt rendering, config loading, and detectors.
2. Shell integration script `shell/init.bash` with:
- explicit `ai()` wrapper,
- optional generated command shims from config.
3. Config example and schema docs.
4. Log store retention/cleanup logic.
5. Minimal helper CLI flows (`aish last`, `aish open`) or equivalent subcommands in runner.
6. Unit + integration test coverage for critical behavior.
7. `README` usage instructions and rollout notes.

## 4. Architecture Plan
### 4.1 Components
- `aish-run` (Rust): command runner and analyzer.
- `init.bash`: shell entrypoint for explicit and shim workflows.
- `~/.config/aish/config.toml`: runtime policy and detector config.
- `~/.local/state/aish/`: run artifacts and retention target.

### 4.2 Runner Data Flow
1. Parse CLI flags and config.
2. Resolve policy for command (`argv[0]`, optional args prefix).
3. Create run directory and write initial `meta.json`.
4. Spawn command in PTY.
5. Stream PTY bytes to `pty.log` and detector pipeline (line-oriented).
6. Wait for child completion while handling signal forwarding.
7. Finalize detector results.
8. Render digest/excerpt according to output mode.
9. Write final artifacts (`meta.json`, `digest.txt`, `relevant.txt`, `last` symlink).
10. Run retention cleanup.
11. Exit with exact child exit code.

## 5. Phase Plan

## Phase 0: Project Scaffolding
### Tasks
- Create crate layout (single binary crate is acceptable for MVP).
- Add dependencies: `clap`, `serde`, `serde_json`, `toml`, `tracing`, `tracing-subscriber`, `time`, `uuid`, `portable-pty`, `regex`.
- Create module boundaries:
- `cli`, `config`, `policy`, `runner`, `pty`, `detectors`, `render`, `store`, `signals`.

### Exit Criteria
- `cargo build` passes.
- `aish-run --help` includes core flags.

## Phase 1: PTY Runner and Artifact Store (No Detectors Yet)
### Tasks
- Implement PTY spawn and command execution.
- Stream PTY output to `pty.log` in run directory.
- Capture timing and exit status.
- Write `meta.json` with core fields:
- run id, timestamps, duration, cwd, argv, exit code/signal, selected env subset.
- Write basic `digest.txt` (`OK/FAIL`, duration, command).
- Maintain `last` symlink.

### Exit Criteria
- Running `aish-run -- bash -lc 'echo hi'` creates run artifacts and exits `0`.
- Running failing command exits with same non-zero code.
- Progress bars/colors still render in PTY-executed commands.

## Phase 2: Output Policy and Rendering
### Tasks
- Add `--show=auto|digest|excerpt|full|quiet` behavior.
- Define default auto behavior:
- success => digest,
- failure => digest + excerpt + log path.
- Add excerpt limits (`max_excerpt_lines`, `max_digest_lines`).
- Optional ANSI stripping for excerpt output (default configurable).

### Exit Criteria
- Rendering mode is deterministic and test-covered.
- Full log path display can be toggled.

## Phase 3: Detector Framework + Generic Detector
### Tasks
- Implement detector trait and result model.
- Add line splitter with bounded buffering (ring buffer for context).
- Implement `GenericErrorDetector` patterns:
- Rust panic/panicked,
- Python traceback,
- Java exception/caused by,
- segfault/killed/oom,
- generic `error:` compiler hints.
- Produce `relevant.txt` excerpt with context window and file:line frame extraction.

### Exit Criteria
- Failing generic commands produce useful, bounded excerpts.
- No excerpt on success unless explicitly requested.

## Phase 4: Tool-Specific Detectors
### Tasks
- Implement `PytestDetector`:
- parse collected/summary/failures section.
- Implement `JestDetector`:
- parse `FAIL` blocks and final `Test Suites`/`Tests` summary.
- Implement `GradleDetector`:
- extract `BUILD FAILED` and `What went wrong` region.
- Implement `MavenDetector`:
- extract first `[ERROR]` block and build failure summary.
- Add confidence scoring and best-result selection.

### Exit Criteria
- Fixture-driven tests validate each detector on representative logs.
- On failure, tool-specific excerpt preferred over generic when confidence is higher.

## Phase 5: Config and Policy Engine
### Tasks
- Implement TOML schema:
- `[store]`, `[output]`, `[wrap]`, `[detectors]`, `[[policy]]`.
- Resolve defaults when config absent.
- Expand `~` in paths.
- Implement match logic:
- `argv[0]` basename,
- optional `args_prefix`.
- Apply per-policy overrides (show mode, excerpt caps, etc.).

### Exit Criteria
- Config-free execution works with sensible defaults.
- Config overrides produce expected behavior in tests.

## Phase 6: Shell Integration
### Tasks
- Add `shell/init.bash` with explicit wrapper:
- `ai() { command aish-run -- "$@"; }`.
- Implement optional shim mode.
- Preferred mechanism: `aish-run --print-shims` emits function shims from config `wrap.commands` minus `wrap.skip_commands`.
- Shell script `eval`s generated shims on init.
- Ensure zsh compatibility for explicit wrapper and shim format.

### Exit Criteria
- `source shell/init.bash` enables `ai` and configured shims.
- Wrapped commands preserve exit codes.
- `command <tool>` bypass remains available.

## Phase 7: Retention and Housekeeping
### Tasks
- Implement cleanup by `keep_days`.
- Implement size cap cleanup by oldest-first deletion to satisfy `max_total_mb`.
- Ensure robust behavior when symlink or metadata is missing/corrupt.

### Exit Criteria
- Repeated runs enforce retention bounds without affecting current run.

## Phase 8: Helper UX Commands
### Tasks
- Add minimal helper commands:
- `aish-run last` (or `aish last`) prints `relevant.txt` else `digest.txt`.
- `aish-run open` opens/pagers `pty.log` for last run.
- Keep command surface small and documented.

### Exit Criteria
- Last-run inspection works without manual path hunting.

## 6. Detailed Task Matrix
| Area | Implementation Notes | Tests |
|---|---|---|
| PTY execution | Use `portable-pty`; merged stream in MVP | integration: success/failure exit propagation |
| Logging | Write raw PTY bytes; avoid lossy transforms | artifact existence + non-empty log checks |
| Metadata | Write start, then finalize end fields atomically | schema roundtrip tests |
| Detectors | Incremental line feed + finalize pass | fixture snapshots per tool |
| Rendering | Central render module, mode-driven | unit tests for all `--show` modes |
| Policy | baseline defaults + per-policy override layering | match precedence tests |
| Retention | time-based then size-based cleanup | tempdir integration tests |
| Shell init | explicit wrapper + generated shims | bash/zsh smoke tests |

## 7. Testing Strategy
### 7.1 Unit Tests
- Config parsing and default resolution.
- Policy matching (`argv[0]`, `args_prefix`).
- Detector parsing with static fixture logs.
- Renderer behavior per mode.
- Retention calculations.

### 7.2 Integration Tests
- Spawn runner against controlled commands (`echo`, `false`, synthetic traceback outputs).
- Verify artifact layout and `last` symlink behavior.
- Verify signal forwarding semantics (`SIGINT`) and partial log persistence.

### 7.3 Manual Validation Checklist
- `ai pytest` failing test shows targeted excerpt.
- `ai cargo test` success shows short digest.
- Colors and progress bars preserved.
- `command pytest` bypasses shim.
- Works in bash and zsh when sourced.

## 8. Milestones and Sequencing
1. M1: PTY + logging + exit propagation + digest baseline.
2. M2: Generic detector + failure excerpt.
3. M3: Tool-specific detectors + confidence routing.
4. M4: Config/policy and shim generation.
5. M5: Retention + helper commands + docs + test hardening.

## 9. Risks and Mitigations
- PTY edge cases across OS/shells:
- Mitigation: keep PTY backend narrow, add fallback `--no-pty` mode.
- Signal handling complexity:
- Mitigation: forward `SIGINT`/`SIGTERM`, preserve partial artifacts, test explicitly.
- Over-aggressive excerpt heuristics:
- Mitigation: bounded windows, confidence scoring, always keep full log.
- Shell wrapping regressions:
- Mitigation: explicit `ai` is baseline; shims are opt-in and bypassable.

## 10. Acceptance Criteria (MVP Release Gate)
- `source ~/.aish/init.bash` enables reliable explicit wrapping via `ai`.
- `ai <cmd...>` runs in PTY, stores full logs and metadata, and returns exact exit code.
- Success output defaults to concise digest.
- Failure output defaults to digest + relevant excerpt + log path.
- Config controls output mode, detector set, wrap command list, and excerpt limits.
- Optional shim mode wraps configured tools safely and predictably.
- Retention cleanup enforces age and size constraints.
- Core behaviors are covered by automated tests and documented for users.

## 11. Post-MVP Next Steps
- Add JSON output mode for downstream automation.
- Add command-specific parser plugins and external detector loading.
- Add richer git context in metadata.
- Add non-PTY backend improvements for CI use cases.
