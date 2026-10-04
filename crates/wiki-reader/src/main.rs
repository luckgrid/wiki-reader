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
    about = "Terminal wiki reader for markdown collections",
    version
)]
struct Args {
    /// Collection root to open (defaults to the current directory).
    root: Option<PathBuf>,
    /// Use Herdr's focused pane or workspace cwd when no root is supplied.
    #[arg(long)]
    herdr_context: bool,
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

fn resolve_root(args: &Args, context: Option<&str>) -> PathBuf {
    if let Some(root) = &args.root {
        return root.clone();
    }
    if args.herdr_context
        && let Some(root) = context.and_then(wiki_reader_core::herdr::parse_context_cwd)
        && root.is_dir()
    {
        return root;
    }
    PathBuf::from(".")
}

fn main() -> ExitCode {
    let args = Args::parse();
    let context = std::env::var("HERDR_PLUGIN_CONTEXT_JSON").ok();
    let root = resolve_root(&args, context.as_deref());
    if let Err(msg) = check_root(&root) {
        eprintln!("wiki-reader: {msg}");
        return ExitCode::FAILURE;
    }
    match tui::app::run(&root, args.config.as_deref()) {
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

    #[test]
    fn root_defaults_to_current_directory_without_flag() {
        let args = Args::try_parse_from(["wiki-reader"]).unwrap();
        assert_eq!(
            resolve_root(&args, Some(r#"{"workspace_cwd":"/"}"#)),
            PathBuf::from(".")
        );
    }

    #[test]
    fn explicit_root_wins_even_when_invalid() {
        let args = Args::try_parse_from([
            "wiki-reader",
            "--herdr-context",
            "/nope-wiki-reader-missing",
        ])
        .unwrap();
        let root = resolve_root(&args, Some(r#"{"workspace_cwd":"/"}"#));
        assert_eq!(root, PathBuf::from("/nope-wiki-reader-missing"));
        assert!(check_root(&root).is_err());
    }

    #[test]
    fn context_root_uses_existing_focused_or_workspace_directory() {
        let args = Args::try_parse_from(["wiki-reader", "--herdr-context"]).unwrap();
        for field in ["focused_pane_cwd", "workspace_cwd"] {
            let context = format!(r#"{{"{field}":"/"}}"#);
            assert_eq!(resolve_root(&args, Some(&context)), PathBuf::from("/"));
        }
    }

    #[test]
    fn unusable_context_silently_defaults_to_current_directory() {
        let args = Args::try_parse_from(["wiki-reader", "--herdr-context"]).unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        let file_context = format!(r#"{{"focused_pane_cwd":"{}"}}"#, file.path().display());
        for context in [
            None,
            Some("not json"),
            Some("{}"),
            Some(r#"{"focused_pane_cwd":"/nope-wiki-reader-missing","workspace_cwd":"/"}"#),
            Some(file_context.as_str()),
        ] {
            assert_eq!(resolve_root(&args, context), PathBuf::from("."));
        }
    }

    #[test]
    fn version_flag_reports_pkg_version() {
        use clap::CommandFactory;
        let ver = Args::command().render_version();
        assert!(
            ver.contains(env!("CARGO_PKG_VERSION")),
            "version output: {ver}"
        );
    }
}
