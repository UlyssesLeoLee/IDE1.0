//! `ide-cli` binary entry point.
//!
//! Thin wrapper over [`ide_cli::run`].

fn main() -> std::process::ExitCode {
    ide_cli::run()
}
