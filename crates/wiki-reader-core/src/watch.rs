//! Filesystem watcher: notify-debouncer-mini → rebuild pings.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use crate::provider::is_markdown;

/// Index-affecting change from the collection root (relative paths).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexEvent {
    /// New markdown file.
    Added(PathBuf),
    /// Changed markdown file.
    Changed(PathBuf),
    /// Removed markdown file.
    Removed(PathBuf),
}

/// Debounced watcher. Any coalesced FS activity sets a dirty flag; callers
/// should re-list pages and diff the index (most robust for renames/symlinks).
pub struct Watcher {
    rx: Receiver<notify_debouncer_mini::DebounceEventResult>,
    root: PathBuf,
    /// Keep the debouncer (and its thread) alive.
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl Watcher {
    /// Start watching `root` (canonicalized). Errors if the root cannot be watched.
    ///
    /// # Errors
    ///
    /// Returns when the path cannot be canonicalized or notify cannot watch it.
    pub fn start(root: &Path) -> Result<Self, notify::Error> {
        let root = std::fs::canonicalize(root).map_err(notify::Error::io)?;
        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(Duration::from_millis(250), tx)?;
        debouncer.watcher().watch(&root, RecursiveMode::Recursive)?;
        Ok(Self {
            rx,
            root,
            _debouncer: debouncer,
        })
    }

    /// Collection root being watched (canonical).
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Non-blocking: true if any debounced activity arrived since last poll.
    #[must_use]
    pub fn poll_dirty(&self) -> bool {
        let mut dirty = false;
        while let Ok(res) = self.rx.try_recv() {
            if res.is_ok() {
                dirty = true;
            }
        }
        dirty
    }

    /// Diff two relative-path sets into added/removed (changed detected by caller via mtime/rebuild).
    #[must_use]
    pub fn diff_paths(old: &[PathBuf], new: &[PathBuf]) -> Vec<IndexEvent> {
        let mut events = Vec::new();
        for p in new {
            if !old.contains(p) && is_markdown(p) {
                events.push(IndexEvent::Added(p.clone()));
            }
        }
        for p in old {
            if !new.contains(p) && is_markdown(p) {
                events.push(IndexEvent::Removed(p.clone()));
            } else if new.contains(p) && is_markdown(p) {
                events.push(IndexEvent::Changed(p.clone()));
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn nonexistent_root_returns_err() {
        let err = Watcher::start(Path::new("/definitely/not/here-wiki-reader-watch"));
        assert!(err.is_err());
    }

    #[test]
    fn dirty_after_write() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        let watcher = Watcher::start(root).unwrap();
        // clear startup noise
        thread::sleep(Duration::from_millis(300));
        let _ = watcher.poll_dirty();
        fs::write(root.join("b.md"), "# b\n").unwrap();
        let mut saw = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            if watcher.poll_dirty() {
                saw = true;
                break;
            }
        }
        assert!(saw, "expected dirty after write");
    }

    #[test]
    fn symlink_root_receives_events() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("a.md"), "# a\n").unwrap();
        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(not(unix))]
        {
            let _ = (real, link);
            return;
        }
        let watcher = Watcher::start(&link).unwrap();
        thread::sleep(Duration::from_millis(300));
        let _ = watcher.poll_dirty();
        fs::write(real.join("b.md"), "# b\n").unwrap();
        let mut saw = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            if watcher.poll_dirty() {
                saw = true;
                break;
            }
        }
        assert!(saw, "symlinked root should see events");
    }
}
