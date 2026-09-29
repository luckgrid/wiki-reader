//! Filesystem watcher: notify-debouncer-mini → markdown/dir dirty pings.

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

/// Debounced watcher. Markdown pages and directory events under the root set dirty.
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
        Self::start_with_debounce(root, Duration::from_millis(250))
    }

    /// Like [`start`] with a custom debounce window (tests use a short value).
    ///
    /// # Errors
    ///
    /// Returns when the path cannot be canonicalized or notify cannot watch it.
    pub fn start_with_debounce(root: &Path, debounce: Duration) -> Result<Self, notify::Error> {
        let root = std::fs::canonicalize(root).map_err(notify::Error::io)?;
        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(debounce, tx)?;
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

    /// Non-blocking: relevant markdown / directory activity / error counts since last poll.
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

/// True when a VCS metadata directory name should be ignored (matches `FsProvider`).
#[must_use]
pub fn is_vcs_dir_name(name: &str) -> bool {
    name == ".git" || name == ".jj"
}

/// True when `path` should trigger a reindex (aligned with discovery, not cargo `target`).
///
/// Markdown pages under the root count, including dot-dirs like `.planning/`.
/// Directory paths (rename/delete) also count. Non-markdown files do not.
///
/// ponytail: gitignore is not consulted on watch events (ceiling: may dirty
/// ignored paths); upgrade with `ignore::gitignore` matching if noise bites.
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
        if is_vcs_dir_name(name) {
            return false;
        }
    }
    if is_markdown(path) {
        return true;
    }
    // Non-markdown files (.txt, etc.) never dirty — including when FSEvents also
    // reports a parent directory for the write.
    if path.extension().is_some() {
        return false;
    }
    // Extension-less path: directory rename/delete under the collection (not root).
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    !rel.as_os_str().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use tempfile::tempdir;

    const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

    fn start_test(root: &Path) -> Watcher {
        Watcher::start_with_debounce(root, TEST_DEBOUNCE).unwrap()
    }

    fn wait_dirty(watcher: &Watcher) -> bool {
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(50));
            if watcher.poll_dirty().dirty {
                return true;
            }
        }
        false
    }

    #[test]
    fn nonexistent_root_returns_err() {
        let err = Watcher::start(Path::new("/definitely/not-here-wiki-reader-watch"));
        assert!(err.is_err());
    }

    #[test]
    fn dirty_after_write() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();
        fs::write(root.join("b.md"), "# b\n").unwrap();
        assert!(wait_dirty(&watcher), "expected dirty after write");
    }

    #[test]
    fn git_and_txt_writes_do_not_set_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // No .md in the tree: late FSEvents for existing pages must not flake this.
        let git = root.join(".git");
        fs::create_dir(&git).unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE.saturating_mul(2));
        let _ = watcher.poll_dirty();

        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(root.join("notes.txt"), "nope\n").unwrap();
        for _ in 0..6 {
            thread::sleep(TEST_DEBOUNCE);
            assert!(
                !watcher.poll_dirty().dirty,
                ".git / .txt must not set dirty"
            );
        }
    }

    #[test]
    fn planning_dot_dir_edit_sets_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let planning = root.join(".planning");
        fs::create_dir(&planning).unwrap();
        fs::write(planning.join("p.md"), "# p\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();
        fs::write(planning.join("p.md"), "# p2\n").unwrap();
        assert!(wait_dirty(&watcher), ".planning/ edit should dirty");
    }

    #[test]
    fn two_md_edits_both_set_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        fs::write(root.join("b.md"), "# b\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();

        fs::write(root.join("a.md"), "# a2\n").unwrap();
        fs::write(root.join("b.md"), "# b2\n").unwrap();
        assert!(
            wait_dirty(&watcher),
            "two .md edits in one window should dirty"
        );
        let provider = crate::provider::FsProvider::open(root).unwrap();
        let index = crate::Index::build(&provider).unwrap();
        assert_eq!(index.pages.len(), 2);
    }

    #[test]
    fn atomic_save_temp_rename_sets_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.md"), "# a\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();

        let tmp = root.join("a.md.tmp");
        fs::write(&tmp, "# a rewritten\n").unwrap();
        fs::rename(&tmp, root.join("a.md")).unwrap();
        assert!(wait_dirty(&watcher), "atomic rename onto .md should dirty");
    }

    #[test]
    fn directory_rename_sets_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let docs = root.join("docs");
        fs::create_dir(&docs).unwrap();
        fs::write(docs.join("a.md"), "# a\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();
        fs::rename(&docs, root.join("docs2")).unwrap();
        assert!(wait_dirty(&watcher), "directory rename should dirty");
    }

    #[test]
    fn directory_delete_sets_dirty() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let docs = root.join("docs");
        fs::create_dir(&docs).unwrap();
        fs::write(docs.join("a.md"), "# a\n").unwrap();
        let watcher = start_test(root);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();
        fs::remove_dir_all(&docs).unwrap();
        assert!(wait_dirty(&watcher), "directory delete should dirty");
    }

    #[test]
    fn path_is_relevant_filters() {
        let root = Path::new("/wiki");
        assert!(path_is_relevant(root, Path::new("/wiki/foo.md")));
        assert!(path_is_relevant(root, Path::new("/wiki/.planning/x.md")));
        assert!(path_is_relevant(root, Path::new("/wiki/target/x.md")));
        assert!(!path_is_relevant(root, Path::new("/wiki/foo.txt")));
        assert!(!path_is_relevant(root, Path::new("/wiki/.git/HEAD")));
        assert!(!path_is_relevant(root, Path::new("/wiki/.jj/x.md")));
        // Directory path (no extension): relevant for rename/delete.
        assert!(path_is_relevant(root, Path::new("/wiki/docs")));
        // Collection root itself must not count (parent-dir noise on file writes).
        assert!(!path_is_relevant(root, Path::new("/wiki")));
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
        let watcher = start_test(&link);
        thread::sleep(TEST_DEBOUNCE);
        let _ = watcher.poll_dirty();
        fs::write(real.join("b.md"), "# b\n").unwrap();
        assert!(wait_dirty(&watcher), "symlinked root should see events");
    }
}
