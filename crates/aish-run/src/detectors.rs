use regex::Regex;
use std::fs;
use std::path::Path;

pub trait Detector {
    fn name(&self) -> &'static str;
    fn observe_line(&mut self, line: &str);
    fn finalize(&self, exit_code: i32) -> DetectorResult;
}

#[derive(Debug, Clone)]
pub struct DetectorResult {
    pub detector: &'static str,
    pub tool: Option<String>,
    pub summary: Vec<String>,
    pub relevant: Vec<String>,
    pub confidence: u8,
}

#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub summary_lines: Vec<String>,
    pub excerpt: Option<String>,
}

pub fn analyze_log(
    log_path: &Path,
    exit_code: i32,
    enabled_detectors: &[String],
) -> AnalysisResult {
    let log_bytes = match fs::read(log_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return AnalysisResult {
                summary_lines: Vec::new(),
                excerpt: None,
            };
        }
    };

    let text = String::from_utf8_lossy(&log_bytes);
    let lines: Vec<&str> = text.lines().collect();

    let mut detectors: Vec<Box<dyn Detector>> = Vec::new();
    if detector_enabled("pytest", enabled_detectors) {
        detectors.push(Box::new(PytestDetector::new()));
    }
    if detector_enabled("jest", enabled_detectors) {
        detectors.push(Box::new(JestDetector::new()));
    }
    if detector_enabled("gradle", enabled_detectors) {
        detectors.push(Box::new(GradleDetector::new()));
    }
    if detector_enabled("maven", enabled_detectors) {
        detectors.push(Box::new(MavenDetector::new()));
    }
    if detector_enabled("generic", enabled_detectors) {
        detectors.push(Box::new(GenericErrorDetector::new()));
    }

    for line in &lines {
        for detector in &mut detectors {
            detector.observe_line(line);
        }
    }

    let mut best: Option<DetectorResult> = None;
    for detector in detectors {
        let result = detector.finalize(exit_code);
        if should_replace_best(best.as_ref(), &result) {
            best = Some(result);
        }
    }

    let summary_lines = best
        .as_ref()
        .map(|r| {
            let mut lines = r.summary.clone();
            lines.push(format!("detector: {}", r.detector));
            if let Some(tool) = &r.tool {
                lines.push(format!("tool: {tool}"));
            }
            lines
        })
        .unwrap_or_default();
    let excerpt = best.and_then(|r| (!r.relevant.is_empty()).then_some(r.relevant.join("\n")));

    AnalysisResult {
        summary_lines,
        excerpt,
    }
}

fn detector_enabled(name: &str, enabled: &[String]) -> bool {
    if enabled.is_empty() {
        return true;
    }
    enabled.iter().any(|item| item == name)
}

fn should_replace_best(current: Option<&DetectorResult>, candidate: &DetectorResult) -> bool {
    match current {
        None => true,
        Some(cur) => candidate.confidence > cur.confidence,
    }
}

struct GenericErrorDetector {
    lines: Vec<String>,
    first_error_line: Option<usize>,
    error_hits: usize,
    strong_markers: Vec<&'static str>,
    frame_regex: Regex,
}

