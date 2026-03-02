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

    if args.print_shims {
        match config::AppConfig::load() {
            Ok(cfg) => print_shims(&cfg),
            Err(err) => {
                eprintln!("aish-run error: {err}");
                std::process::exit(1);
            }
        }
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

fn print_shims(cfg: &config::AppConfig) {
    if cfg.wrap.default_mode != "on" {
        return;
    }

    for cmd in &cfg.wrap.commands {
        if cfg.wrap.skip_commands.iter().any(|skip| skip == cmd) {
            continue;
        }
        if !is_valid_shell_identifier(cmd) {
            continue;
        }

        println!("{cmd}() {{ command aish-run -- {cmd} \"$@\"; }}");
    }
}

fn is_valid_shell_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
