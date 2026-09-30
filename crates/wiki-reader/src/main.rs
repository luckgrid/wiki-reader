//! wiki-reader binary: TUI app over the core and render crates.
//!
//! See [architecture overview](../../wiki/architecture/overview.md).

mod tui;

use std::path::{Path, PathBuf};
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
    /// Optional config TOML (overrides XDG and `<root>/.wiki-reader.toml`).
    #[arg(long = "config", value_name = "PATH")]
    config: Option<PathBuf>,
}

fn check_root(root: &Path) -> Result<(), String> {
    match std::fs::metadata(root) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) | Err(_) => Err(format!("{}: not a directory", root.display())),
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    if let Err(msg) = check_root(&args.root) {
        eprintln!("wiki-reader: {msg}");
        return ExitCode::FAILURE;
    }
    match tui::app::run(&args.root, args.config.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("wiki-reader: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn check_root_accepts_existing_dir() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(check_root(root).is_ok());
    }

    #[test]
    fn check_root_rejects_missing_path() {
        let err = check_root(Path::new("/nope-wiki-reader-missing")).unwrap_err();
        assert!(err.contains("not a directory"), "got: {err}");
    }
}
