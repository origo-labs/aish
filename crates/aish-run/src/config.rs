use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub max_excerpt_lines: usize,
    pub max_digest_lines: usize,
    pub show_log_path: bool,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub store: StoreConfig,
    pub output: OutputConfig,
}

impl AppConfig {
    pub fn default() -> Self {
        let root = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/state/aish");

        Self {
            store: StoreConfig { root },
            output: OutputConfig {
                max_excerpt_lines: 200,
                max_digest_lines: 3,
                show_log_path: true,
            },
        }
    }
}