impl GenericErrorDetector {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            first_error_line: None,
            error_hits: 0,
            strong_markers: vec![
                "thread '",
                "panicked at",
                "panic:",
                "traceback (most recent call last):",
                "exception in thread",
                "caused by:",
                "segmentation fault",
                "out of memory",
                "killed",
                "error:",
            ],
            frame_regex: Regex::new(r"([A-Za-z0-9_./\\-]+):(\\d+)").expect("valid frame regex"),
        }
    }

    fn is_error_marker(&self, line: &str) -> bool {
        let lower = line.to_ascii_lowercase();
        self.strong_markers
            .iter()
            .any(|marker| lower.contains(marker))
    }

    fn build_excerpt(&self, exit_code: i32) -> Vec<String> {
        if self.lines.is_empty() {
            return Vec::new();
        }

        let mut excerpt = Vec::new();

        if let Some(err_idx) = self.first_error_line {
            let start = err_idx.saturating_sub(25);
            let end = usize::min(err_idx + 80, self.lines.len());
            excerpt.extend(self.lines[start..end].iter().cloned());
        } else if exit_code != 0 {
            let start = self.lines.len().saturating_sub(80);
            excerpt.extend(self.lines[start..].iter().cloned());
        }

        let mut frames = Vec::new();
        for line in &self.lines {
            if self.frame_regex.is_match(line) {
                frames.push(line.clone());
            }
        }

        if !frames.is_empty() {
            excerpt.push(String::new());
            excerpt.push("stack-like frames:".to_string());
            for frame in frames.iter().take(20) {
                excerpt.push(frame.clone());
            }
        }

        excerpt
    }
}

impl Detector for GenericErrorDetector {
    fn name(&self) -> &'static str {
        "generic"
    }

    fn observe_line(&mut self, line: &str) {
        let idx = self.lines.len();
        self.lines.push(line.to_string());

        if self.is_error_marker(line) {
            self.error_hits += 1;
            if self.first_error_line.is_none() {
                self.first_error_line = Some(idx);
            }
        }
    }

    fn finalize(&self, exit_code: i32) -> DetectorResult {
        let mut summary = Vec::new();
        if exit_code == 0 {
            summary.push("command completed successfully".to_string());
        } else {
            summary.push(format!("command failed with exit code {exit_code}"));
            summary.push(format!("detected {} error markers", self.error_hits));
        }

        let confidence = if self.first_error_line.is_some() {
            60
        } else if exit_code != 0 {
            30
        } else {
            5
        };

        DetectorResult {
            detector: self.name(),
            tool: None,
            summary,
            relevant: self.build_excerpt(exit_code),
            confidence,
        }
    }
}

struct PytestDetector {
    lines: Vec<String>,
}

impl PytestDetector {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

impl Detector for PytestDetector {
    fn name(&self) -> &'static str {
        "pytest"
    }

    fn observe_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn finalize(&self, exit_code: i32) -> DetectorResult {
        let mut summary = Vec::new();
        let mut relevant = Vec::new();
        let mut confidence = 0;

        let failures_idx = self.lines.iter().position(|l| l.contains("FAILURES"));
        let summary_line = self
            .lines
            .iter()
            .rev()
            .find(|l| l.contains("failed") || l.contains("passed") || l.contains("skipped"));

        if failures_idx.is_some()
            || self
                .lines
                .iter()
                .any(|l| l.contains("collected") && l.contains("items"))
        {
            confidence = if failures_idx.is_some() { 90 } else { 40 };
            summary.push(format!("command failed with exit code {exit_code}"));
            if let Some(line) = summary_line {
                summary.push(line.trim().to_string());
            }
        }

        if let Some(idx) = failures_idx {
            let end = self
                .lines
                .iter()
                .skip(idx + 1)
                .position(|l| l.contains("short test summary info") || l.contains("===="))
                .map(|offset| idx + 1 + offset)
                .unwrap_or_else(|| usize::min(idx + 120, self.lines.len()));
            let bounded_end = usize::min(end, self.lines.len());
            relevant.extend(self.lines[idx..bounded_end].iter().cloned());
        }

        DetectorResult {
            detector: self.name(),
            tool: Some("pytest".to_string()),
            summary,
            relevant,
            confidence,
        }
    }
}

struct JestDetector {
    lines: Vec<String>,
}

impl JestDetector {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

impl Detector for JestDetector {
    fn name(&self) -> &'static str {
        "jest"
    }

