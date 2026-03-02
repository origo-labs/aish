use crate::cli::Cli;
use crate::{config, detectors, pty, render, store};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Serialize)]
struct RunMeta {
    id: String,
    timestamp_start: String,
    timestamp_end: String,
    duration_ms: i128,
    cwd: String,
    command_argv: Vec<String>,
    exit_code: i32,
    success: bool,
    status_text: String,
    env: EnvMeta,
}

#[derive(Debug, Serialize)]
struct EnvMeta {
    term: Option<String>,
    colorterm: Option<String>,
    ci: Option<String>,
}

pub fn run(args: &Cli) -> Result<i32, String> {
    let started = OffsetDateTime::now_utc();
    let cwd = std::env::current_dir().map_err(|e| format!("failed to get cwd: {e}"))?;

    let base_config = config::AppConfig::default();
    let root = args
        .log_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or(base_config.store.root.clone());

    fs::create_dir_all(&root).map_err(|e| format!("failed to create store root: {e}"))?;
    let run_paths = store::prepare_run_dir(&root, started)
        .map_err(|e| format!("failed to prepare run dir: {e}"))?;

    let command_outcome = if args.no_pty {
        pty::run_without_pty(&args.command, &cwd, &run_paths.log_path)
            .map_err(|e| format!("failed to run command without pty: {e}"))?
    } else {
        pty::run_in_pty(&args.command, &cwd, &run_paths.log_path)
            .map_err(|e| format!("failed to run command in pty: {e}"))?
    };

    let ended = OffsetDateTime::now_utc();
    let duration_ms = (ended - started).whole_milliseconds();

    let analysis = detectors::analyze_log(&run_paths.log_path, command_outcome.exit_code);
    let mut digest =
        render::build_digest(command_outcome.success, duration_ms, &args.command, ended);
    if let Some(summary) = analysis.summary_lines.first() {
        digest.push_str(" | ");
        digest.push_str(summary);
    }
    fs::write(&run_paths.digest_path, &digest)
        .map_err(|e| format!("failed to write digest: {e}"))?;

    let excerpt = if !command_outcome.success {
        let detected = analysis
            .excerpt
            .unwrap_or_else(|| format!("command failed ({})", command_outcome.status_text));
        fs::write(&run_paths.relevant_path, &detected)
            .map_err(|e| format!("failed to write relevant excerpt: {e}"))?;
        Some(detected)
    } else {
        None
    };

    let id = run_paths
        .run_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let meta = RunMeta {
        id,
        timestamp_start: format_time(started),
        timestamp_end: format_time(ended),
        duration_ms,
        cwd: cwd.display().to_string(),
        command_argv: args.command.clone(),
        exit_code: command_outcome.exit_code,
        success: command_outcome.success,
        status_text: command_outcome.status_text,
        env: EnvMeta {
            term: std::env::var("TERM").ok(),
            colorterm: std::env::var("COLORTERM").ok(),
            ci: std::env::var("CI").ok(),
        },
    };

    let meta_json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("failed to serialize metadata: {e}"))?;
    fs::write(&run_paths.meta_path, meta_json)
        .map_err(|e| format!("failed to write metadata: {e}"))?;

    store::update_last_symlink(&run_paths.last_link, &run_paths.run_dir)
        .map_err(|e| format!("failed to update last symlink: {e}"))?;

    render::render_summary(render::RenderContext {
        show_mode: args.show.clone(),
        success: command_outcome.success,
        digest: &digest,
        excerpt: excerpt.as_deref(),
        log_path: &run_paths.log_path,
        max_excerpt_lines: base_config.output.max_excerpt_lines,
        max_digest_lines: base_config.output.max_digest_lines,
        show_log_path: base_config.output.show_log_path,
    });

    Ok(command_outcome.exit_code)
}

fn format_time(ts: OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".to_string())
}
