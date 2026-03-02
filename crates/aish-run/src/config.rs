use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub store: StoreConfig,
}

impl AppConfig {
    pub fn default() -> Self {
        let root = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/state/aish");

        Self {
            store: StoreConfig { root },
        }
    }
}
