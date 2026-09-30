//! Navigation session: tabs, history, `navigate` / `back` / `forward`.
//!
//! See [architecture overview](../../../../../wiki/architecture/overview.md) and
//! [ADR-0005](../../../../../wiki/decisions/0005-navigation-model.md).

use std::collections::HashSet;

use crate::Error;
use crate::index::Index;
use crate::provider::PageKey;

use super::resolve::{Target, resolve};
use super::tree::{NavItem, NavTree, NodeId};

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

/// Viewer cursor/scroll captured before a navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewState {
    /// 0-based **source** line (stable across raw/rendered).
    pub cursor_line: u32,
    /// Viewer scroll offset (display lines; approximate across modes).
    pub scroll: u32,
}

/// A point in history within a tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Page shown.
    pub page: PageKey,
    /// Optional heading slug.
    pub anchor: Option<String>,
    /// 0-based **source** line (restored on back).
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

/// Side-nav keyboard cursor stop (search row or a tree node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavStop {
    /// ⌕ Search… row above the tree.
    Search,
    /// Page, group, or `OtherPages` node.
    Node(NodeId),
}

/// Side-nav UI state derived from the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavState {
    /// Site nav tree.
    pub tree: NavTree,
    /// Expanded group ids.
    pub expanded: HashSet<NodeId>,
    /// Keyboard cursor (search row or tree node).
    pub cursor: NavStop,
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
    /// Current page no longer exists; keep history for Back.
    PageRemoved,
}

/// Pure navigation state machine (no terminal deps).
#[derive(Debug, Clone)]
pub struct Navigator {
    index: Index,
    tabs: Vec<Tab>,
    active: usize,
    nav: NavState,
    notice: Option<String>,
    label_mode: crate::config::LabelMode,
}

