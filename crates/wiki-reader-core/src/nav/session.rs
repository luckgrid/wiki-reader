//! Navigation session: tabs, history, `navigate` / `back` / `forward`.
//!
//! See [architecture overview](../../../../../wiki/architecture/overview.md) and
//! [ADR-0005](../../../../../wiki/decisions/0005-navigation-model.md).

use std::collections::HashSet;

use crate::index::Index;
use crate::provider::PageKey;

use super::resolve::{Target, resolve};
use super::tree::{NavTree, NodeId};

/// How a navigation should affect tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Replace the current view and push history (default).
    Replace,
    /// Open in a new tab.
    NewTab,
    /// Open in a new background tab (keep focus).
    BackgroundTab,
}

/// Viewer display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Rendered markdown.
    #[default]
    Rendered,
    /// Raw source.
    Raw,
}

/// A point in history within a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Page shown.
    pub page: PageKey,
    /// Optional heading slug.
    pub anchor: Option<String>,
    /// Viewer cursor line (restored on back).
    pub cursor_line: u32,
    /// Viewer scroll offset (restored on back).
    pub scroll: u32,
    /// Rendered vs raw.
    pub mode: ViewMode,
}

/// One browser-style tab with a history stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    /// History entries.
    pub history: Vec<Location>,
    /// Index into `history` (current location).
    pub cursor: usize,
}

impl Tab {
    /// Current location.
    #[must_use]
    pub fn current(&self) -> &Location {
        &self.history[self.cursor]
    }

    fn current_mut(&mut self) -> &mut Location {
        &mut self.history[self.cursor]
    }
}

/// Side-nav UI state derived from the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavState {
    /// Site nav tree.
    pub tree: NavTree,
    /// Expanded group ids.
    pub expanded: HashSet<NodeId>,
    /// Remembered nav cursor; `None` → current page's item.
    pub cursor: Option<NodeId>,
    /// Page current when nav last had focus (stale-cursor rule).
    pub seen_page: Option<PageKey>,
}

/// Side effects emitted by navigation (consumed by the TUI later).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Load page content into the viewer.
    LoadPage(PageKey),
    /// Expand ancestors and select the page in the tree.
    RevealInTree(PageKey),
    /// Scroll/jump to an anchor (or top when `None`).
    ScrollTo(Option<String>),
    /// Footer notice (e.g. missing anchor, unresolved).
    Notice(String),
    /// Confirm before opening an external URL.
    ConfirmExternal(String),
}

/// Pure navigation state machine (no terminal deps).
#[derive(Debug, Clone)]
pub struct Navigator {
    /// Collection index.
    pub index: Index,
    /// Open tabs.
    pub tabs: Vec<Tab>,
    /// Active tab index.
    pub active: usize,
    /// Side nav state.
    pub nav: NavState,
    /// Footer notice from the last navigation, if any.
    pub notice: Option<String>,
}

impl Navigator {
    /// Start on `start` (or the root README / first page).
    ///
    /// # Panics
    ///
    /// Panics when the index has no pages.
    #[must_use]
    pub fn new(index: Index, start: Option<PageKey>) -> Self {
        let tree = NavTree::build(&index);
        let start = start
            .or_else(|| tree.page_order().into_iter().next())
            .expect("collection has at least one page");
        let loc = Location {
            page: start.clone(),
            anchor: None,
            cursor_line: 0,
            scroll: 0,
            mode: ViewMode::Rendered,
        };
        let mut expanded = HashSet::new();
        expand_ancestors(&tree, &start, &mut expanded);
        Self {
            index,
            tabs: vec![Tab {
                history: vec![loc],
                cursor: 0,
            }],
            active: 0,
            nav: NavState {
                tree,
                expanded,
                cursor: Some(NodeId::Page(start.clone())),
                seen_page: Some(start),
            },
            notice: None,
        }
    }

    /// Active tab.
    #[must_use]
    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Number of tabs (N2: normal browse stays at 1).
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Navigate to a resolved or unresolved [`Target`].
    pub fn navigate(&mut self, target: Target, how: Disposition) -> Vec<Effect> {
        match target {
            Target::External(url) => {
                vec![Effect::ConfirmExternal(url)]
            }
            Target::Unresolved(raw) => {
                self.notice = Some(format!("broken link: {raw}"));
                vec![Effect::Notice(format!("broken link: {raw}"))]
            }
            Target::Anchor(slug) => {
                let page = self.tab().current().page.clone();
                let outcome = resolve(&format!("#{slug}"), &page, &self.index);
                self.push_location(
                    Location {
                        page: page.clone(),
                        anchor: Some(slug.clone()),
                        cursor_line: 0,
                        scroll: 0,
                        mode: self.tab().current().mode,
                    },
                    how,
                    outcome.notice,
                )
            }
            Target::Page(key, anchor) => {
                let mut notice = None;
                if let Some(ref slug) = anchor {
                    let outcome = resolve(&format!("#{slug}"), &key, &self.index);
                    notice = outcome.notice;
                }
                self.push_location(
                    Location {
                        page: key,
                        anchor,
                        cursor_line: 0,
                        scroll: 0,
                        mode: self.tab().current().mode,
                    },
                    how,
                    notice,
                )
            }
        }
    }

