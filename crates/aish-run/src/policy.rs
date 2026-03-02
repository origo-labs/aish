use crate::cli::ShowMode;
use crate::config::{AppConfig, PolicyConfig};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct EffectivePolicy {
    pub show_mode: ShowMode,
    pub max_excerpt_lines: usize,
    pub max_digest_lines: usize,
    pub show_log_path: bool,
    pub excerpt_on_success: bool,
}

pub fn resolve(
    command: &[String],
    config: &AppConfig,
    cli_show: Option<ShowMode>,
) -> EffectivePolicy {
    let cli_show_override = cli_show.clone();
    let mut effective = EffectivePolicy {
        show_mode: cli_show.unwrap_or_else(|| config.output.mode.clone()),
        max_excerpt_lines: config.output.max_excerpt_lines,
        max_digest_lines: config.output.max_digest_lines,
        show_log_path: config.output.show_log_path,
        excerpt_on_success: false,
    };

    let cmd_name = command
        .first()
        .map(|c| basename(c))
        .unwrap_or_default()
        .to_string();
    let args = if command.len() > 1 {
        &command[1..]
    } else {
        &[][..]
    };

    for policy in &config.policies {
        if policy_matches(policy, &cmd_name, args) {
            if cli_show_override.is_none() {
                if let Some(mode) = &policy.show {
                    effective.show_mode = mode.clone();
                }
            }
            if let Some(lines) = policy.max_excerpt_lines {
                effective.max_excerpt_lines = lines;
            }
            if let Some(lines) = policy.max_digest_lines {
                effective.max_digest_lines = lines;
            }
            if let Some(on_success) = policy.excerpt_on_success {
                effective.excerpt_on_success = on_success;
            }
        }
    }

    effective
}

fn policy_matches(policy: &PolicyConfig, cmd_name: &str, args: &[String]) -> bool {
    if policy.match_cmd != cmd_name {
        return false;
    }

    if let Some(prefix) = &policy.args_prefix {
        if args.len() < prefix.len() {
            return false;
        }
        return prefix
            .iter()
            .zip(args.iter())
            .all(|(want, got)| want == got);
    }

    true
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}