impl Navigator {
    /// Start on `start` (or the root README / first page).
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyCollection`] when the index has no pages, or
    /// [`Error::PageNotFound`] when `start` is not in the index.
    pub fn new(index: Index, start: Option<PageKey>) -> Result<Self, Error> {
        Self::new_with_labels(index, start, crate::config::LabelMode::Title)
    }

    /// Like [`new`] with an explicit nav label mode.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyCollection`] when the index has no pages, or
    /// [`Error::PageNotFound`] when `start` is not in the index.
    pub fn new_with_labels(
        index: Index,
        start: Option<PageKey>,
        labels: crate::config::LabelMode,
    ) -> Result<Self, Error> {
        if index.pages.is_empty() {
            return Err(Error::EmptyCollection);
        }
        let tree = NavTree::build_with(&index, labels);
        let start = match start {
            Some(key) if index.pages.contains_key(&key) => key,
            Some(key) => return Err(Error::PageNotFound(key)),
            None => tree
                .page_order()
                .into_iter()
                .next()
                .ok_or(Error::EmptyCollection)?,
        };
        let loc = Location {
            page: start.clone(),
            anchor: None,
            cursor_line: 0,
            scroll: 0,
            mode: ViewMode::Rendered,
        };
        let mut expanded = HashSet::new();
        expand_ancestors(&tree, &start, &mut expanded);
        Ok(Self {
            index,
            tabs: vec![Tab {
                history: vec![loc],
                cursor: 0,
            }],
            active: 0,
            nav: NavState {
                tree,
                expanded,
                cursor: NavStop::Node(NodeId::Page(start.clone())),
                seen_page: Some(start),
            },
            notice: None,
            label_mode: labels,
        })
    }

    /// Collection index.
    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Set viewer mode on the current history entry.
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.tabs[self.active].current_mut().mode = mode;
    }

    /// Replace the index after a filesystem change; preserve tabs/history and live view.
    #[must_use]
    pub fn reindex(&mut self, index: Index, view: ViewState) -> Vec<Effect> {
        self.save_view(view);
        let cur = self.tab().current().page.clone();
        self.index = index;
        self.nav.tree = NavTree::build_with(&self.index, self.label_mode);

        let ids = collect_node_ids(&self.nav.tree.items);
        self.nav.expanded.retain(|id| ids.contains(id));

        if let NavStop::Node(id) = &self.nav.cursor
            && !ids.contains(id)
        {
            self.nav.cursor = NavStop::Search;
        }

        if !self.index.pages.contains_key(&cur) {
            self.notice = Some("page removed".into());
            // Keep history so Back still works; do not LoadPage a missing file.
            return vec![Effect::PageRemoved];
        }
        self.notice = None;
        expand_ancestors(&self.nav.tree, &cur, &mut self.nav.expanded);
        // Same-page reload: restore live cursor/scroll — do not re-apply the anchor.
        vec![
            Effect::LoadPage(cur.clone()),
            Effect::RevealInTree(cur),
            Effect::ScrollTo(None),
        ]
    }

    /// Open tabs.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Active tab index.
    #[must_use]
    pub fn active(&self) -> usize {
        self.active
    }

    /// Side nav state.
    #[must_use]
    pub fn nav(&self) -> &NavState {
        &self.nav
    }

    /// Mutable side nav state (session restore).
    pub fn nav_mut(&mut self) -> &mut NavState {
        &mut self.nav
    }

    /// Replace tabs after session restore (caller validated non-empty).
    pub fn replace_tabs(&mut self, tabs: Vec<Tab>, active: usize) {
        if tabs.is_empty() {
            return;
        }
        self.tabs = tabs;
        self.active = active.min(self.tabs.len().saturating_sub(1));
        let page = self.tab().current().page.clone();
        self.nav.seen_page = Some(page);
    }

    /// Footer notice from the last navigation, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
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

    /// Set the side-nav keyboard cursor to a tree node.
    pub fn set_nav_cursor(&mut self, id: NodeId) {
        self.nav.cursor = NavStop::Node(id);
    }

    /// Set the side-nav keyboard cursor (search or node).
    pub fn set_nav_stop(&mut self, stop: NavStop) {
        self.nav.cursor = stop;
    }

    /// Expand or collapse a group in the side nav.
    pub fn set_group_expanded(&mut self, id: NodeId, expanded: bool) {
        if expanded {
            self.nav.expanded.insert(id);
        } else {
            self.nav.expanded.remove(&id);
        }
    }

    /// Record the current page as `seen_page` when the nav loses focus.
    pub fn nav_focus_lost(&mut self) {
        self.nav.seen_page = Some(self.tab().current().page.clone());
    }

    /// Apply the stale-cursor rule when the nav gains focus; returns the cursor.
    ///
    /// If the current page changed while the nav was unfocused, the cursor jumps
    /// to the current page item. Otherwise the remembered cursor is kept
    /// (including [`NavStop::Search`]; defaulting to the current page when the
    /// remembered stop was somehow empty — not applicable with [`NavStop`]).
    pub fn nav_focus_gained(&mut self) -> NavStop {
        let current = self.tab().current().page.clone();
        let stale = self.nav.seen_page.as_ref() != Some(&current);
        let stop = if stale {
            NavStop::Node(NodeId::Page(current.clone()))
        } else {
            self.nav.cursor.clone()
        };
        self.nav.cursor = stop.clone();
        self.nav.seen_page = Some(current);
        stop
    }

    /// Navigate to a resolved or unresolved [`Target`].
    pub fn navigate(&mut self, target: Target, how: Disposition, view: ViewState) -> Vec<Effect> {
        match target {
            Target::External(url) => {
                // Leave history unchanged.
                vec![Effect::ConfirmExternal(url)]
            }
            Target::Unresolved(raw) => {
                // Leave history unchanged.
                self.notice = Some(format!("broken link: {raw}"));
                vec![Effect::Notice(format!("broken link: {raw}"))]
            }
            Target::Anchor(slug) => {
                self.save_view(view);
                let page = self.tab().current().page.clone();
                let outcome = resolve(&format!("#{slug}"), &page, &self.index);
                self.push_location(
                    Location {
                        page: page.clone(),
                        anchor: Some(slug),
                        cursor_line: 0,
                        scroll: 0,
                        mode: self.tab().current().mode,
                    },
                    how,
                    outcome.notice,
                )
            }
            Target::Page(key, anchor) => {
                self.save_view(view);
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
    pub fn follow_link(&mut self, raw: &str, view: ViewState) -> Vec<Effect> {
        let from = self.tab().current().page.clone();
        let outcome = resolve(raw, &from, &self.index);
        // Single notice path: `push_location` / Unresolved arm emit Notice.
        self.navigate(outcome.target, Disposition::Replace, view)
    }

    /// Tree / breadcrumb / search / start-page entry: go to a page with `Replace`.
    pub fn go_to_page(&mut self, key: PageKey, view: ViewState) -> Vec<Effect> {
        self.navigate(Target::Page(key, None), Disposition::Replace, view)
    }

    /// Prev page in the nav tree.
    pub fn go_prev(&mut self, view: ViewState) -> Vec<Effect> {
        let cur = self.tab().current().page.clone();
        match self.nav.tree.prev(&cur) {
            Some(key) => self.go_to_page(key, view),
            None => Vec::new(),
        }
    }

    /// Next page in the nav tree.
    pub fn go_next(&mut self, view: ViewState) -> Vec<Effect> {
        let cur = self.tab().current().page.clone();
        match self.nav.tree.next(&cur) {
            Some(key) => self.go_to_page(key, view),
            None => Vec::new(),
        }
    }

    /// Browser back (saves current view onto the location being left).
    pub fn back(&mut self, view: ViewState) -> Vec<Effect> {
        self.save_view(view);
        let tab = &mut self.tabs[self.active];
        if tab.cursor == 0 {
            return Vec::new();
        }
        tab.cursor -= 1;
        self.notice = None;
        self.effects_for_current()
    }

    /// Browser forward.
    pub fn forward(&mut self, view: ViewState) -> Vec<Effect> {
        self.save_view(view);
        let tab = &mut self.tabs[self.active];
        if tab.cursor + 1 >= tab.history.len() {
            return Vec::new();
        }
        tab.cursor += 1;
        self.notice = None;
        self.effects_for_current()
    }

    fn save_view(&mut self, view: ViewState) {
        let loc = self.tabs[self.active].current_mut();
        loc.cursor_line = view.cursor_line;
        loc.scroll = view.scroll;
    }

    fn push_location(
        &mut self,
        loc: Location,
        how: Disposition,
        notice: Option<String>,
    ) -> Vec<Effect> {
        // Identical page+anchor: refresh notice / reveal, but do not push history.
        if how == Disposition::Replace {
            let cur = self.tab().current();
            if cur.page == loc.page && cur.anchor == loc.anchor {
                self.notice.clone_from(&notice);
                let mut effects = self.effects_for_current();
                if let Some(n) = notice {
                    effects.push(Effect::Notice(n));
                }
                return effects;
            }
        }

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
        // Expand ancestors for ● / N4. Do not clobber nav cursor or seen_page —
        // those are owned by focus (nav_focus_lost / nav_focus_gained).
        expand_ancestors(&self.nav.tree, &page, &mut self.nav.expanded);
        vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(anchor),
        ]
    }
}

fn collect_node_ids(items: &[NavItem]) -> HashSet<NodeId> {
    let mut ids = HashSet::new();
    collect_node_ids_into(items, &mut ids);
    ids.insert(NodeId::OtherPages);
    ids
}

fn collect_node_ids_into(items: &[NavItem], ids: &mut HashSet<NodeId>) {
    for item in items {
        match item {
            NavItem::Page { id, .. } => {
                ids.insert(id.clone());
            }
            NavItem::Group { id, children, .. } => {
                ids.insert(id.clone());
                collect_node_ids_into(children, ids);
            }
        }
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
        Navigator::new(index, None).unwrap()
    }

    fn key(path: &str) -> PageKey {
        PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from(path),
        }
    }

    fn view(cursor_line: u32, scroll: u32) -> ViewState {
        ViewState {
            cursor_line,
            scroll,
        }
    }

    fn history_pages(nav: &Navigator) -> Vec<PathBuf> {
        nav.tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect()
    }

    /// N1: every entry point yields the same history + tree reveal for the same page.
    fn assert_same_arrival(nav: &Navigator, expected: &PageKey) {
        assert_eq!(&nav.tab().current().page, expected);
        let mut ancestors = HashSet::new();
        expand_ancestors(&nav.nav().tree, expected, &mut ancestors);
        for id in &ancestors {
            assert!(
                nav.nav().expanded.contains(id),
                "expected ancestor {id:?} expanded for {}; expanded={:?}",
                expected.relative_path.display(),
                nav.nav().expanded
            );
        }
    }

    /// N4: ancestors expanded; current page marked ● in `render_text`.
    fn assert_n4(nav: &Navigator, expected: &PageKey) {
        assert_same_arrival(nav, expected);
        let text = nav
            .nav()
            .tree
            .render_text(&nav.nav().expanded, Some(expected));
        assert!(
            text.contains('●'),
            "expected ● marker for current page, got:\n{text}"
        );
        // Nested pages must have ancestor groups expanded.
        let path = &expected.relative_path;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            assert!(
                nav.nav()
                    .expanded
                    .contains(&NodeId::Group(parent.to_path_buf())),
                "expected ancestor {parent:?} expanded; expanded={:?}",
                nav.nav().expanded
            );
        }
    }

    #[test]
    fn empty_collection_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::build(&FsProvider::open(dir.path()).unwrap()).unwrap();
        let err = Navigator::new(index, None).unwrap_err();
        assert!(matches!(err, Error::EmptyCollection));
    }

    #[test]
    fn start_page_not_found_is_error() {
        let nav = worked();
        let index = nav.index().clone();
        let err = Navigator::new(index, Some(key("nope.md"))).unwrap_err();
        assert!(matches!(err, Error::PageNotFound(_)));
    }

    #[test]
    fn start_page_selects_root_readme() {
        let nav = worked();
        assert_eq!(nav.tab_count(), 1);
        assert_n4(&nav, &key("README.md"));
    }

    #[test]
    fn tree_select_replace_pushes_history() {
        let mut nav = worked();
        let target = key("architecture/README.md");
        nav.go_to_page(target.clone(), ViewState::default());
        assert_n4(&nav, &target);
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
    fn entry_points_agree_on_history_and_selection() {
        let target = key("architecture/design-system/tokens.md");

        let mut via_tree = worked();
        via_tree.go_to_page(target.clone(), ViewState::default());

        let mut via_link = worked();
        via_link.follow_link("architecture/design-system/tokens.md", ViewState::default());

        let mut via_search = worked();
        via_search.go_to_page(target.clone(), ViewState::default()); // search → go_to_page

        let mut via_crumb = worked();
        via_crumb.go_to_page(target.clone(), ViewState::default()); // breadcrumb → go_to_page

        let mut via_next = worked();
        via_next.go_to_page(
            key("architecture/design-system/README.md"),
            ViewState::default(),
        );
        via_next.go_next(ViewState::default());

        for nav in [&via_tree, &via_link, &via_search, &via_crumb, &via_next] {
            assert_n4(nav, &target);
        }
        // Direct entry points share history shape; prev/next may include intermediate stops.
        assert_eq!(via_tree.tab().history, via_link.tab().history);
        assert_eq!(via_tree.tab().history, via_search.tab().history);
        assert_eq!(via_tree.tab().history, via_crumb.tab().history);
        assert_eq!(via_next.tab().current().page, target);
    }

    #[test]
    fn prev_next_move_in_tree_order() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), ViewState::default());
        nav.go_next(ViewState::default());
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/design-system/README.md")
        );
        assert_n4(&nav, &key("architecture/design-system/README.md"));
        nav.go_prev(ViewState::default());
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn identical_anchor_does_not_duplicate_history() {
        let mut nav = worked();
        nav.navigate(
            Target::Anchor("worked-example-wiki".into()),
            Disposition::Replace,
            ViewState::default(),
        );
        assert_eq!(nav.tab().history.len(), 2);
        nav.navigate(
            Target::Anchor("worked-example-wiki".into()),
            Disposition::Replace,
            ViewState::default(),
        );
        assert_eq!(nav.tab().history.len(), 2);
    }

    #[test]
    fn unresolved_and_external_leave_history_unchanged() {
        let mut nav = worked();
        let before = nav.tab().history.clone();
        let effects = nav.navigate(
            Target::Unresolved("missing.md".into()),
            Disposition::Replace,
            ViewState::default(),
        );
        assert_eq!(nav.tab().history, before);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::Notice(_)));

        let effects = nav.navigate(
            Target::External("https://example.com".into()),
            Disposition::Replace,
            ViewState::default(),
        );
        assert_eq!(nav.tab().history, before);
        assert!(matches!(effects[0], Effect::ConfirmExternal(_)));
    }

    #[test]
    fn new_tab_and_background_tab() {
        let mut nav = worked();
        nav.navigate(
            Target::Page(key("architecture/README.md"), None),
            Disposition::NewTab,
            ViewState::default(),
        );
        assert_eq!(nav.tab_count(), 2);
        assert_eq!(nav.active(), 1);
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );

        nav.navigate(
            Target::Page(key("decisions/0001-stack.md"), None),
            Disposition::BackgroundTab,
            ViewState::default(),
        );
        assert_eq!(nav.tab_count(), 3);
        assert_eq!(nav.active(), 1); // focus unchanged
    }

    #[test]
    fn forward_truncated_after_back_then_navigate() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), ViewState::default());
        nav.go_to_page(
            key("architecture/design-system/tokens.md"),
            ViewState::default(),
        );
        nav.back(ViewState::default());
        assert_eq!(nav.tab().history.len(), 3);
        nav.go_to_page(key("decisions/0001-stack.md"), ViewState::default());
        assert_eq!(
            history_pages(&nav),
            vec![
                PathBuf::from("README.md"),
                PathBuf::from("architecture/README.md"),
                PathBuf::from("decisions/0001-stack.md"),
            ]
        );
        assert!(nav.forward(ViewState::default()).is_empty());
    }

    #[test]
    fn single_notice_per_follow_link() {
        let mut nav = worked();
        // Link to missing page → one Notice.
        let effects = nav.follow_link("./nope-missing.md", ViewState::default());
        let notices: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::Notice(_)))
            .collect();
        assert_eq!(notices.len(), 1, "effects={effects:?}");
    }

    #[test]
    fn reindex_keeps_history_when_page_still_exists() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), ViewState::default());
        let index = nav.index().clone();
        let effects = nav.reindex(index, ViewState::default());
        assert!(effects.iter().any(|e| matches!(e, Effect::LoadPage(_))));
        assert!(
            effects.iter().any(|e| matches!(e, Effect::ScrollTo(None))),
            "reindex must restore view, not re-apply anchor: {effects:?}"
        );
        assert_eq!(nav.tab().history.len(), 2);
    }

    #[test]
    fn reindex_anchored_page_keeps_saved_cursor() {
        let mut nav = worked();
        nav.go_to_page(key("README.md"), ViewState::default());
        // Navigate via anchor then move the live cursor away from the heading.
        nav.navigate(
            Target::Anchor("worked-example-wiki".into()),
            Disposition::Replace,
            ViewState::default(),
        );
        assert!(nav.tab().current().anchor.is_some());
        let index = nav.index().clone();
        let effects = nav.reindex(index, view(26, 10));
        assert!(
            effects.iter().any(|e| matches!(e, Effect::ScrollTo(None))),
            "anchored reload must not ScrollTo(Some): {effects:?}"
        );
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::ScrollTo(Some(_))))
        );
        assert_eq!(nav.tab().current().cursor_line, 26);
        assert_eq!(nav.tab().current().scroll, 10);
    }

    #[test]
    fn reindex_missing_page_emits_page_removed() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), ViewState::default());
        // Empty-ish index: rebuild from a tiny temp collection without that page.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("only.md"), "# only\n").unwrap();
        let index = Index::build(&FsProvider::open(dir.path()).unwrap()).unwrap();
        let effects = nav.reindex(index, ViewState::default());
        assert!(
            matches!(effects.as_slice(), [Effect::PageRemoved]),
            "{effects:?}"
        );
        assert_eq!(nav.notice(), Some("page removed"));
        // History preserved for Back.
        assert!(nav.tab().history.len() >= 2);
    }

    #[test]
    fn back_clears_stale_notice() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), ViewState::default());
        nav.follow_link(
            "design-system/tokens.md#missing-anchor",
            ViewState::default(),
        );
        assert!(nav.notice().is_some());
        nav.back(ViewState::default());
        assert!(nav.notice().is_none());
    }

    #[test]
    fn back_restores_scroll_and_forward_round_trips() {
        let mut nav = worked();
        nav.go_to_page(key("architecture/README.md"), view(12, 34));
        nav.go_to_page(key("architecture/design-system/tokens.md"), view(1, 2));

        assert_eq!(nav.tab().history.len(), 3);
        // History[0] should have saved 12/34 from the navigate that left README.
        assert_eq!(nav.tab().history[0].cursor_line, 12);
        assert_eq!(nav.tab().history[0].scroll, 34);

        nav.back(view(9, 9));
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        assert_eq!(nav.tab().current().cursor_line, 1);
        assert_eq!(nav.tab().current().scroll, 2);

        nav.back(ViewState::default());
        assert_eq!(nav.tab().current().cursor_line, 12);
        assert_eq!(nav.tab().current().scroll, 34);

        for _ in 0..10 {
            nav.back(ViewState::default());
        }
        assert_eq!(nav.tab_count(), 1);
        assert_eq!(nav.tab().cursor, 0);

        nav.forward(ViewState::default());
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
        nav.forward(ViewState::default());
        assert_eq!(
            nav.tab().current().page.relative_path,
            PathBuf::from("architecture/design-system/tokens.md")
        );
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn exit_criterion_ten_links_back_ten() {
        let mut nav = worked();
        let start = nav.tab().current().page.clone();
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
        nav.go_to_page(key(paths[0]), view(10, 20));
        for (i, p) in paths.iter().enumerate().skip(1) {
            nav.go_to_page(
                key(p),
                view(u32::try_from(i).unwrap(), u32::try_from(i * 2).unwrap()),
            );
        }
        assert_eq!(nav.tab().history.len(), 11); // start + 10
        assert_eq!(nav.tab_count(), 1);

        for _ in 0..10 {
            nav.back(ViewState::default());
        }
        assert_eq!(&nav.tab().current().page, &start);
        assert_eq!(nav.tab().current().cursor_line, 10);
        assert_eq!(nav.tab().current().scroll, 20);
        assert_eq!(nav.tab_count(), 1);

        // Forward round-trip through the stack.
        for p in paths {
            nav.forward(ViewState::default());
            assert_eq!(nav.tab().current().page.relative_path, PathBuf::from(p));
        }
        assert_eq!(nav.tab_count(), 1);
    }

    #[test]
    fn nav_focus_stale_cursor_and_defaults() {
        let mut nav = worked();
        let root = key("README.md");
        let tokens = key("architecture/design-system/tokens.md");

        // Default: remembered cursor is the start page.
        assert_eq!(nav.nav().cursor, NavStop::Node(NodeId::Page(root.clone())));
        assert_eq!(nav.nav().seen_page.as_ref(), Some(&root));

        // Remember a different cursor, leave nav, navigate elsewhere, refocus → jump.
        nav.set_nav_cursor(NodeId::Page(key("decisions/0001-stack.md")));
        nav.nav_focus_lost();
        assert_eq!(nav.nav().seen_page.as_ref(), Some(&root));
        nav.go_to_page(tokens.clone(), ViewState::default());
        // Cursor not clobbered by navigate.
        assert_eq!(
            nav.nav().cursor,
            NavStop::Node(NodeId::Page(key("decisions/0001-stack.md")))
        );
        let gained = nav.nav_focus_gained();
        assert_eq!(gained, NavStop::Node(NodeId::Page(tokens.clone())));
        assert_eq!(
            nav.nav().cursor,
            NavStop::Node(NodeId::Page(tokens.clone()))
        );
        assert_eq!(nav.nav().seen_page.as_ref(), Some(&tokens));

        // Round-trip through viewer without navigate keeps remembered cursor.
        nav.set_nav_cursor(NodeId::Page(key("architecture/README.md")));
        nav.nav_focus_lost();
        let kept = nav.nav_focus_gained();
        assert_eq!(
            kept,
            NavStop::Node(NodeId::Page(key("architecture/README.md")))
        );

        // Search stop kept when not stale.
        nav.set_nav_stop(NavStop::Search);
        nav.nav_focus_lost();
        assert_eq!(nav.nav_focus_gained(), NavStop::Search);
    }

    #[test]
    fn set_group_expanded_toggles() {
        let mut nav = worked();
        let id = NodeId::Group(PathBuf::from("decisions"));
        assert!(!nav.nav().expanded.contains(&id));
        nav.set_group_expanded(id.clone(), true);
        assert!(nav.nav().expanded.contains(&id));
        nav.set_group_expanded(id.clone(), false);
        assert!(!nav.nav().expanded.contains(&id));
    }
}