    /// Resolve a raw link from the current page and navigate (`Replace`).
    pub fn follow_link(&mut self, raw: &str) -> Vec<Effect> {
        let from = self.tab().current().page.clone();
        let outcome = resolve(raw, &from, &self.index);
        let mut effects = self.navigate(outcome.target, Disposition::Replace);
        if let Some(n) = outcome.notice {
            self.notice = Some(n.clone());
            effects.push(Effect::Notice(n));
        }
        effects
    }

    /// Tree / breadcrumb / search / start-page entry: go to a page with `Replace`.
    pub fn go_to_page(&mut self, key: PageKey) -> Vec<Effect> {
        self.navigate(Target::Page(key, None), Disposition::Replace)
    }

    /// Prev page in the nav tree.
    pub fn go_prev(&mut self) -> Vec<Effect> {
        let cur = self.tab().current().page.clone();
        match self.nav.tree.prev(&cur) {
            Some(key) => self.go_to_page(key),
            None => Vec::new(),
        }
    }

    /// Next page in the nav tree.
    pub fn go_next(&mut self) -> Vec<Effect> {
        let cur = self.tab().current().page.clone();
        match self.nav.tree.next(&cur) {
            Some(key) => self.go_to_page(key),
            None => Vec::new(),
        }
    }

    /// Save viewer scroll/cursor into the current location (call before navigate).
    pub fn save_view(&mut self, cursor_line: u32, scroll: u32) {
        let loc = self.tabs[self.active].current_mut();
        loc.cursor_line = cursor_line;
        loc.scroll = scroll;
    }

    /// Browser back.
    pub fn back(&mut self) -> Vec<Effect> {
        let tab = &mut self.tabs[self.active];
        if tab.cursor == 0 {
            return Vec::new();
        }
        tab.cursor -= 1;
        self.effects_for_current()
    }

    /// Browser forward.
    pub fn forward(&mut self) -> Vec<Effect> {
        let tab = &mut self.tabs[self.active];
        if tab.cursor + 1 >= tab.history.len() {
            return Vec::new();
        }
        tab.cursor += 1;
        self.effects_for_current()
    }

    fn push_location(
        &mut self,
        loc: Location,
        how: Disposition,
        notice: Option<String>,
    ) -> Vec<Effect> {
        self.notice.clone_from(&notice);
        match how {
            Disposition::Replace => {
                let tab = &mut self.tabs[self.active];
                tab.history.truncate(tab.cursor + 1);
                tab.history.push(loc);
                tab.cursor = tab.history.len() - 1;
            }
            Disposition::NewTab => {
                self.tabs.push(Tab {
                    history: vec![loc],
                    cursor: 0,
                });
                self.active = self.tabs.len() - 1;
            }
            Disposition::BackgroundTab => {
                self.tabs.push(Tab {
                    history: vec![loc],
                    cursor: 0,
                });
            }
        }
        let mut effects = self.effects_for_current();
        if let Some(n) = notice {
            effects.push(Effect::Notice(n));
        }
        effects
    }

    fn effects_for_current(&mut self) -> Vec<Effect> {
        let page = self.tab().current().page.clone();
        let anchor = self.tab().current().anchor.clone();
        expand_ancestors(&self.nav.tree, &page, &mut self.nav.expanded);
        self.nav.cursor = Some(NodeId::Page(page.clone()));
        self.nav.seen_page = Some(page.clone());
        vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(anchor),
        ]
    }
}

fn expand_ancestors(tree: &NavTree, page: &PageKey, expanded: &mut HashSet<NodeId>) {
    expand_in_items(&tree.items, page, expanded);
}

