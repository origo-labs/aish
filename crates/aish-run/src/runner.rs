use crate::cli::{Cli, ShowMode};
use crate::{config, detectors, policy, pty, render, store};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
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

    let app_config = config::AppConfig::load()?;
    let root = args
        .log_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or(app_config.store.root.clone());
    let effective_policy = policy::resolve(&args.command, &app_config, args.show.clone());
    let stream_output = matches!(effective_policy.show_mode, ShowMode::Full);

    fs::create_dir_all(&root).map_err(|e| format!("failed to create store root: {e}"))?;
    let run_paths = store::prepare_run_dir(&root, started)
        .map_err(|e| format!("failed to prepare run dir: {e}"))?;

    let command_outcome = if args.no_pty {
        pty::run_without_pty(&args.command, &cwd, &run_paths.log_path, stream_output)
            .map_err(|e| format!("failed to run command without pty: {e}"))?
    } else {
        pty::run_in_pty(&args.command, &cwd, &run_paths.log_path, stream_output)
            .map_err(|e| format!("failed to run command in pty: {e}"))?
    };
    let has_log_output = cleanup_empty_log(&run_paths.log_path)
        .map_err(|e| format!("failed to clean up log file: {e}"))?;

    let ended = OffsetDateTime::now_utc();
    let duration_ms = (ended - started).whole_milliseconds();

    let analysis = detectors::analyze_log(
        &run_paths.log_path,
        command_outcome.exit_code,
        &app_config.detectors.enabled,
        &args.command,
    );
    let mut digest =
        render::build_digest(command_outcome.success, duration_ms, &args.command, ended);
    if let Some(summary) = analysis.summary_lines.first() {
        digest.push_str(" | ");
        digest.push_str(summary);
    }
    fs::write(&run_paths.digest_path, &digest)
        .map_err(|e| format!("failed to write digest: {e}"))?;

    let should_show_warning_excerpt = should_show_warning_excerpt(
        command_outcome.success,
        analysis.warning_detected,
        effective_policy.show_warnings_on_success,
    );
    let should_show_success_highlight = should_show_success_highlight(
        command_outcome.success,
        analysis.success_highlight_detected,
    );
    let excerpt = if should_write_relevant_excerpt(
        command_outcome.success,
        effective_policy.excerpt_on_success,
        should_show_warning_excerpt,
        should_show_success_highlight,
    ) {
        let detected = analysis.excerpt.unwrap_or_else(|| {
            if command_outcome.success {
                "command completed with warnings".to_string()
            } else {
                format!("command failed ({})", command_outcome.status_text)
            }
        });
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
    store::enforce_retention(
        &root,
        app_config.store.keep_days,
        app_config.store.max_total_mb,
        &run_paths.run_dir,
    )
    .map_err(|e| format!("failed to enforce retention policy: {e}"))?;

    render::render_summary(render::RenderContext {
        show_mode: effective_policy.show_mode,
        success: command_outcome.success,
        digest: &digest,
        excerpt: excerpt.as_deref(),
        log_path: &run_paths.log_path,
        max_excerpt_lines: effective_policy.max_excerpt_lines,
        max_digest_lines: effective_policy.max_digest_lines,
        show_log_path: effective_policy.show_log_path && has_log_output,
        show_excerpt_on_success: effective_policy.excerpt_on_success
            || should_show_warning_excerpt
            || should_show_success_highlight,
    });

    Ok(command_outcome.exit_code)
}

fn should_show_warning_excerpt(
    success: bool,
    warning_detected: bool,
    show_warnings_on_success: bool,
) -> bool {
    success && warning_detected && show_warnings_on_success
}

fn should_write_relevant_excerpt(
    success: bool,
    excerpt_on_success: bool,
    warning_excerpt_on_success: bool,
    success_highlight_on_success: bool,
) -> bool {
    !success || excerpt_on_success || warning_excerpt_on_success || success_highlight_on_success
}

fn should_show_success_highlight(success: bool, success_highlight_detected: bool) -> bool {
    success && success_highlight_detected
}

