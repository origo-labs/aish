use regex::Regex;
use std::borrow::Cow;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub summary_lines: Vec<String>,
    pub excerpt: Option<String>,
    pub warning_detected: bool,
}

#[derive(Debug, Clone)]
struct RuleResult {
    detector: &'static str,
    tool: Option<&'static str>,
    confidence: u8,
    marker_hits: usize,
    summary: Vec<String>,
    excerpt_lines: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct ToolRule {
    id: &'static str,
    tool: Option<&'static str>,
    commands: &'static [&'static str],
    markers: &'static [&'static str],
    summary_markers: &'static [&'static str],
    excerpt_start: &'static [&'static str],
    excerpt_end: &'static [&'static str],
    base_confidence: u8,
}

pub fn analyze_log(
    log_path: &Path,
    exit_code: i32,
    enabled_detectors: &[String],
    command: &[String],
) -> AnalysisResult {
    let log_bytes = match fs::read(log_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return AnalysisResult {
                summary_lines: Vec::new(),
                excerpt: None,
                warning_detected: false,
            };
        }
    };

    let raw_text = String::from_utf8_lossy(&log_bytes);
    let parsed_text = strip_ansi(&raw_text);
    analyze_text(&parsed_text, exit_code, enabled_detectors, command)
}

fn analyze_text(
    text: &str,
    exit_code: i32,
    enabled_detectors: &[String],
    command: &[String],
) -> AnalysisResult {
    let lines: Vec<&str> = text.lines().collect();
    let cmd_name = command_basename(command).to_ascii_lowercase();

    let mut best: Option<RuleResult> = None;
    for &rule in tool_rules() {
        if !detector_enabled(rule.id, enabled_detectors) {
            continue;
        }
        let result = evaluate_rule(rule, &cmd_name, &lines, exit_code);
        if should_replace(best.as_ref(), &result) {
            best = Some(result);
        }
    }

    let warning_detected = exit_code == 0 && best.as_ref().is_some_and(|r| r.marker_hits > 0);
    let summary_lines = best
        .as_ref()
        .map(|r| {
            let mut lines = r.summary.clone();
            lines.push(format!("detector: {}", r.detector));
            if let Some(tool) = r.tool {
                lines.push(format!("tool: {tool}"));
            }
            lines
        })
        .unwrap_or_default();
    let excerpt = best
        .as_ref()
        .and_then(|r| (!r.excerpt_lines.is_empty()).then_some(r.excerpt_lines.join("\n")));

    AnalysisResult {
        summary_lines,
        excerpt,
        warning_detected,
    }
}

fn evaluate_rule(rule: ToolRule, cmd_name: &str, lines: &[&str], exit_code: i32) -> RuleResult {
    let command_match = matches_command(rule.commands, cmd_name);
    let marker_hits = count_marker_hits(lines, rule.markers);

    let mut confidence: i32 = i32::from(rule.base_confidence);
    if command_match {
        confidence += 20;
    }
    confidence += (marker_hits.min(6) as i32) * 6;
    if exit_code == 0 {
        confidence -= 20;
    }
    if !command_match && marker_hits == 0 {
        confidence = 0;
    }

    let summary = build_summary(rule, lines, exit_code, marker_hits);
    let excerpt_lines = build_excerpt(rule, lines, exit_code, command_match, marker_hits);

    RuleResult {
        detector: rule.id,
        tool: rule.tool,
        confidence: confidence.clamp(0, 100) as u8,
        marker_hits,
        summary,
        excerpt_lines,
    }
}

fn build_summary(
    rule: ToolRule,
    lines: &[&str],
    exit_code: i32,
    marker_hits: usize,
) -> Vec<String> {
    let mut summary = Vec::new();
    if exit_code == 0 {
        if marker_hits > 0 {
            summary.push("command completed with warnings".to_string());
            summary.push(format!("matched {marker_hits} {} markers", rule.id));
            if let Some(line) = find_last_line_with_any(lines, rule.summary_markers) {
                summary.push(line.trim().to_string());
            }
        } else {
            summary.push("command completed successfully".to_string());
        }
        return summary;
    }

    summary.push(format!("command failed with exit code {exit_code}"));
    if marker_hits > 0 {
        summary.push(format!("matched {marker_hits} {} markers", rule.id));
    }

    if let Some(line) = find_last_line_with_any(lines, rule.summary_markers) {
        summary.push(line.trim().to_string());
    }

    summary
}

