use crate::cli::ShowMode;
use std::path::Path;
use time::OffsetDateTime;

pub struct RenderContext<'a> {
    pub show_mode: ShowMode,
    pub success: bool,
    pub digest: &'a str,
    pub excerpt: Option<&'a str>,
    pub log_path: &'a Path,
    pub max_excerpt_lines: usize,
    pub max_digest_lines: usize,
    pub show_log_path: bool,
}

pub fn build_digest(
    success: bool,
    duration_ms: i128,
    command: &[String],
    timestamp: OffsetDateTime,
) -> String {
    let status = if success { "OK" } else { "FAIL" };
    let cmd = if command.is_empty() {
        "<none>".to_string()
    } else {
        command.join(" ")
    };
    format!(
        "[{status}] {} | {} ms | {cmd}",
        timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown-time".to_string()),
        duration_ms
    )
}

pub fn render_summary(ctx: RenderContext<'_>) {
    let digest = clamp_lines(ctx.digest, ctx.max_digest_lines);
    let excerpt = ctx
        .excerpt
        .map(|text| clamp_lines(text, ctx.max_excerpt_lines));

    match ctx.show_mode {
        ShowMode::Quiet => {}
        ShowMode::Full => {
            if ctx.show_log_path {
                println!("full log: {}", ctx.log_path.display());
            }
        }
        ShowMode::Digest => {
            println!("\n{digest}");
            if ctx.show_log_path {
                println!("full log: {}", ctx.log_path.display());
            }
        }
        ShowMode::Excerpt => {
            if !ctx.success {
                if let Some(excerpt) = excerpt {
                    println!("\n{excerpt}");
                } else {
                    println!("\n{digest}");
                }
            } else {
                println!("\n{digest}");
            }
            if ctx.show_log_path {
                println!("full log: {}", ctx.log_path.display());
            }
        }
        ShowMode::Auto => {
            println!("\n{digest}");
            if !ctx.success {
                if let Some(excerpt) = excerpt {
                    println!("\n{excerpt}");
                }
            }
            if ctx.show_log_path {
                println!("full log: {}", ctx.log_path.display());
            }
        }
    }
}

fn clamp_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }

    let lines: Vec<&str> = text.lines().take(max_lines).collect();
    let mut out = lines.join("\n");

    if text.lines().count() > max_lines {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("... (truncated)");
    }

    out
}
