use crate::cli::ShowMode;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub root: PathBuf,
    pub keep_days: u32,
    pub max_total_mb: u64,
}

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub mode: ShowMode,
    pub max_excerpt_lines: usize,
    pub max_digest_lines: usize,
    pub show_log_path: bool,
}

#[derive(Debug, Clone)]
pub struct WrapConfig {
    pub default_mode: String,
    pub commands: Vec<String>,
    pub skip_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DetectorsConfig {
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub match_cmd: String,
    pub show: Option<ShowMode>,
    pub excerpt_on_success: Option<bool>,
    pub max_excerpt_lines: Option<usize>,
    pub max_digest_lines: Option<usize>,
    pub args_prefix: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub store: StoreConfig,
    pub output: OutputConfig,
    pub wrap: WrapConfig,
    pub detectors: DetectorsConfig,
    pub policies: Vec<PolicyConfig>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    store: Option<RawStoreConfig>,
    output: Option<RawOutputConfig>,
    wrap: Option<RawWrapConfig>,
    detectors: Option<RawDetectorsConfig>,
    #[serde(default, rename = "policy")]
    policies: Vec<RawPolicyConfig>,
}

#[derive(Debug, Deserialize)]
struct RawStoreConfig {
    root: Option<String>,
    keep_days: Option<u32>,
    max_total_mb: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawOutputConfig {
    mode: Option<String>,
    max_excerpt_lines: Option<usize>,
    max_digest_lines: Option<usize>,
    show_log_path: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawWrapConfig {
    default: Option<String>,
    commands: Option<Vec<String>>,
    skip_commands: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawDetectorsConfig {
    enabled: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawPolicyConfig {
    #[serde(rename = "match")]
    match_cmd: String,
    show: Option<String>,
    excerpt_on_success: Option<bool>,
    max_excerpt_lines: Option<usize>,
    max_digest_lines: Option<usize>,
    args_prefix: Option<Vec<String>>,
}

impl AppConfig {
    pub fn load() -> Result<Self, String> {
        let default = Self::default();
        let path = default_config_path();
        if !path.exists() {
            return Ok(default);
        }

        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read config {}: {e}", path.display()))?;
        let parsed: RawConfig = toml::from_str(&raw)
            .map_err(|e| format!("failed to parse config {}: {e}", path.display()))?;

        Ok(merge_config(default, parsed))
    }

    pub fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            store: StoreConfig {
                root: home.join(".local/state/aish"),
                keep_days: 14,
                max_total_mb: 2000,
            },
            output: OutputConfig {
                mode: ShowMode::Auto,
                max_excerpt_lines: 200,
                max_digest_lines: 3,
                show_log_path: true,
            },
            wrap: WrapConfig {
                default_mode: "off".to_string(),
                commands: vec![
                    "pytest".to_string(),
                    "jest".to_string(),
                    "gradle".to_string(),
                    "mvn".to_string(),
                    "go".to_string(),
                    "cargo".to_string(),
                    "npm".to_string(),
                    "pnpm".to_string(),
                    "yarn".to_string(),
                ],
                skip_commands: vec![
                    "cat".to_string(),
                    "less".to_string(),
                    "more".to_string(),
                    "man".to_string(),
                    "ssh".to_string(),
                    "vim".to_string(),
                    "nano".to_string(),
                    "top".to_string(),
                    "htop".to_string(),
                ],
            },
            detectors: DetectorsConfig {
                enabled: vec![
                    "generic".to_string(),
                    "pytest".to_string(),
                    "jest".to_string(),
                    "gradle".to_string(),
                    "maven".to_string(),
                ],
            },
            policies: Vec::new(),
        }
    }
}

fn merge_config(default: AppConfig, raw: RawConfig) -> AppConfig {
    let store = StoreConfig {
        root: raw
            .store
            .as_ref()
            .and_then(|s| s.root.as_deref())
            .map(expand_home)
            .unwrap_or(default.store.root),
        keep_days: raw
            .store
            .as_ref()
            .and_then(|s| s.keep_days)
            .unwrap_or(default.store.keep_days),
        max_total_mb: raw
            .store
            .as_ref()
            .and_then(|s| s.max_total_mb)
            .unwrap_or(default.store.max_total_mb),
    };

    let output = OutputConfig {
        mode: raw
            .output
            .as_ref()
            .and_then(|o| o.mode.as_deref())
            .and_then(parse_show_mode)
            .unwrap_or(default.output.mode),
        max_excerpt_lines: raw
            .output
            .as_ref()
            .and_then(|o| o.max_excerpt_lines)
            .unwrap_or(default.output.max_excerpt_lines),
        max_digest_lines: raw
            .output
            .as_ref()
            .and_then(|o| o.max_digest_lines)
            .unwrap_or(default.output.max_digest_lines),
        show_log_path: raw
            .output
            .as_ref()
            .and_then(|o| o.show_log_path)
            .unwrap_or(default.output.show_log_path),
    };

    let wrap = WrapConfig {
        default_mode: raw
            .wrap
            .as_ref()
            .and_then(|w| w.default.clone())
            .unwrap_or(default.wrap.default_mode),
        commands: raw
            .wrap
            .as_ref()
            .and_then(|w| w.commands.clone())
            .unwrap_or(default.wrap.commands),
        skip_commands: raw
            .wrap
            .as_ref()
            .and_then(|w| w.skip_commands.clone())
            .unwrap_or(default.wrap.skip_commands),
    };

    let detectors = DetectorsConfig {
        enabled: raw
            .detectors
            .as_ref()
            .and_then(|d| d.enabled.clone())
            .unwrap_or(default.detectors.enabled),
    };

    let policies = raw
        .policies
        .into_iter()
        .map(|p| PolicyConfig {
            match_cmd: p.match_cmd,
            show: p.show.as_deref().and_then(parse_show_mode),
            excerpt_on_success: p.excerpt_on_success,
            max_excerpt_lines: p.max_excerpt_lines,
            max_digest_lines: p.max_digest_lines,
            args_prefix: p.args_prefix,
        })
        .collect();

    AppConfig {
        store,
        output,
        wrap,
        detectors,
        policies,
    }
}

fn default_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("AISH_CONFIG") {
        return PathBuf::from(path);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/aish/config.toml")
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path));
    }

    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return Path::new(&home).join(rest);
        }
    }

    PathBuf::from(path)
}

fn parse_show_mode(value: &str) -> Option<ShowMode> {
    match value {
        "auto" => Some(ShowMode::Auto),
        "digest" => Some(ShowMode::Digest),
        "excerpt" => Some(ShowMode::Excerpt),
        "full" => Some(ShowMode::Full),
        "quiet" => Some(ShowMode::Quiet),
        _ => None,
    }
}
