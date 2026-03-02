# AISH Next Iteration Plan

## Goals
1. Expand detector coverage for the most-used tools across stacks.
2. Improve detector selection reliability (command-aware routing + better fallback).
3. Add first-class zsh shell integration.
4. Add detector fixture tests for regression safety.

## Priority Tool Set (Top Cross-Stack Coverage)
- pytest
- jest
- vitest
- cargo (test/build)
- go test
- tsc
- eslint
- ruff
- mypy
- maven
- gradle
- dotnet
- cmake/ctest
- terraform
- docker (build/compose)
- kubectl

## Phase N0: Planning and Specs
### Tasks
- Capture detector priorities and rationale.
- Define acceptance criteria for detector matching and excerpt quality.
- Define zsh support parity with bash integration.

### Exit Criteria
- `docs/NEXT_ITERATION.md` committed.

## Phase N1: Detector Platform Hardening
### Tasks
- Switch detector engine to command-aware rule selection:
  - prefer command-matched detectors,
  - retain generic fallback.
- Parse from ANSI-stripped view while preserving raw `pty.log`.
- Add deterministic confidence scoring model.
- Keep detector enable/disable via config.

### Exit Criteria
- Relevant detector is selected consistently for known command families.
- Generic detector still catches unknown failures.

## Phase N2: Detector Expansion (Top Tools)
### Tasks
- Implement/upgrade rules for:
  - pytest, jest, vitest
  - cargo, go test
  - tsc, eslint, ruff, mypy
  - maven, gradle, dotnet
  - cmake/ctest
  - terraform
  - docker build/compose
  - kubectl
- Include summary extraction and bounded excerpt windows.

### Exit Criteria
- New detectors produce useful summaries/excerpts for representative logs.

## Phase N3: Fixture Test Harness
### Tasks
- Add fixture-based detector tests.
- Add one representative failure fixture per major tool family.
- Assert selected detector name and excerpt non-empty on failure fixtures.

### Exit Criteria
- `cargo test` covers detector routing regressions.

## Phase N4: zsh Support
### Tasks
- Add `shell/init.zsh`:
  - `ai()` wrapper,
  - optional shim loading via `--print-shims`,
  - env flag to disable shims.
- Verify shim output is zsh-safe.
- Add shell docs for bash + zsh setup.

### Exit Criteria
- zsh users can source init script and use `ai`/shims with preserved exit codes.

## Phase N5: Docs and UX Polish
### Tasks
- Update README for new detectors and zsh instructions.
- Document detector configuration and known limitations.

### Exit Criteria
- README reflects implemented behavior and setup steps.

## Iteration Acceptance Criteria
- At least 10 additional/common detector rules implemented.
- Command-aware routing reduces false detector selection.
- Fixture tests exist and pass.
- zsh setup path documented and shipped.
