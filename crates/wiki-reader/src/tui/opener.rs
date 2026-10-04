//! External URL opener (injected for tests).

use std::io;
use std::process::{Command, Stdio};

/// Open a URL in the system browser.
pub trait Opener: Send {
    /// Launch the URL; errors are non-fatal in the UI.
    fn open(&self, url: &str) -> io::Result<()>;
}

/// `open` / `xdg-open` on the host.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, url: &str) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(target_os = "linux")]
        let mut cmd = Command::new("xdg-open");
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let mut cmd = Command::new("xdg-open");
        // ponytail: fire-and-forget so the TUI is not blocked / painted over
        cmd.arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }
}

/// Configurable opener: first word is the program, remaining words are args, then the URL.
#[derive(Debug, Clone)]
pub struct CommandOpener {
    pub command: String,
}

impl Opener for CommandOpener {
    fn open(&self, url: &str) -> io::Result<()> {
        let words = shell_words::split(self.command.trim())
            .unwrap_or_else(|_| self.command.split_whitespace().map(str::to_owned).collect());
        let (program, prefix) = match words.split_first() {
            Some((p, rest)) => (p.clone(), rest.to_vec()),
            None => (self.command.trim().to_owned(), Vec::new()),
        };
        let mut cmd = Command::new(&program);
        cmd.args(&prefix)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }
}
