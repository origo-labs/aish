use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum ShowMode {
    Auto,
    Digest,
    Excerpt,
    Full,
    Quiet,
}

#[derive(Debug, Parser)]
#[command(
    name = "aish-run",
    about = "AISH command runner",
    disable_version_flag = true
)]
pub struct Cli {
    /// Output mode for terminal rendering.
    #[arg(long, value_enum)]
    pub show: Option<ShowMode>,

    /// Disable PTY execution and use non-interactive execution.
    #[arg(long)]
    pub no_pty: bool,

    /// Override log directory for this run.
    #[arg(long)]
    pub log_dir: Option<String>,

    /// Optional label used for grouping runs.
    #[arg(long)]
    pub label: Option<String>,

    /// Print version information.
    #[arg(long)]
    pub version_info: bool,

    /// Print shell function shims derived from config wrap commands.
    #[arg(long)]
    pub print_shims: bool,

    /// Print last run relevant excerpt (or digest fallback).
    #[arg(long)]
    pub last: bool,

    /// Open last run full log in pager.
    #[arg(long)]
    pub open: bool,

    /// Command and args to execute; pass after `--`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}
