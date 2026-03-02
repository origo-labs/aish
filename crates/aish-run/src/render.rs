use time::OffsetDateTime;

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
