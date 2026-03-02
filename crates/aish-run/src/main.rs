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

    println!("Phase 0 scaffold ready. Command: {:?}", args.command);
}
