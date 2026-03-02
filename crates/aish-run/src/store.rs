use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RunPaths {
    pub run_dir: PathBuf,
    pub meta_path: PathBuf,
    pub log_path: PathBuf,
    pub digest_path: PathBuf,
    pub relevant_path: PathBuf,
    pub last_link: PathBuf,
}

#[derive(Debug, Clone)]
struct RunDiskUsage {
    path: PathBuf,
    size_bytes: u64,
    modified: SystemTime,
}

pub fn prepare_run_dir(root: &Path, now: OffsetDateTime) -> io::Result<RunPaths> {
    let date = now
        .format(&time::macros::format_description!("[year]-[month]-[day]"))
        .unwrap_or_else(|_| "unknown-date".to_string());
    let stamp = now
        .format(&time::macros::format_description!(
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .unwrap_or_else(|_| "unknown-time".to_string());

    let run_id = format!("{}_{}", stamp, Uuid::new_v4());
    let runs_dir = root.join("runs").join(date);
    let run_dir = runs_dir.join(run_id);

    fs::create_dir_all(&run_dir)?;

    let meta_path = run_dir.join("meta.json");
    let log_path = run_dir.join("pty.log");
    let digest_path = run_dir.join("digest.txt");
    let relevant_path = run_dir.join("relevant.txt");
    let last_link = root.join("last");

    Ok(RunPaths {
        run_dir,
        meta_path,
        log_path,
        digest_path,
        relevant_path,
        last_link,
    })
}

pub fn update_last_symlink(last_link: &Path, run_dir: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(last_link) {
        if meta.is_dir() {
            fs::remove_dir_all(last_link)?;
        } else {
            fs::remove_file(last_link)?;
        }
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(run_dir, last_link)?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(run_dir, last_link)?;
    }

    Ok(())
}

pub fn enforce_retention(
    root: &Path,
    keep_days: u32,
    max_total_mb: u64,
    preserve_run_dir: &Path,
) -> io::Result<()> {
    let runs_root = root.join("runs");
    if !runs_root.exists() {
        return Ok(());
    }

    let mut runs = collect_run_dirs(&runs_root)?;

    let now = SystemTime::now();
    let keep_duration = Duration::from_secs(keep_days as u64 * 24 * 60 * 60);

    let mut retained = Vec::new();
    for run in runs.drain(..) {
        if run.path == preserve_run_dir {
            retained.push(run);
            continue;
        }

        let is_old = now
            .duration_since(run.modified)
            .map(|age| age > keep_duration)
            .unwrap_or(false);

        if is_old {
            let _ = fs::remove_dir_all(&run.path);
        } else {
            retained.push(run);
        }
    }

    let max_total_bytes = max_total_mb.saturating_mul(1024 * 1024);
    retained.sort_by_key(|run| run.modified);

    let mut total_bytes: u64 = retained.iter().map(|r| r.size_bytes).sum();
    for run in retained {
        if total_bytes <= max_total_bytes {
            break;
        }
        if run.path == preserve_run_dir {
            continue;
        }

        if fs::remove_dir_all(&run.path).is_ok() {
            total_bytes = total_bytes.saturating_sub(run.size_bytes);
        }
    }

    Ok(())
}

fn collect_run_dirs(runs_root: &Path) -> io::Result<Vec<RunDiskUsage>> {
    let mut out = Vec::new();

    for day_entry in fs::read_dir(runs_root)? {
        let day_entry = day_entry?;
        if !day_entry.file_type()?.is_dir() {
            continue;
        }

        for run_entry in fs::read_dir(day_entry.path())? {
            let run_entry = run_entry?;
            if !run_entry.file_type()?.is_dir() {
                continue;
            }

            let run_path = run_entry.path();
            let size_bytes = directory_size(&run_path)?;
            let modified = fs::metadata(&run_path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push(RunDiskUsage {
                path: run_path,
                size_bytes,
                modified,
            });
        }
    }

    Ok(out)
}

fn directory_size(path: &Path) -> io::Result<u64> {
    let mut total = 0_u64;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total = total.saturating_add(directory_size(&entry.path())?);
        } else if meta.is_file() {
            total = total.saturating_add(meta.len());
        }
    }

    Ok(total)
}