fn build_excerpt(
    rule: ToolRule,
    lines: &[&str],
    exit_code: i32,
    command_match: bool,
    marker_hits: usize,
) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    if exit_code == 0 && marker_hits == 0 {
        return Vec::new();
    }

    let start = find_first_line_with_any(lines, rule.excerpt_start)
        .or_else(|| find_first_line_with_any(lines, rule.markers));

    if let Some(start_idx) = start {
        let end = find_end_index(lines, start_idx, rule.excerpt_end)
            .unwrap_or_else(|| usize::min(start_idx + 120, lines.len()));
        return lines[start_idx..end]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }

    if exit_code != 0 && command_match {
        let start_idx = lines.len().saturating_sub(80);
        return lines[start_idx..]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    }

    Vec::new()
}

fn find_end_index(lines: &[&str], start_idx: usize, markers: &[&str]) -> Option<usize> {
    if markers.is_empty() {
        return None;
    }

    lines
        .iter()
        .enumerate()
        .skip(start_idx + 1)
        .find(|(_, line)| contains_any(line, markers))
        .map(|(idx, _)| idx)
}

fn should_replace(current: Option<&RuleResult>, candidate: &RuleResult) -> bool {
    match current {
        None => candidate.confidence > 0,
        Some(existing) => candidate.confidence > existing.confidence,
    }
}

fn detector_enabled(id: &str, enabled: &[String]) -> bool {
    if enabled.is_empty() {
        return true;
    }
    enabled.iter().any(|entry| entry == id)
}

fn matches_command(commands: &[&str], cmd_name: &str) -> bool {
    commands.is_empty() || commands.iter().any(|name| *name == cmd_name)
}

fn count_marker_hits(lines: &[&str], markers: &[&str]) -> usize {
    if markers.is_empty() {
        return 0;
    }

    lines
        .iter()
        .filter(|line| contains_any(line, markers))
        .count()
}

fn find_first_line_with_any(lines: &[&str], markers: &[&str]) -> Option<usize> {
    if markers.is_empty() {
        return None;
    }

    lines.iter().position(|line| contains_any(line, markers))
}

fn find_last_line_with_any(lines: &[&str], markers: &[&str]) -> Option<String> {
    if markers.is_empty() {
        return None;
    }

    lines
        .iter()
        .rev()
        .find(|line| contains_any(line, markers))
        .map(|line| (*line).to_string())
}

fn contains_any(line: &str, markers: &[&str]) -> bool {
    let lower = line.to_ascii_lowercase();
    markers.iter().any(|marker| lower.contains(marker))
}

fn command_basename(command: &[String]) -> &str {
    command
        .first()
        .and_then(|cmd| Path::new(cmd).file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("")
}

fn strip_ansi(input: &str) -> Cow<'_, str> {
    // Typical CSI ANSI escape sequences; keep parser simple for MVP.
    let ansi_re = Regex::new(r"\x1B\[[0-9;?]*[ -/]*[@-~]").expect("valid ansi regex");
    ansi_re.replace_all(input, "")
}

