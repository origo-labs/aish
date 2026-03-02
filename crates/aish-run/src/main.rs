mod cli;
mod config;
mod detectors;
mod policy;
mod pty;
mod render;
mod runner;
mod signals;
mod store;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();

    if args.version_info {
        println!("aish-run {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.command.is_empty() {
        eprintln!("No command provided. Use `aish-run -- <command> [args...]`.");
        std::process::exit(2);
    }

    match runner::run(&args) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("aish-run error: {err}");
            std::process::exit(1);
        }
    }
}