    fn observe_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn finalize(&self, exit_code: i32) -> DetectorResult {
        let mut summary = Vec::new();
        let mut relevant = Vec::new();
        let mut confidence = 0;

        let first_fail = self
            .lines
            .iter()
            .position(|l| l.starts_with("FAIL ") || l.trim_start().starts_with("FAIL "));
        let suites = self.lines.iter().rev().find(|l| l.contains("Test Suites:"));
        let tests = self.lines.iter().rev().find(|l| l.contains("Tests:"));

        if first_fail.is_some() || suites.is_some() {
            confidence = if first_fail.is_some() { 88 } else { 35 };
            summary.push(format!("command failed with exit code {exit_code}"));
            if let Some(s) = suites {
                summary.push(s.trim().to_string());
            }
            if let Some(t) = tests {
                summary.push(t.trim().to_string());
            }
        }

        if let Some(idx) = first_fail {
            let end = self
                .lines
                .iter()
                .skip(idx + 1)
                .position(|l| l.contains("Test Suites:"))
                .map(|offset| idx + 1 + offset + 1)
                .unwrap_or_else(|| usize::min(idx + 120, self.lines.len()));
            relevant.extend(
                self.lines[idx..usize::min(end, self.lines.len())]
                    .iter()
                    .cloned(),
            );
        }

        DetectorResult {
            detector: self.name(),
            tool: Some("jest".to_string()),
            summary,
            relevant,
            confidence,
        }
    }
}

struct GradleDetector {
    lines: Vec<String>,
}

impl GradleDetector {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

impl Detector for GradleDetector {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn observe_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn finalize(&self, exit_code: i32) -> DetectorResult {
        let mut summary = Vec::new();
        let mut relevant = Vec::new();
        let failure_idx = self
            .lines
            .iter()
            .position(|l| l.contains("FAILURE: Build failed with an exception."));
        let build_failed = self.lines.iter().any(|l| l.contains("BUILD FAILED"));

        let confidence = if failure_idx.is_some() {
            86
        } else if build_failed {
            50
        } else {
            0
        };

        if confidence > 0 {
            summary.push(format!("command failed with exit code {exit_code}"));
            summary.push("gradle build failure detected".to_string());
        }

        if let Some(idx) = failure_idx {
            let end = self
                .lines
                .iter()
                .skip(idx + 1)
                .position(|l| l.contains("* Try:") || l.contains("BUILD FAILED"))
                .map(|offset| idx + 1 + offset + 1)
                .unwrap_or_else(|| usize::min(idx + 120, self.lines.len()));
            relevant.extend(
                self.lines[idx..usize::min(end, self.lines.len())]
                    .iter()
                    .cloned(),
            );
        }

        DetectorResult {
            detector: self.name(),
            tool: Some("gradle".to_string()),
            summary,
            relevant,
            confidence,
        }
    }
}

struct MavenDetector {
    lines: Vec<String>,
}

impl MavenDetector {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

impl Detector for MavenDetector {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn observe_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    fn finalize(&self, exit_code: i32) -> DetectorResult {
        let mut summary = Vec::new();
        let mut relevant = Vec::new();
        let first_error = self.lines.iter().position(|l| l.starts_with("[ERROR]"));
        let build_failure = self.lines.iter().any(|l| l.contains("BUILD FAILURE"));

        let confidence = if first_error.is_some() {
            84
        } else if build_failure {
            45
        } else {
            0
        };

        if confidence > 0 {
            summary.push(format!("command failed with exit code {exit_code}"));
            summary.push("maven build failure detected".to_string());
        }

        if let Some(idx) = first_error {
            let end = self
                .lines
                .iter()
                .skip(idx + 1)
                .position(|l| !l.starts_with("[ERROR]") && !l.trim().is_empty())
                .map(|offset| idx + 1 + offset)
                .unwrap_or_else(|| usize::min(idx + 60, self.lines.len()));
            relevant.extend(
                self.lines[idx..usize::min(end, self.lines.len())]
                    .iter()
                    .cloned(),
            );
        }

        DetectorResult {
            detector: self.name(),
            tool: Some("maven".to_string()),
            summary,
            relevant,
            confidence,
        }
    }
}