fn format_time(ts: OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".to_string())
}

fn cleanup_empty_log(log_path: &std::path::Path) -> std::io::Result<bool> {
    let metadata = match fs::metadata(log_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    if metadata.len() == 0 {
        fs::remove_file(log_path)?;
        return Ok(false);
    }

    Ok(true)
}

pub fn show_last(cfg: &config::AppConfig) -> Result<i32, String> {
    let last_dir = resolve_last_run_dir(cfg)?;
    let relevant = last_dir.join("relevant.txt");
    let digest = last_dir.join("digest.txt");

    if relevant.exists() {
        let content = fs::read_to_string(&relevant)
            .map_err(|e| format!("failed to read {}: {e}", relevant.display()))?;
        println!("{content}");
        return Ok(0);
    }

    if digest.exists() {
        let content = fs::read_to_string(&digest)
            .map_err(|e| format!("failed to read {}: {e}", digest.display()))?;
        println!("{content}");
        return Ok(0);
    }

    Err(format!(
        "no relevant.txt or digest.txt found in last run: {}",
        last_dir.display()
    ))
}

pub fn open_last(cfg: &config::AppConfig) -> Result<i32, String> {
    let last_dir = resolve_last_run_dir(cfg)?;
    let log_path = last_dir.join("pty.log");
    if !log_path.exists() {
        return Err(format!("missing log file: {}", log_path.display()));
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let status = Command::new(&pager)
        .arg(&log_path)
        .status()
        .or_else(|_| Command::new("cat").arg(&log_path).status())
        .map_err(|e| format!("failed to open {}: {e}", log_path.display()))?;

    Ok(status.code().unwrap_or(1))
}

fn resolve_last_run_dir(cfg: &config::AppConfig) -> Result<PathBuf, String> {
    let last_link = cfg.store.root.join("last");
    if !last_link.exists() {
        return Err(format!("last run link not found: {}", last_link.display()));
    }

    let run_dir = fs::read_link(&last_link)
        .map_err(|e| format!("failed to resolve {}: {e}", last_link.display()))?;
    Ok(run_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn warning_excerpt_only_when_all_conditions_match() {
        assert!(should_show_warning_excerpt(true, true, true));
        assert!(!should_show_warning_excerpt(false, true, true));
        assert!(!should_show_warning_excerpt(true, false, true));
        assert!(!should_show_warning_excerpt(true, true, false));
    }

    #[test]
    fn relevant_excerpt_write_conditions_match_policy() {
        assert!(should_write_relevant_excerpt(false, false, false, false));
        assert!(should_write_relevant_excerpt(true, true, false, false));
        assert!(should_write_relevant_excerpt(true, false, true, false));
        assert!(should_write_relevant_excerpt(true, false, false, true));
        assert!(!should_write_relevant_excerpt(true, false, false, false));
    }

    #[test]
    fn success_highlight_only_on_success() {
        assert!(should_show_success_highlight(true, true));
        assert!(!should_show_success_highlight(false, true));
        assert!(!should_show_success_highlight(true, false));
    }

    #[test]
    fn cleanup_empty_log_removes_zero_byte_file() {
        let path = unique_temp_file("empty");
        fs::write(&path, "").expect("failed to create empty temp log");

        assert!(
            !cleanup_empty_log(&path).expect("cleanup failed for empty log"),
            "expected no retained log output for empty file"
        );
        assert!(
            !path.exists(),
            "expected empty log file to be removed from disk"
        );
    }

    #[test]
    fn cleanup_empty_log_keeps_non_empty_file() {
        let path = unique_temp_file("nonempty");
        fs::write(&path, "hello").expect("failed to create non-empty temp log");

        assert!(
            cleanup_empty_log(&path).expect("cleanup failed for non-empty log"),
            "expected non-empty log to be retained"
        );
        assert!(path.exists(), "expected non-empty log file to remain");

        fs::remove_file(path).expect("failed to remove non-empty temp log");
    }

    fn unique_temp_file(suffix: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("aish-runner-{suffix}-{nanos}.log"))
    }
}
