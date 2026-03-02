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

pub fn analyze_log(log_path: &Path, exit_code: i32) -> AnalysisResult {
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

    let mut detectors: Vec<Box<dyn Detector>> = vec![Box::new(GenericErrorDetector::new())];
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