fn expand_in_items(
    items: &[super::tree::NavItem],
    page: &PageKey,
    expanded: &mut HashSet<NodeId>,
) -> bool {
    use super::tree::NavItem;
    for item in items {
        match item {
            NavItem::Page { key, .. } if key == page => return true,
            NavItem::Page { .. } => {}
            NavItem::Group { id, children, .. } => {
                if expand_in_items(children, page, expanded) {
                    expanded.insert(id.clone());
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FsProvider;
    use std::path::{Path, PathBuf};

    fn worked() -> Navigator {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let index = Index::build(&FsProvider::open(root).unwrap()).unwrap();
        Navigator::new(index, None)
    }

    fn key(path: &str) -> PageKey {
        PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from(path),
        }
    }

    fn history_pages(nav: &Navigator) -> Vec<PathBuf> {
        nav.tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect()
    }

    /// N1: every entry point yields the same history + tree selection for the same page.
    fn assert_same_arrival(nav: &Navigator, expected: &PageKey) {
        assert_eq!(&nav.tab().current().page, expected);
        assert_eq!(nav.nav.cursor, Some(NodeId::Page(expected.clone())));
        assert_eq!(nav.nav.seen_page.as_ref(), Some(expected));
    }

    #[test]
    fn start_page_selects_root_readme() {
        let nav = worked();
        assert_eq!(nav.tab_count(), 1);
        assert_same_arrival(&nav, &key("README.md"));
    }

    #[test]
    fn tree_select_replace_pushes_history() {
        let mut nav = worked();
        let target = key("architecture/README.md");
        nav.go_to_page(target.clone());
        assert_same_arrival(&nav, &target);
        assert_eq!(
            history_pages(&nav),
            vec![
                PathBuf::from("README.md"),
                PathBuf::from("architecture/README.md")
            ]
        );
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn link_follow_matches_tree_select_history() {
        let mut via_tree = worked();
        let mut via_link = worked();
        let target = key("architecture/README.md");
        via_tree.go_to_page(target.clone());
        via_link.follow_link("architecture/README.md");
        assert_eq!(via_tree.tab().history, via_link.tab().history);
        assert_eq!(via_tree.nav.cursor, via_link.nav.cursor);
        assert_same_arrival(&via_link, &target);
    }

    #[test]
    fn breadcrumb_and_prev_next_funnel_through_navigate() {
        let mut nav = worked();
        // breadcrumb ≡ go_to_page
        nav.go_to_page(key("architecture/design-system/tokens.md"));
        let after_crumb = nav.tab().history.clone();
        let cursor = nav.nav.cursor.clone();

        let mut nav2 = worked();
        nav2.go_to_page(key("architecture/design-system/tokens.md"));
        assert_eq!(nav2.tab().history, after_crumb);
        assert_eq!(nav2.nav.cursor, cursor);

        // prev/next from architecture landing
        let mut nav3 = worked();
        nav3.go_to_page(key("architecture/README.md"));
        nav3.go_next();
        assert_eq!(
            nav3.tab().current().page.relative_path,
            PathBuf::from("architecture/design-system/README.md")
        );
        nav3.go_prev();
        assert_eq!(
            nav3.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        assert_eq!(nav3.tab_count(), 1);
    }

    #[test]
    fn anchor_jump_pushes_history_entry() {
        let mut nav = worked();
        nav.navigate(
            Target::Anchor("worked-example-wiki".into()),
            Disposition::Replace,
        );
        assert_eq!(nav.tab().history.len(), 2);
        assert_eq!(
            nav.tab().current().anchor.as_deref(),
            Some("worked-example-wiki")
        );
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn back_restores_scroll_and_forward_round_trips() {
        let mut nav = worked();
        nav.save_view(12, 34);
        nav.go_to_page(key("architecture/README.md"));
        nav.save_view(1, 2);
        nav.go_to_page(key("architecture/design-system/tokens.md"));

        assert_eq!(nav.tab().history.len(), 3);
        nav.back();
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        assert_eq!(nav.tab().current().cursor_line, 1);
        assert_eq!(nav.tab().current().scroll, 2);

        nav.back();
        assert_eq!(nav.tab().current().cursor_line, 12);
        assert_eq!(nav.tab().current().scroll, 34);

        // back ×10 never creates tabs; stays at start
        for _ in 0..10 {
            nav.back();
        }
        assert_eq!(nav.tab_count(), 1);
        assert_eq!(nav.tab().cursor, 0);

        nav.forward();
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        nav.forward();
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/design-system/tokens.md")
        );
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn replace_never_opens_a_second_tab() {
        let mut nav = worked();
        let paths = [
            "architecture/README.md",
            "architecture/design-system/tokens.md",
            "decisions/0001-stack.md",
            "architecture/wfos/README.md",
            "decisions/0002-adapters.md",
            "architecture/design-system/README.md",
            "README.md",
            "architecture/README.md",
            "architecture/design-system/tokens.md",
            "decisions/0001-stack.md",
        ];
        for p in paths {
            nav.go_to_page(key(p));
        }
        for _ in 0..10 {
            nav.back();
        }
        assert_eq!(nav.tab_count(), 1);
    }
}
