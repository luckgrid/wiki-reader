//! Filesystem watcher: notify-debouncer-mini → markdown-only dirty pings.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use crate::provider::is_markdown;

/// Result of draining the debounce channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PollDirty {
    /// True when at least one relevant markdown path changed.
    pub dirty: bool,
    /// Debounce/notify error events dropped this poll.
    pub errors: u32,
}

/// Debounced watcher. Only markdown paths under the collection root set dirty.
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

    /// Non-blocking: relevant markdown activity / error counts since last poll.
    #[must_use]
    pub fn poll_dirty(&self) -> PollDirty {
        let mut out = PollDirty::default();
        while let Ok(res) = self.rx.try_recv() {
            match res {
                Ok(events) => {
                    for ev in events {
                        if path_is_relevant(&self.root, &ev.path) {
                            out.dirty = true;
                        }
                    }
                }
                Err(_) => {
                    out.errors = out.errors.saturating_add(1);
                }
            }
        }
        out
    }
}

/// True when `path` is a markdown file under `root`, not inside ignored dirs.
#[must_use]
pub fn path_is_relevant(root: &Path, path: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    for c in rel.components() {
        let std::path::Component::Normal(name) = c else {
            continue;
        };
        let Some(name) = name.to_str() else {
            continue;
        };
        if name == ".git" || name == ".jj" || name == "target" || name.starts_with('.') {
            return false;
        }
    }
    is_markdown(path)
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
            if watcher.poll_dirty().dirty {
                saw = true;
                break;
            }
        }
        assert!(saw, "expected dirty after write");
    }

    #[test]
    fn git_and_txt_writes_do_not_set_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        let git = root.join(".git");
        fs::create_dir(&git).unwrap();
        let watcher = Watcher::start(root).unwrap();
        thread::sleep(Duration::from_millis(300));
        let _ = watcher.poll_dirty();

        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(root.join("notes.txt"), "nope\n").unwrap();
        thread::sleep(Duration::from_millis(400));
        assert!(
            !watcher.poll_dirty().dirty,
            ".git / .txt must not set dirty"
        );
    }

    #[test]
    fn two_md_edits_both_set_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        fs::write(root.join("b.md"), "# b\n").unwrap();
        let watcher = Watcher::start(root).unwrap();
        thread::sleep(Duration::from_millis(300));
        let _ = watcher.poll_dirty();

        fs::write(root.join("a.md"), "# a2\n").unwrap();
        fs::write(root.join("b.md"), "# b2\n").unwrap();
        let mut saw = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            if watcher.poll_dirty().dirty {
                saw = true;
                break;
            }
        }
        assert!(saw, "two .md edits in one window should dirty");
        // Rebuild sees both (Index::build lists both).
        let provider = crate::provider::FsProvider::open(root).unwrap();
        let index = crate::Index::build(&provider).unwrap();
        assert_eq!(index.pages.len(), 2);
    }

    #[test]
    fn atomic_save_temp_rename_sets_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        let watcher = Watcher::start(root).unwrap();
        thread::sleep(Duration::from_millis(300));
        let _ = watcher.poll_dirty();

        let tmp = root.join("a.md.tmp");
        fs::write(&tmp, "# a rewritten\n").unwrap();
        fs::rename(&tmp, root.join("a.md")).unwrap();
        let mut saw = false;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            if watcher.poll_dirty().dirty {
                saw = true;
                break;
            }
        }
        assert!(saw, "atomic rename onto .md should dirty");
    }

    #[test]
    fn path_is_relevant_filters() {
        let root = Path::new("/wiki");
        assert!(path_is_relevant(root, Path::new("/wiki/foo.md")));
        assert!(!path_is_relevant(root, Path::new("/wiki/foo.txt")));
        assert!(!path_is_relevant(root, Path::new("/wiki/.git/HEAD")));
        assert!(!path_is_relevant(root, Path::new("/wiki/target/x.md")));
        assert!(!path_is_relevant(root, Path::new("/wiki/.hidden/x.md")));
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
            if watcher.poll_dirty().dirty {
                saw = true;
                break;
            }
        }
        assert!(saw, "symlinked root should see events");
    }
}
