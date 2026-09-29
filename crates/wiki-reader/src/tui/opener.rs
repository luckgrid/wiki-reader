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

/// Test double: records opens without spawning.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct RecordingOpener {
    pub opened: std::sync::Mutex<Vec<String>>,
}

impl Opener for RecordingOpener {
    fn open(&self, url: &str) -> io::Result<()> {
        self.opened.lock().expect("lock").push(url.to_owned());
        Ok(())
    }
}
