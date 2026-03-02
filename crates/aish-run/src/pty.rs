use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub exit_code: i32,
    pub success: bool,
    pub status_text: String,
}

pub fn run_in_pty(command: &[String], cwd: &Path, log_path: &Path) -> io::Result<CommandOutcome> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(to_io_error)?;

    let mut builder = CommandBuilder::new(&command[0]);
    builder.args(command.iter().skip(1));
    builder.cwd(cwd);

    let mut child = pair.slave.spawn_command(builder).map_err(to_io_error)?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(to_io_error)?;
    let mut log_file = File::create(log_path)?;
    let mut stdout = io::stdout().lock();

    let mut buf = [0_u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                log_file.write_all(&buf[..n])?;
                stdout.write_all(&buf[..n])?;
                stdout.flush()?;
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }

    let status = child.wait()?;
    let exit_code = status.exit_code() as i32;

    Ok(CommandOutcome {
        exit_code,
        success: status.success(),
        status_text: status.to_string(),
    })
}

pub fn run_without_pty(
    command: &[String],
    cwd: &Path,
    log_path: &Path,
) -> io::Result<CommandOutcome> {
    let output = std::process::Command::new(&command[0])
        .args(command.iter().skip(1))
        .current_dir(cwd)
        .output()?;

    let mut log_file = File::create(log_path)?;
    log_file.write_all(&output.stdout)?;
    log_file.write_all(&output.stderr)?;

    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;

    let exit_code = output.status.code().unwrap_or(1);
    let success = output.status.success();
    let status_text = format!("{}", output.status);

    Ok(CommandOutcome {
        exit_code,
        success,
        status_text,
    })
}

fn to_io_error(err: anyhow::Error) -> io::Error {
    io::Error::other(err.to_string())
}
