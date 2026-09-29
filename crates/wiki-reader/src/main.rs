//! wiki-reader binary: TUI app over the core and render crates.
//!
//! See [architecture overview](../../wiki/architecture/overview.md).

mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "wiki-reader",
    about = "Terminal wiki reader for markdown collections"
)]
struct Args {
    /// Collection root to open (defaults to the current directory).
    #[arg(default_value = ".")]
    root: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match tui::app::run(&args.root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("wiki-reader: {err}");
            ExitCode::FAILURE
        }
    }
}
