# AISH Config Reference

Default config path:
- `~/.config/aish/config.toml`

Override path:
- `AISH_CONFIG=/path/to/config.toml`

## Top-Level Sections
- `[store]`
- `[output]`
- `[wrap]`
- `[detectors]`
- `[[policy]]` (repeatable)

## `[store]`
- `root` (`string`): directory where run artifacts are stored.
- `keep_days` (`integer`): age-based retention window in days.
- `max_total_mb` (`integer`): total storage cap in MB; oldest runs are deleted first.

Example:
```toml
[store]
root = "~/.local/state/aish"
keep_days = 14
max_total_mb = 2000
```

## `[output]`
- `mode` (`string`): default terminal rendering mode.
- Allowed: `auto`, `digest`, `excerpt`, `full`, `quiet`.
- `max_excerpt_lines` (`integer`): cap printed excerpt lines.
- `max_digest_lines` (`integer`): cap printed digest lines.
- `show_log_path` (`bool`): include `full log: ...` in terminal output.
- `show_warnings_on_success` (`bool`): allow warning excerpts in terminal for successful runs when detectors find warning markers.

Example:
```toml
[output]
mode = "auto"
max_excerpt_lines = 200
max_digest_lines = 3
show_log_path = true
show_warnings_on_success = false
```

## `[wrap]`
Controls shell shim generation via `aish-run --print-shims`.

- `default` (`string`): `on` or `off`.
- `commands` (`array[string]`): command names to shim.
- `skip_commands` (`array[string]`): excluded command names.

Example:
```toml
[wrap]
default = "on"
commands = ["pytest", "cargo", "go"]
skip_commands = ["cat", "less", "man"]
```

## `[detectors]`
- `enabled` (`array[string]`): detector IDs to enable.

Current IDs:
- `generic`
- `pytest`
- `jest`
- `vitest`
- `cargo`
- `go`
- `tsc`
- `eslint`
- `ruff`
- `mypy`
- `maven`
- `gradle`
- `dotnet`
- `cmake`
- `terraform`
- `docker`
- `kubectl`

Example:
```toml
[detectors]
enabled = ["generic", "pytest", "cargo", "go"]
```

## `[[policy]]`
Per-command overrides, matched by command basename and optional argument prefix.

- `match` (`string`): command basename to match (for example `cargo`, `pytest`).
- `show` (`string`, optional): `auto`, `digest`, `excerpt`, `full`, `quiet`.
- `excerpt_on_success` (`bool`, optional): write and allow excerpt on successful runs.
- `show_warnings_on_success` (`bool`, optional): print detector warning excerpts for successful runs with warning markers.
- `max_excerpt_lines` (`integer`, optional): per-policy excerpt cap.
- `max_digest_lines` (`integer`, optional): per-policy digest cap.
- `args_prefix` (`array[string]`, optional): only match when command args start with this prefix.

Examples:
```toml
[[policy]]
match = "pytest"
show = "excerpt"
max_excerpt_lines = 400

[[policy]]
match = "cargo"
args_prefix = ["test"]
show = "auto"

[[policy]]
match = "eslint"
show_warnings_on_success = true
```

## Complete Example
See [`examples/config.toml`](/Users/origo/src/aish/examples/config.toml).

## Additional Presets
- [`examples/config-minimal.toml`](/Users/origo/src/aish/examples/config-minimal.toml)
- [`examples/config-dev-shims.toml`](/Users/origo/src/aish/examples/config-dev-shims.toml)
- [`examples/config-ci.toml`](/Users/origo/src/aish/examples/config-ci.toml)
