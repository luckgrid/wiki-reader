//! Per-root session restore (`$XDG_STATE_HOME/wiki-reader/`).
//!
//! See [product spec](../../../wiki/product/spec.md) (M1).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::nav::{Location, NavStop, Navigator, NodeId, Tab, ViewMode};
use crate::provider::PageKey;

/// Serializable session snapshot (no live `NavTree`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionState {
    pub tabs: Vec<TabState>,
    pub active: usize,
    pub expanded: Vec<NodeIdState>,
    pub nav_cursor: NavStopState,
    pub focus: FocusPaneState,
    pub nav_visible: Option<bool>,
    /// User-resized nav column width (P2-14); absent in older session files.
    #[serde(default)]
    pub nav_width: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabState {
    pub history: Vec<LocationState>,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocationState {
    pub relative_path: PathBuf,
    pub anchor: Option<String>,
    pub cursor_line: u32,
    pub scroll: u32,
    pub mode: ViewModeState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewModeState {
    Rendered,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NavStopState {
    Search,
    Node { id: NodeIdState },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeIdState {
    Page { relative_path: PathBuf },
    Group { path: PathBuf },
    OtherPages,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FocusPaneState {
    Nav,
    #[default]
    Viewer,
}

/// Load result for the TUI.
#[derive(Debug, Clone)]
pub struct LoadedSession {
    pub state: SessionState,
    pub notice: Option<String>,
}

/// `$XDG_STATE_HOME/wiki-reader` or `~/.local/state/wiki-reader`.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME")
        && let Some(dir) = absolute_state_home(Some(xdg.as_str()))
    {
        return Some(dir);
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state/wiki-reader"))
}

fn absolute_state_home(xdg: Option<&str>) -> Option<PathBuf> {
    let xdg = xdg?.trim();
    // XDG Base Directory Spec: empty means unset; relative is ignored.
    if xdg.is_empty() {
        return None;
    }
    let p = PathBuf::from(xdg);
    p.is_absolute().then(|| p.join("wiki-reader"))
}

/// Stable filename for a canonical collection root.
#[must_use]
pub fn state_path_for_root(root: &Path) -> Option<PathBuf> {
    let dir = state_dir()?;
    let key = root_key(root);
    Some(dir.join(format!("{key}.toml")))
}

/// Stable FNV-1a 64-bit over the display path (not `DefaultHasher`).
fn root_key(root: &Path) -> String {
    fnv1a64(root.to_string_lossy().as_bytes())
}

fn fnv1a64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Load session for `root`; missing → `None`. Corrupt → empty state + notice.
#[must_use]
pub fn load(root: &Path) -> Option<LoadedSession> {
    load_from_path(&state_path_for_root(root)?)
}

/// Load from an explicit path (tests).
#[must_use]
pub fn load_from_path(path: &Path) -> Option<LoadedSession> {
    if !path.is_file() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    match toml::from_str::<SessionState>(&text) {
        Ok(state) => Some(LoadedSession {
            state,
            notice: None,
        }),
        Err(_) => Some(LoadedSession {
            state: SessionState {
                tabs: Vec::new(),
                active: 0,
                expanded: Vec::new(),
                nav_cursor: NavStopState::Search,
                focus: FocusPaneState::Viewer,
                nav_visible: None,
                nav_width: None,
            },
            notice: Some("session restore failed; starting clean".into()),
        }),
    }
}

/// Persist session atomically (write temp then rename).
///
/// # Errors
///
/// Returns IO errors from `create_dir` / `write` / `rename`.
pub fn save(root: &Path, state: &SessionState) -> std::io::Result<()> {
    let path = state_path_for_root(root)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no state dir"))?;
    save_to_path(&path, state)
}

/// Save to an explicit path (tests).
///
/// # Errors
///
/// IO errors from `create_dir` / `write` / `rename`.
pub fn save_to_path(path: &Path, state: &SessionState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(state).map_err(|e| std::io::Error::other(e.to_string()))?;
    let tmp = path.with_extension(format!(
        "tmp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    // A failed write or rename must not leave `<hash>.tmp-<nanos>` files behind.
    if let Err(err) = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

impl SessionState {
    /// Capture from a live navigator + focus.
    #[must_use]
    pub fn from_navigator(
        nav: &Navigator,
        focus: FocusPaneState,
        nav_visible: Option<bool>,
        nav_width: Option<u16>,
    ) -> Self {
        let tabs = nav
            .tabs()
            .iter()
            .map(|t| TabState {
                history: t
                    .history
                    .iter()
                    .map(|l| LocationState {
                        relative_path: l.page.relative_path.clone(),
                        anchor: l.anchor.clone(),
                        cursor_line: l.cursor_line,
                        scroll: l.scroll,
                        mode: match l.mode {
                            ViewMode::Rendered => ViewModeState::Rendered,
                            ViewMode::Raw => ViewModeState::Raw,
                        },
                    })
                    .collect(),
                cursor: t.cursor,
            })
            .collect();
        let expanded = nav
            .nav()
            .expanded
            .iter()
            .map(NodeIdState::from_node)
            .collect();
        let nav_cursor = match &nav.nav().cursor {
            NavStop::Search => NavStopState::Search,
            NavStop::Node(id) => NavStopState::Node {
                id: NodeIdState::from_node(id),
            },
        };
        Self {
            tabs,
            active: nav.active(),
            expanded,
            nav_cursor,
            focus,
            nav_visible,
            nav_width,
        }
    }

    /// Apply onto a fresh navigator (drops missing pages; clamps cursors).
    pub fn apply_to(&self, nav: &mut Navigator, collection_id: &str) -> Option<String> {
        let index = nav.index();
        let mut notice = None;
        let mut tabs: Vec<Tab> = Vec::new();
        for tab in &self.tabs {
            let mut history: Vec<Location> = Vec::new();
            for loc in &tab.history {
                let key = PageKey {
                    collection_id: collection_id.to_owned(),
                    relative_path: loc.relative_path.clone(),
                };
                if !index.pages.contains_key(&key) {
                    notice = Some("dropped missing pages from session".into());
                    continue;
                }
                history.push(Location {
                    page: key,
                    anchor: loc.anchor.clone(),
                    cursor_line: loc.cursor_line,
                    scroll: loc.scroll,
                    mode: match loc.mode {
                        ViewModeState::Rendered => ViewMode::Rendered,
                        ViewModeState::Raw => ViewMode::Raw,
                    },
                });
            }
            if history.is_empty() {
                continue;
            }
            let cursor = tab.cursor.min(history.len().saturating_sub(1));
            tabs.push(Tab { history, cursor });
        }
        if tabs.is_empty() {
            return notice.or(Some("session had no valid tabs".into()));
        }
        let active = self.active.min(tabs.len().saturating_sub(1));
        nav.replace_tabs(tabs, active);

        let mut expanded = HashSet::new();
        for id in &self.expanded {
            expanded.insert(id.to_node(collection_id));
        }
        nav.nav_mut().expanded = expanded;

        nav.nav_mut().cursor = match &self.nav_cursor {
            NavStopState::Search => NavStop::Search,
            NavStopState::Node { id } => NavStop::Node(id.to_node(collection_id)),
        };
        notice
    }
}

impl NodeIdState {
    fn from_node(id: &NodeId) -> Self {
        match id {
            NodeId::Page(k) => Self::Page {
                relative_path: k.relative_path.clone(),
            },
            NodeId::Group(p) => Self::Group { path: p.clone() },
            NodeId::OtherPages => Self::OtherPages,
        }
    }

    fn to_node(&self, collection_id: &str) -> NodeId {
        match self {
            Self::Page { relative_path } => NodeId::Page(PageKey {
                collection_id: collection_id.to_owned(),
                relative_path: relative_path.clone(),
            }),
            Self::Group { path } => NodeId::Group(path.clone()),
            Self::OtherPages => NodeId::OtherPages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Index;
    use crate::nav::{Disposition, Target, ViewState};
    use crate::provider::FsProvider;
    use tempfile::tempdir;

    #[test]
    fn a_failed_save_leaves_no_tmp_files_behind() {
        let wiki = tempdir().unwrap();
        fs::write(wiki.path().join("README.md"), "# Hi\n").unwrap();
        let provider = FsProvider::open(wiki.path()).unwrap();
        let nav = Navigator::new(Index::build(&provider).unwrap(), None).unwrap();
        let state = SessionState::from_navigator(&nav, FocusPaneState::Viewer, None, None);
        let dir = tempdir().unwrap();
        let target = dir.path().join("session.toml");
        // Renaming a file onto a directory fails after the temp file was written.
        fs::create_dir(&target).unwrap();
        assert!(save_to_path(&target, &state).is_err());
        let leftovers = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("tmp-"))
            .count();
        assert_eq!(leftovers, 0, "tmp file removed after a failed save");
    }

    #[test]
    fn session_round_trip_temp_state_dir() {
        let wiki = tempdir().unwrap();
        fs::write(wiki.path().join("README.md"), "# Hi\n\n## Sec\n").unwrap();
        fs::write(wiki.path().join("other.md"), "# Other\n").unwrap();
        let provider = FsProvider::open(wiki.path()).unwrap();
        let index = Index::build(&provider).unwrap();
        let mut nav = Navigator::new(index, None).unwrap();
        let other = PageKey {
            collection_id: nav.index().collection_id.clone(),
            relative_path: PathBuf::from("other.md"),
        };
        let _ = nav.navigate(
            Target::Page(other, None),
            Disposition::Replace,
            ViewState::default(),
        );

        let state_root = tempdir().unwrap();
        let path = state_root.path().join("sess.toml");
        let snap = SessionState::from_navigator(&nav, FocusPaneState::Viewer, Some(true), Some(42));
        save_to_path(&path, &snap).unwrap();
        let loaded = load_from_path(&path).unwrap();
        assert!(loaded.notice.is_none());
        assert_eq!(loaded.state.nav_width, Some(42));
        assert_eq!(loaded.state.tabs.len(), 1);
        assert!(
            loaded.state.tabs[0]
                .history
                .iter()
                .any(|l| l.relative_path.ends_with("other.md"))
        );
    }

    #[test]
    fn removed_formatted_view_field_is_ignored() {
        let wiki = tempdir().unwrap();
        fs::write(wiki.path().join("README.md"), "# Hi\n").unwrap();
        let provider = FsProvider::open(wiki.path()).unwrap();
        let index = Index::build(&provider).unwrap();
        let nav = Navigator::new(index, None).unwrap();
        let state =
            SessionState::from_navigator(&nav, FocusPaneState::Viewer, Some(true), Some(42));

        let path = wiki.path().join("session.toml");
        save_to_path(&path, &state).unwrap();
        let old = fs::read_to_string(&path).unwrap().replacen(
            "nav_width = 42\n",
            "nav_width = 42\nformatted_view = true\n",
            1,
        );
        fs::write(&path, old).unwrap();

        let loaded = load_from_path(&path).unwrap();
        assert!(loaded.notice.is_none());
        assert_eq!(loaded.state, state);
    }

    #[test]
    fn removed_page_dropped_from_history() {
        let wiki = tempdir().unwrap();
        fs::write(wiki.path().join("README.md"), "# Hi\n").unwrap();
        fs::write(wiki.path().join("gone.md"), "# Gone\n").unwrap();
        let provider = FsProvider::open(wiki.path()).unwrap();
        let index = Index::build(&provider).unwrap();
        let nav = Navigator::new(index, None).unwrap();

        let mut snap = SessionState::from_navigator(&nav, FocusPaneState::Nav, None, None);
        snap.tabs[0].history.push(LocationState {
            relative_path: PathBuf::from("gone.md"),
            anchor: None,
            cursor_line: 0,
            scroll: 0,
            mode: ViewModeState::Rendered,
        });
        fs::remove_file(wiki.path().join("gone.md")).unwrap();
        let index = Index::build(&FsProvider::open(wiki.path()).unwrap()).unwrap();
        let mut nav2 = Navigator::new(index, None).unwrap();
        let cid = nav2.index().collection_id.clone();
        let notice = snap.apply_to(&mut nav2, &cid);
        assert!(notice.is_some());
        assert!(
            nav2.tabs()
                .iter()
                .flat_map(|t| &t.history)
                .all(|l| l.page.relative_path != *"gone.md")
        );
    }

    #[test]
    fn corrupt_file_yields_clean_start_message() {
        let state = tempdir().unwrap();
        let path = state.path().join("bad.toml");
        fs::write(&path, "not = [toml").unwrap();
        let loaded = load_from_path(&path).unwrap();
        assert!(loaded.notice.unwrap().contains("clean"));
        assert!(loaded.state.tabs.is_empty());
    }

    #[test]
    fn root_key_is_deterministic() {
        let a = root_key(Path::new("/tmp/wiki-a"));
        let b = root_key(Path::new("/tmp/wiki-a"));
        let c = root_key(Path::new("/tmp/wiki-b"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn empty_or_relative_xdg_state_is_rejected_by_helper() {
        assert!(absolute_state_home(Some("")).is_none());
        assert!(absolute_state_home(Some("  ")).is_none());
        assert!(absolute_state_home(Some("relative/path")).is_none());
        assert_eq!(
            absolute_state_home(Some("/abs/state")),
            Some(PathBuf::from("/abs/state/wiki-reader"))
        );
    }
}
