use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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