fn tool_rules() -> &'static [ToolRule] {
    &[
        ToolRule {
            id: "pytest",
            tool: Some("pytest"),
            commands: &["pytest"],
            markers: &["failures", "collected", "short test summary info"],
            summary_markers: &[" failed", " passed", " skipped", " xfailed", "error"],
            excerpt_start: &["failures"],
            excerpt_end: &["short test summary info", "===="],
            base_confidence: 68,
        },
        ToolRule {
            id: "jest",
            tool: Some("jest"),
            commands: &["jest"],
            markers: &["fail ", "test suites:", "tests:"],
            summary_markers: &["test suites:", "tests:"],
            excerpt_start: &["fail "],
            excerpt_end: &["test suites:"],
            base_confidence: 68,
        },
        ToolRule {
            id: "vitest",
            tool: Some("vitest"),
            commands: &["vitest"],
            markers: &["failed tests", "test files", "vitest"],
            summary_markers: &["failed", "passed", "test files"],
            excerpt_start: &["failed tests", "error"],
            excerpt_end: &["test files", "duration"],
            base_confidence: 64,
        },
        ToolRule {
            id: "cargo",
            tool: Some("cargo"),
            commands: &["cargo"],
            markers: &["error:", "test result:", "failures:"],
            summary_markers: &["test result:", "error:"],
            excerpt_start: &["error:", "failures:"],
            excerpt_end: &["test result:", "error: could not compile"],
            base_confidence: 62,
        },
        ToolRule {
            id: "go",
            tool: Some("go"),
            commands: &["go"],
            markers: &["--- fail:", "fail\t", "panic:", "build failed"],
            summary_markers: &["fail\t", "ok\t", "?\t"],
            excerpt_start: &["--- fail:", "panic:"],
            excerpt_end: &["fail\t", "exit status"],
            base_confidence: 62,
        },
        ToolRule {
            id: "tsc",
            tool: Some("typescript"),
            commands: &["tsc"],
            markers: &["error ts", "found ", "errors"],
            summary_markers: &["error ts", "found", "errors"],
            excerpt_start: &["error ts"],
            excerpt_end: &["found", "errors"],
            base_confidence: 60,
        },
        ToolRule {
            id: "eslint",
            tool: Some("eslint"),
            commands: &["eslint"],
            markers: &["problems (", "error", "warning"],
            summary_markers: &["problems (", "✖"],
            excerpt_start: &["error", "warning"],
            excerpt_end: &["problems (", "✖"],
            base_confidence: 58,
        },
        ToolRule {
            id: "ruff",
            tool: Some("ruff"),
            commands: &["ruff"],
            markers: &["found", "error", "would fix"],
            summary_markers: &["found", "would fix", "all checks passed"],
            excerpt_start: &["error", "found"],
            excerpt_end: &["found", "would fix"],
            base_confidence: 58,
        },
        ToolRule {
            id: "mypy",
            tool: Some("mypy"),
            commands: &["mypy"],
            markers: &[": error:", "found ", "error in"],
            summary_markers: &["found ", "success: no issues found"],
            excerpt_start: &[": error:"],
            excerpt_end: &["found", "error in"],
            base_confidence: 58,
        },
        ToolRule {
            id: "maven",
            tool: Some("maven"),
            commands: &["mvn"],
            markers: &["[error]", "build failure", "failed to execute goal"],
            summary_markers: &["build failure", "[error]"],
            excerpt_start: &["[error]"],
            excerpt_end: &["[help", "[info]"],
            base_confidence: 66,
        },
        ToolRule {
            id: "gradle",
            tool: Some("gradle"),
            commands: &["gradle"],
            markers: &[
                "build failed",
                "failure: build failed with an exception",
                "what went wrong",
            ],
            summary_markers: &["build failed", "what went wrong"],
            excerpt_start: &["failure: build failed with an exception", "what went wrong"],
            excerpt_end: &["* try:", "build failed"],
            base_confidence: 66,
        },
        ToolRule {
            id: "dotnet",
            tool: Some("dotnet"),
            commands: &["dotnet"],
            markers: &["build failed", "test run failed", "error cs", "failed!"],
            summary_markers: &["build failed", "test run failed", "failed!"],
            excerpt_start: &["error cs", "failed!", "test run failed"],
            excerpt_end: &["build failed", "total tests:"],
            base_confidence: 62,
        },
        ToolRule {
            id: "cmake",
            tool: Some("cmake/ctest"),
            commands: &["cmake", "ctest"],
            markers: &[
                "cmake error",
                "the following tests failed:",
                "errors occurred",
            ],
            summary_markers: &["the following tests failed:", "errors occurred"],
            excerpt_start: &["cmake error", "the following tests failed:"],
            excerpt_end: &["-- configuring incomplete", "errors occurred"],
            base_confidence: 56,
        },
        ToolRule {
            id: "terraform",
            tool: Some("terraform"),
            commands: &["terraform"],
            markers: &["error:", "failed", "planning failed", "apply complete!"],
            summary_markers: &["error:", "planning failed", "apply complete!"],
            excerpt_start: &["error:"],
            excerpt_end: &["terraform used", "╵", "exit status"],
            base_confidence: 56,
        },
        ToolRule {
            id: "docker",
            tool: Some("docker"),
            commands: &["docker", "docker-compose", "compose"],
            markers: &["error", "failed to", "executor failed", "service"],
            summary_markers: &["error", "failed to"],
            excerpt_start: &["error", "failed to"],
            excerpt_end: &["executor failed", "service"],
            base_confidence: 55,
        },
        ToolRule {
            id: "kubectl",
            tool: Some("kubectl"),
            commands: &["kubectl"],
            markers: &["error from server", "unable to", "forbidden", "not found"],
            summary_markers: &["error from server", "unable to", "forbidden", "not found"],
            excerpt_start: &["error from server", "unable to", "forbidden"],
            excerpt_end: &[],
            base_confidence: 55,
        },
        ToolRule {
            id: "generic",
            tool: None,
            commands: &[],
            markers: &[
                "panic:",
                "panicked at",
                "traceback (most recent call last):",
                "exception in thread",
                "caused by:",
                "segmentation fault",
                "out of memory",
                "killed",
                "error:",
            ],
            summary_markers: &["error:", "exception", "panic", "traceback"],
            excerpt_start: &[
                "panic:",
                "traceback (most recent call last):",
                "exception",
                "error:",
            ],
            excerpt_end: &[],
            base_confidence: 30,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn strips_ansi_sequences() {
        let input = "\u{1b}[31merror\u{1b}[0m";
        assert_eq!(strip_ansi(input), "error");
    }

    #[test]
    fn command_bias_prefers_pytest_over_generic() {
        let lines = vec![
            "=========================== FAILURES ===========================",
            "FAILED test_x.py::test_nope - AssertionError",
        ];
        let pytest_rule = tool_rules()
            .iter()
            .find(|r| r.id == "pytest")
            .copied()
            .expect("pytest rule");
        let generic_rule = tool_rules()
            .iter()
            .find(|r| r.id == "generic")
            .copied()
            .expect("generic rule");

        let pytest = evaluate_rule(pytest_rule, "pytest", &lines, 1);
        let generic = evaluate_rule(generic_rule, "pytest", &lines, 1);

        assert!(pytest.confidence > generic.confidence);
    }

    #[test]
    fn fixture_suite_routes_to_expected_detectors() {
        let cases = [
            ("pytest_failure.log", vec!["pytest"], "pytest"),
            ("jest_failure.log", vec!["jest"], "jest"),
            ("vitest_failure.log", vec!["vitest"], "vitest"),
            ("cargo_failure.log", vec!["cargo", "test"], "cargo"),
            ("go_failure.log", vec!["go", "test"], "go"),
            ("tsc_failure.log", vec!["tsc"], "tsc"),
            ("eslint_failure.log", vec!["eslint"], "eslint"),
            (
                "terraform_failure.log",
                vec!["terraform", "plan"],
                "terraform",
            ),
            ("docker_failure.log", vec!["docker", "build"], "docker"),
            ("kubectl_failure.log", vec!["kubectl", "apply"], "kubectl"),
        ];

        for (fixture, command, expected_detector) in cases {
            let text = read_fixture(fixture);
            let command_vec = command.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let enabled = tool_rules()
                .iter()
                .map(|r| r.id.to_string())
                .collect::<Vec<_>>();
            let result = analyze_text(&text, 1, &enabled, &command_vec);

            let detector_line = result
                .summary_lines
                .iter()
                .find(|line| line.starts_with("detector: "))
                .cloned()
                .unwrap_or_default();

            assert_eq!(
                detector_line,
                format!("detector: {expected_detector}"),
                "fixture {fixture} selected wrong detector"
            );
            assert!(
                result
                    .excerpt
                    .as_ref()
                    .is_some_and(|e| !e.trim().is_empty()),
                "fixture {fixture} should produce non-empty excerpt"
            );
        }
    }

    #[test]
    fn warning_detection_on_success_works_for_eslint() {
        let text = "src/main.ts\n  7:3  warning  Unexpected console statement  no-console\n\n✖ 1 problem (0 errors, 1 warning)\n";
        let enabled = tool_rules()
            .iter()
            .map(|r| r.id.to_string())
            .collect::<Vec<_>>();
        let command_vec = vec!["eslint".to_string(), ".".to_string()];
        let result = analyze_text(text, 0, &enabled, &command_vec);

        assert!(result.warning_detected);
        assert!(
            result
                .summary_lines
                .first()
                .is_some_and(|line| line == "command completed with warnings")
        );
        assert!(
            result
                .excerpt
                .as_ref()
                .is_some_and(|e| e.contains("warning"))
        );
    }

    fn read_fixture(name: &str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/fixtures");
        path.push(name);
        fs::read_to_string(&path).expect("fixture file")
    }
}
