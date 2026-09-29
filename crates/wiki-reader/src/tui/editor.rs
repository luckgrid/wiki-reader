//! Open the current page in `$VISUAL` / `$EDITOR` (E1).

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resolved editor invocation (program + argv).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCmd {
    /// Executable (from `$VISUAL` / `$EDITOR`, possibly a path).
    pub program: String,
    /// Arguments including the file (and line syntax).
    pub args: Vec<String>,
}

/// Spawn an editor and wait (injected for tests).
pub trait EditorLauncher: Send {
    /// Run `cmd` with inherited stdio; return when the editor exits.
    ///
    /// # Errors
    ///
    /// Propagates spawn / wait failures.
    fn launch(&self, cmd: &EditorCmd) -> io::Result<()>;
}

/// Real editor: inherit stdio and wait.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemEditor;

impl EditorLauncher for SystemEditor {
    fn launch(&self, cmd: &EditorCmd) -> io::Result<()> {
        let status = Command::new(&cmd.program)
            .args(&cmd.args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!("editor exited with {status}")))
        }
    }
}

/// Test double: records launches, never spawns.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct RecordingEditor {
    /// Captured commands (shared so tests can keep a handle after boxing).
    pub launched: std::sync::Arc<std::sync::Mutex<Vec<EditorCmd>>>,
}

impl EditorLauncher for RecordingEditor {
    fn launch(&self, cmd: &EditorCmd) -> io::Result<()> {
        self.launched.lock().expect("lock").push(cmd.clone());
        Ok(())
    }
}

/// `$VISUAL`, then `$EDITOR`; `None` if both unset/empty.
#[must_use]
pub fn resolve_editor() -> Option<String> {
    std::env::var("VISUAL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
}

/// Build argv for `editor` opening `path` at 1-based `line`.
#[must_use]
pub fn build_editor_command(editor: &str, path: &Path, line: u32) -> EditorCmd {
    let program = editor.trim().to_owned();
    let base = Path::new(&program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program.as_str());
    let path_s = path.to_string_lossy().into_owned();
    let line = line.max(1);
    let args = match base {
        "code" | "code-insiders" => vec!["-g".to_owned(), format!("{path_s}:{line}")],
        "hx" | "helix" | "zed" | "zeditor" => vec![format!("{path_s}:{line}")],
        // vi / vim / nvim / nano / emacs / default
        _ => vec![format!("+{line}"), path_s],
    };
    EditorCmd { program, args }
}

/// Absolute path for a page under the collection root.
#[must_use]
pub fn page_abs_path(root: &Path, relative: &Path) -> PathBuf {
    root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn command_builder_per_editor() {
        let path = PathBuf::from("/wiki/my page.md");
        let vim = build_editor_command("vim", &path, 12);
        assert_eq!(vim.program, "vim");
        assert_eq!(
            vim.args,
            vec!["+12".to_owned(), "/wiki/my page.md".to_owned()]
        );

        let nvim = build_editor_command("/usr/bin/nvim", &path, 3);
        assert_eq!(
            nvim.args,
            vec!["+3".to_owned(), "/wiki/my page.md".to_owned()]
        );

        let nano = build_editor_command("nano", &path, 1);
        assert_eq!(
            nano.args,
            vec!["+1".to_owned(), "/wiki/my page.md".to_owned()]
        );

        let emacs = build_editor_command("emacs", &path, 7);
        assert_eq!(
            emacs.args,
            vec!["+7".to_owned(), "/wiki/my page.md".to_owned()]
        );

        let hx = build_editor_command("hx", &path, 9);
        assert_eq!(hx.args, vec!["/wiki/my page.md:9".to_owned()]);

        let code = build_editor_command("code", &path, 4);
        assert_eq!(
            code.args,
            vec!["-g".to_owned(), "/wiki/my page.md:4".to_owned()]
        );

        let zed = build_editor_command("zed", &path, 5);
        assert_eq!(zed.args, vec!["/wiki/my page.md:5".to_owned()]);
    }

    #[test]
    fn line_zero_becomes_one() {
        let cmd = build_editor_command("vi", Path::new("a.md"), 0);
        assert_eq!(cmd.args[0], "+1");
    }
}
