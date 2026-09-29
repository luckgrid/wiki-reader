//! App shell: state, update, render, event loop.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use wiki_reader_core::Index;
use wiki_reader_core::nav::{Effect, NavStop, Navigator, NodeId, ViewState};
use wiki_reader_core::provider::{CollectionProvider, FsProvider, PageKey};

use super::action::Action;
use super::focus::FocusPane;
use super::hit::HitMap;
use super::keymap::{Chord, InputMode};
use super::opener::{Opener, SystemOpener};
use super::page_doc::PageDoc;
use super::rendered_doc::RenderedViewerDoc;
use super::search_ui::{SearchMode, SearchOverlay};
use super::theme::Theme;
use super::viewer_doc::{RawDoc, ViewerDoc};
use wiki_reader_core::nav::ViewMode;
use wiki_reader_core::search;

mod draw;
mod events;
mod nav_ui;
mod viewer_state;

#[cfg(test)]
mod tests;

pub use events::run;

/// Owned application state.
#[allow(clippy::struct_excessive_bools)] // pane/nav/quit/missing flags; not a state machine yet
pub struct App {
    /// Navigation session.
    pub navigator: Navigator,
    /// Collection provider.
    pub provider: FsProvider,
    /// Focused pane.
    pub focus: FocusPane,
    /// Side nav visible (docked ≥80 cols, overlay &lt;80).
    pub nav_visible: bool,
    /// User toggled `b`/◫; cleared when width crosses the 80-col boundary.
    pub nav_user_override: bool,
    /// Last drawn terminal width (for overlay close on navigation).
    pub term_width: u16,
    /// Viewer text column width used for the current rendered layout (`min(inner, 100)`).
    pub layout_width: u16,
    /// Previous frame wide (≥80) vs narrow; `None` until first draw.
    nav_width_regime: Option<bool>,
    /// Side nav scroll offset (rows below the search line).
    pub nav_scroll: u16,
    /// Current viewer document.
    pub doc: PageDoc,
    /// Viewer cursor (0-based source line index).
    pub cursor_line: u32,
    /// Viewer scroll offset (lines from top).
    pub scroll: u32,
    /// Tab-cycle focused item index into `focus_list`.
    pub focused_item: Option<usize>,
    /// Status / notice message.
    pub message: String,
    /// Visible viewer body rows (from last layout).
    pub viewer_rows: u16,
    /// Visible nav tree rows below the search line (from last layout).
    pub nav_viewport: u16,
    /// Last frame hit map.
    pub hit_map: HitMap,
    /// Theme tokens.
    pub theme: Theme,
    /// Quit requested.
    pub quit: bool,
    /// Pending multi-key chord (`gg`).
    pub chord: Chord,
    /// Input mode (Normal vs Overlay).
    pub input_mode: InputMode,
    /// URL awaiting y/N confirmation.
    pub pending_external: Option<String>,
    /// External link opener (swappable in tests).
    pub(crate) opener: Box<dyn Opener>,
    /// Search overlay (None when closed).
    pub search: Option<SearchOverlay>,
    /// Optional filesystem watcher (live reload).
    pub(crate) watcher: Option<wiki_reader_core::watch::Watcher>,
    /// In-flight background index rebuild.
    rebuild_rx: Option<Receiver<Result<Index, String>>>,
    /// Dirty arrived while a rebuild was in flight — start another when done.
    rebuild_pending: bool,
    /// In-flight raw-view highlight job (`token` must match [`Self::highlight_token`]).
    highlight_rx: Option<(u64, Receiver<Vec<Vec<crate::tui::highlight::HlSpan>>>)>,
    /// Bumped on each Raw load / leave-Raw so stale highlight results are dropped.
    highlight_token: u64,
    /// Sticky "page removed" until the user navigates elsewhere.
    pub(crate) page_missing: bool,
    /// In-page search match **source** lines (after opening a text hit).
    pub(crate) search_matches: Vec<u32>,
    /// Page that `search_matches` belong to (clear on navigate away).
    search_match_page: Option<PageKey>,
    /// Index into `search_matches`.
    pub(crate) search_match_idx: usize,
    /// Display line highlighted as the current search match.
    pub(crate) match_highlight: Option<u32>,
}

impl App {
    /// Build app from a collection root.
    ///
    /// # Errors
    ///
    /// Propagates provider/index failures. Empty collection / missing start use
    /// process exit in [`run`].
    pub fn new(root: &Path) -> Result<Self, wiki_reader_core::Error> {
        let provider = FsProvider::open(root)?;
        let index = Index::build(&provider)?;
        let navigator = Navigator::new(index, None)?;
        let (watcher, watch_msg) = match wiki_reader_core::watch::Watcher::start(root) {
            Ok(w) => (Some(w), String::new()),
            Err(err) => (None, format!("live reload off: {err}")),
        };
        let mut app = Self {
            navigator,
            provider,
            focus: FocusPane::Viewer,
            nav_visible: false,
            nav_user_override: false,
            term_width: 80,
            layout_width: 0,
            nav_width_regime: None,
            nav_scroll: 0,
            doc: PageDoc::Raw(RawDoc::from_source("", None)),
            cursor_line: 0,
            scroll: 0,
            focused_item: None,
            message: watch_msg,
            // ponytail: defaults until first draw; layout overwrites each frame
            viewer_rows: 20,
            nav_viewport: 20,
            hit_map: HitMap::default(),
            theme: Theme::default(),
            quit: false,
            chord: Chord::None,
            input_mode: InputMode::Normal,
            pending_external: None,
            opener: Box::new(SystemOpener),
            search: None,
            watcher,
            rebuild_rx: None,
            rebuild_pending: false,
            highlight_rx: None,
            highlight_token: 0,
            page_missing: false,
            search_matches: Vec::new(),
            search_match_page: None,
            search_match_idx: 0,
            match_highlight: None,
        };
        let page = app.navigator.tab().current().page.clone();
        app.apply_effects(vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(None),
        ]);
        Ok(app)
    }

    pub(crate) fn view_state(&self) -> ViewState {
        ViewState {
            cursor_line: self.doc.source_cursor(self.cursor_line),
            scroll: self.doc.source_cursor(self.scroll),
        }
    }

    /// Pure state update (unit-testable without a terminal).
    #[allow(clippy::too_many_lines)] // split in P1-R10
    pub fn update(&mut self, action: Action) {
        // Transient notices clear on the next key/action (Tab keeps focus target).
        // Sticky "page removed" survives until real navigation.
        if !self.page_missing
            && !matches!(
                action,
                Action::None
                    | Action::ViewerTab
                    | Action::ViewerBackTab
                    | Action::SearchNextMatch
                    | Action::SearchPrevMatch
            )
        {
            self.message.clear();
        }
        // Clear match highlight on any cursor move outside n/N cycling.
        if !matches!(
            action,
            Action::SearchNextMatch | Action::SearchPrevMatch | Action::None
        ) && matches!(
            action,
            Action::ViewerUp
                | Action::ViewerDown
                | Action::ViewerBlockUp
                | Action::ViewerBlockDown
                | Action::ViewerPageUp
                | Action::ViewerPageDown
                | Action::ViewerHome
                | Action::ViewerEnd
                | Action::SetCursorLine(_)
                | Action::GoToPage(_)
                | Action::Back
                | Action::Forward
                | Action::PrevPage
                | Action::NextPage
                | Action::ViewerTab
                | Action::ViewerBackTab
                | Action::FollowLinkId(_)
                | Action::ViewerActivate
        ) {
            self.clear_search_matches();
        }
        match action {
            Action::Quit => self.quit = true,
            Action::ToggleNav => {
                self.nav_user_override = true;
                self.nav_visible = !self.nav_visible;
            }
            Action::PrevPage | Action::NextPage => {
                let effects = if matches!(action, Action::PrevPage) {
                    self.navigator.go_prev(self.view_state())
                } else {
                    self.navigator.go_next(self.view_state())
                };
                self.apply_effects(effects);
            }
            Action::Back => {
                let effects = self.navigator.back(self.view_state());
                self.apply_effects(effects);
            }
            Action::Forward => {
                let effects = self.navigator.forward(self.view_state());
                self.apply_effects(effects);
            }
            Action::GoToPage(key) => {
                self.navigator.set_nav_cursor(NodeId::Page(key.clone()));
                let effects = self.navigator.go_to_page(key, self.view_state());
                self.apply_effects(effects);
            }
            Action::ToggleGroup(id) => {
                let open = !self.navigator.nav().expanded.contains(&id);
                self.navigator.set_group_expanded(id, open);
                self.clamp_nav_scroll();
            }
            Action::OpenSearch => self.open_search(),
            Action::CloseSearch => self.close_search(false),
            Action::SearchChar(c) => self.search_type(c),
            Action::SearchBackspace => self.search_backspace(),
            Action::SearchSelectDelta(d) => self.search_select(d),
            Action::SearchToggleMode => self.search_toggle_mode(),
            Action::SearchActivate => self.search_activate(None),
            Action::SearchActivateIndex(i) => self.search_activate(Some(i)),
            Action::SearchNextMatch => self.cycle_search_match(1),
            Action::SearchPrevMatch => self.cycle_search_match(-1),
            Action::FocusNav => {
                if self.focus != FocusPane::Nav {
                    self.navigator.nav_focus_gained();
                    self.focus = FocusPane::Nav;
                }
            }
            Action::FocusViewer => {
                if self.focus != FocusPane::Viewer {
                    self.navigator.nav_focus_lost();
                    self.focus = FocusPane::Viewer;
                }
            }
            Action::CycleFocus => {
                if self.focus == FocusPane::Nav {
                    self.update(Action::FocusViewer);
                } else {
                    self.update(Action::FocusNav);
                }
            }
            Action::SetCursorLine(line) => {
                if self.focus != FocusPane::Viewer {
                    self.navigator.nav_focus_lost();
                    self.focus = FocusPane::Viewer;
                }
                let max =
                    u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(u32::MAX);
                self.cursor_line = line.min(max);
                self.focused_item = None;
            }
            Action::NavStepUp => self.nav_step(-1),
            Action::NavStepDown => self.nav_step(1),
            Action::NavJumpUp => self.nav_jump_group(-1),
            Action::NavJumpDown => self.nav_jump_group(1),
            Action::NavExpand => self.nav_expand(),
            Action::NavCollapse => self.nav_collapse(),
            Action::NavActivate => self.nav_activate(),
            Action::ViewerUp => self.viewer_move_line(-1),
            Action::ViewerDown => self.viewer_move_line(1),
            Action::ViewerBlockUp => self.viewer_block(-1),
            Action::ViewerBlockDown => self.viewer_block(1),
            Action::ViewerPageUp => {
                let step = i32::from(self.viewer_rows.saturating_sub(1).max(1));
                self.viewer_move_line(-step);
            }
            Action::ViewerPageDown => {
                let step = i32::from(self.viewer_rows.saturating_sub(1).max(1));
                self.viewer_move_line(step);
            }
            Action::ViewerHome => {
                self.focused_item = None;
                self.cursor_line = 0;
                self.ensure_cursor_visible();
            }
            Action::ViewerEnd => {
                self.focused_item = None;
                let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
                self.cursor_line = max;
                self.ensure_cursor_visible();
            }
            Action::ViewerTab => self.viewer_tab(false),
            Action::ViewerBackTab => self.viewer_tab(true),
            Action::ViewerActivate => self.viewer_activate(),
            Action::FollowLinkId(id) => self.follow_link_id(id),
            Action::ConfirmOpen => {
                if let Some(url) = self.pending_external.take() {
                    let _ = self.opener.open(&url);
                }
                self.input_mode = InputMode::Normal;
                self.message.clear();
            }
            Action::ConfirmDecline => {
                self.pending_external = None;
                self.input_mode = InputMode::Normal;
                self.message.clear();
            }
            Action::ToggleViewMode => self.toggle_view_mode(),
            Action::None => {}
        }
    }

    fn open_search(&mut self) {
        self.navigator.set_nav_stop(NavStop::Search);
        self.search = Some(SearchOverlay {
            query: String::new(),
            mode: SearchMode::Pages,
            selected: 0,
            page_hits: Vec::new(),
            text_hits: Vec::new(),
            prev_focus: self.focus,
            prev_cursor: self.cursor_line,
            prev_scroll: self.scroll,
            prev_nav_stop: self.navigator.nav().cursor.clone(),
        });
        self.input_mode = InputMode::Overlay;
        self.message.clear();
    }

    fn close_search(&mut self, keep_navigation: bool) {
        let Some(overlay) = self.search.take() else {
            return;
        };
        self.input_mode = InputMode::Normal;
        if !keep_navigation {
            self.focus = overlay.prev_focus;
            self.cursor_line = overlay.prev_cursor;
            self.scroll = overlay.prev_scroll;
            match overlay.prev_nav_stop {
                NavStop::Search => self.navigator.set_nav_stop(NavStop::Search),
                NavStop::Node(id) => self.navigator.set_nav_cursor(id),
            }
            self.ensure_cursor_visible();
        }
        self.message.clear();
    }

    fn search_refresh(&mut self) {
        let Some(overlay) = self.search.as_mut() else {
            return;
        };
        let index = self.navigator.index();
        match overlay.mode {
            SearchMode::Pages => {
                overlay.page_hits = search::search_pages(&overlay.query, index);
                overlay.text_hits.clear();
            }
            SearchMode::Text => {
                overlay.text_hits = search::search_text(&overlay.query, index);
                overlay.page_hits.clear();
            }
        }
        overlay.clamp_selected();
    }

    fn search_type(&mut self, c: char) {
        if let Some(overlay) = self.search.as_mut() {
            overlay.query.push(c);
        }
        self.search_refresh();
    }

    fn search_backspace(&mut self) {
        if let Some(overlay) = self.search.as_mut() {
            overlay.query.pop();
        }
        self.search_refresh();
    }

    fn search_select(&mut self, delta: i32) {
        let Some(overlay) = self.search.as_mut() else {
            return;
        };
        let n = i32::try_from(overlay.result_len()).unwrap_or(0);
        if n == 0 {
            return;
        }
        let cur = i32::try_from(overlay.selected).unwrap_or(0);
        let next = (cur + delta).rem_euclid(n);
        overlay.selected = u32::try_from(next).unwrap_or(0) as usize;
    }

    fn search_toggle_mode(&mut self) {
        if let Some(overlay) = self.search.as_mut() {
            overlay.mode = match overlay.mode {
                SearchMode::Pages => SearchMode::Text,
                SearchMode::Text => SearchMode::Pages,
            };
            overlay.selected = 0;
        }
        self.search_refresh();
    }

    fn clear_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_page = None;
        self.search_match_idx = 0;
        self.match_highlight = None;
    }

    /// Dedupe consecutive source hits that collapse to the same display line.
    fn store_search_matches(&mut self, sources: Vec<u32>, prefer_src: u32) {
        let mut deduped = Vec::new();
        let mut last_display = None;
        for src in sources {
            let display = self.doc.display_cursor(src);
            if last_display == Some(display) {
                continue;
            }
            last_display = Some(display);
            deduped.push(src);
        }
        let start_idx = deduped
            .iter()
            .position(|&s| s == prefer_src)
            .or_else(|| {
                let want = self.doc.display_cursor(prefer_src);
                deduped
                    .iter()
                    .position(|&s| self.doc.display_cursor(s) == want)
            })
            .unwrap_or(0);
        self.search_matches = deduped;
        self.search_match_page = Some(self.navigator.tab().current().page.clone());
        self.search_match_idx = start_idx.min(self.search_matches.len().saturating_sub(1));
        self.focus_current_search_match();
    }

    fn focus_current_search_match(&mut self) {
        let Some(&src) = self.search_matches.get(self.search_match_idx) else {
            self.match_highlight = None;
            return;
        };
        let display = self.doc.display_cursor(src);
        self.match_highlight = Some(display);
        self.cursor_line = display;
        self.scroll = display;
        self.ensure_cursor_visible();
        self.message = format!(
            "{}/{}",
            self.search_match_idx.saturating_add(1),
            self.search_matches.len()
        );
    }

    /// Remap match highlight after raw/rendered toggle, resize, or reload.
    fn remap_search_matches(&mut self) {
        if self.search_matches.is_empty() {
            self.match_highlight = None;
            return;
        }
        self.search_match_idx = self
            .search_match_idx
            .min(self.search_matches.len().saturating_sub(1));
        if let Some(&src) = self.search_matches.get(self.search_match_idx) {
            self.match_highlight = Some(self.doc.display_cursor(src));
        }
    }

    fn cycle_search_match(&mut self, delta: i32) {
        if self.search_matches.is_empty() {
            return;
        }
        let n = i32::try_from(self.search_matches.len()).unwrap_or(0);
        if n == 0 {
            return;
        }
        let cur = i32::try_from(self.search_match_idx).unwrap_or(0);
        let next = (cur + delta).rem_euclid(n);
        self.search_match_idx = usize::try_from(next).unwrap_or(0);
        self.focus_current_search_match();
    }

    fn search_activate(&mut self, index: Option<usize>) {
        let Some(overlay) = self.search.as_ref() else {
            return;
        };
        let selected = index.unwrap_or(overlay.selected);
        match overlay.mode {
            SearchMode::Pages => {
                let Some(hit) = overlay.page_hits.get(selected) else {
                    return;
                };
                let key = hit.page.clone();
                self.close_search(true);
                self.clear_search_matches();
                self.focus = FocusPane::Viewer;
                let effects = self.navigator.go_to_page(key, self.view_state());
                self.apply_effects(effects);
            }
            SearchMode::Text => {
                let Some(hit) = overlay.text_hits.get(selected) else {
                    return;
                };
                let key = hit.page.clone();
                let prefer = hit.line.saturating_sub(1);
                let page_hits: Vec<u32> = overlay
                    .text_hits
                    .iter()
                    .filter(|h| h.page == key)
                    .map(|h| h.line.saturating_sub(1))
                    .collect();
                self.close_search(true);
                self.focus = FocusPane::Viewer;
                let effects = self.navigator.go_to_page(key, self.view_state());
                self.apply_effects(effects);
                self.store_search_matches(page_hits, prefer);
            }
        }
    }

    fn toggle_view_mode(&mut self) {
        let key = self.navigator.tab().current().page.clone();
        let source = self.doc.source_cursor(self.cursor_line);
        let source_scroll = self.doc.source_cursor(self.scroll);
        let new_mode = match self.navigator.tab().current().mode {
            ViewMode::Rendered => ViewMode::Raw,
            ViewMode::Raw => ViewMode::Rendered,
        };
        self.navigator.set_view_mode(new_mode);
        self.reload_page_keeping_view(&key, source, source_scroll);
    }

    /// Poll the filesystem watcher and reindex when dirty (rebuild off UI thread).
    pub(crate) fn poll_watcher(&mut self) {
        self.poll_highlight_rx();
        let finished = self.poll_rebuild_rx();
        if let Some(watcher) = self.watcher.as_ref() {
            let poll = watcher.poll_dirty();
            if poll.errors > 0 {
                self.message = format!("live reload: {} watch error(s)", poll.errors);
            }
            self.note_watcher_dirty(poll.dirty);
        }
        if finished && self.rebuild_pending && self.rebuild_rx.is_none() {
            self.rebuild_pending = false;
            self.spawn_rebuild();
        }
    }

    fn poll_highlight_rx(&mut self) {
        let Some((token, rx)) = &self.highlight_rx else {
            return;
        };
        let token = *token;
        match rx.try_recv() {
            Ok(hl) => {
                self.highlight_rx = None;
                if token == self.highlight_token
                    && let PageDoc::Raw(doc) = &mut self.doc
                {
                    doc.set_highlights(hl);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.highlight_rx = None;
            }
        }
    }

    fn cancel_highlight(&mut self) {
        self.highlight_token = self.highlight_token.wrapping_add(1);
        self.highlight_rx = None;
    }

    fn spawn_highlight(&mut self, source: String) {
        self.highlight_token = self.highlight_token.wrapping_add(1);
        let token = self.highlight_token;
        let (tx, rx) = mpsc::channel();
        self.highlight_rx = Some((token, rx));
        std::thread::spawn(move || {
            let hl = crate::tui::highlight::highlight_markdown(&source);
            let _ = tx.send(hl);
        });
    }

    /// Apply a completed rebuild if ready. Returns true when a rebuild just finished.
    fn poll_rebuild_rx(&mut self) -> bool {
        let Some(rx) = &self.rebuild_rx else {
            return false;
        };
        match rx.try_recv() {
            Ok(Ok(index)) => {
                self.rebuild_rx = None;
                let view = self.view_state();
                let effects = self.navigator.reindex(index, view);
                self.apply_effects(effects);
                true
            }
            Ok(Err(err)) => {
                self.rebuild_rx = None;
                self.message = format!("reindex failed: {err}");
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.rebuild_rx = None;
                true
            }
        }
    }

    /// Record dirty: start a rebuild, or queue one if already rebuilding.
    fn note_watcher_dirty(&mut self, dirty: bool) {
        if !dirty {
            return;
        }
        if self.rebuild_rx.is_some() {
            self.rebuild_pending = true;
            return;
        }
        self.spawn_rebuild();
    }

    fn spawn_rebuild(&mut self) {
        let provider = self.provider.clone();
        let (tx, rx) = mpsc::channel();
        self.rebuild_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Index::build(&provider).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    pub(crate) fn apply_effects(&mut self, effects: Vec<Effect>) {
        let page_changed = effects.iter().any(|e| matches!(e, Effect::LoadPage(_)));
        for effect in effects {
            match effect {
                Effect::LoadPage(key) => {
                    if self.search_match_page.as_ref() != Some(&key) {
                        self.clear_search_matches();
                    }
                    self.page_missing = false;
                    self.load_page(&key);
                    if self.search_match_page.as_ref() == Some(&key) {
                        self.remap_search_matches();
                    }
                }
                Effect::RevealInTree(page) => {
                    self.reveal_page_in_nav(&page);
                }
                Effect::ScrollTo(anchor) => {
                    if let Some(slug) = anchor {
                        if let Some(line) = self.anchor_line(&slug) {
                            self.cursor_line = line.saturating_sub(1);
                            self.scroll = self.cursor_line;
                        }
                    } else {
                        let loc = self.navigator.tab().current();
                        self.cursor_line = self.doc.display_cursor(loc.cursor_line);
                        self.scroll = self.doc.display_cursor(loc.scroll);
                        self.ensure_cursor_visible();
                    }
                }
                Effect::Notice(msg) => self.message = msg,
                Effect::ConfirmExternal(url) => {
                    self.pending_external = Some(url.clone());
                    self.input_mode = InputMode::Confirm;
                    self.message = format!("open {url}? [y/N]");
                }
                Effect::PageRemoved => {
                    self.page_missing = true;
                    self.message = "page removed".into();
                    self.cancel_highlight();
                    self.doc = PageDoc::Raw(RawDoc::from_source(
                        "# page removed\n\nThis page no longer exists on disk.\nPress Back to leave.\n",
                        None,
                    ));
                    self.cursor_line = 0;
                    self.scroll = 0;
                    self.focused_item = None;
                }
            }
        }
        if let Some(n) = self.navigator.notice() {
            self.message = n.to_owned();
        }
        // Navigation while nav is focused: cursor follows the current page.
        if self.focus == FocusPane::Nav {
            let page = self.navigator.tab().current().page.clone();
            self.navigator.set_nav_cursor(NodeId::Page(page.clone()));
            self.reveal_page_in_nav(&page);
        }
        if page_changed && self.term_width < 80 {
            self.nav_visible = false;
        }
    }

    pub(crate) fn load_page(&mut self, key: &PageKey) {
        match self.provider.read(key) {
            Ok(src) => {
                let index = self.navigator.index();
                let page = index.pages.get(key);
                let mode = self.navigator.tab().current().mode;
                let width = self.layout_width.max(20);
                self.doc = match mode {
                    ViewMode::Raw => PageDoc::Raw(RawDoc::from_source_ctx(
                        &src,
                        page,
                        Some(key),
                        Some(index),
                        Some(&self.provider),
                    )),
                    ViewMode::Rendered => {
                        PageDoc::Rendered(RenderedViewerDoc::build(&src, page, key, index, width))
                    }
                };
                self.cursor_line = 0;
                self.scroll = 0;
                self.focused_item = None;
                if matches!(mode, ViewMode::Raw) {
                    self.spawn_highlight(src);
                } else {
                    self.cancel_highlight();
                }
            }
            Err(err) => {
                self.message = format!("read failed: {err}");
            }
        }
    }

    /// Re-lay out the rendered page when the viewer text width changes.
    pub(crate) fn ensure_layout_width(&mut self, text_width: u16) {
        let w = text_width.clamp(20, 100);
        if self.layout_width == w {
            return;
        }
        self.layout_width = w;
        if matches!(self.navigator.tab().current().mode, ViewMode::Rendered) {
            let key = self.navigator.tab().current().page.clone();
            let source = self.doc.source_cursor(self.cursor_line);
            let source_scroll = self.doc.source_cursor(self.scroll);
            self.reload_page_keeping_view(&key, source, source_scroll);
        }
    }

    fn reload_page_keeping_view(&mut self, key: &PageKey, source_cursor: u32, source_scroll: u32) {
        match self.provider.read(key) {
            Ok(src) => {
                let index = self.navigator.index();
                let page = index.pages.get(key);
                let mode = self.navigator.tab().current().mode;
                let width = self.layout_width.max(20);
                self.doc = match mode {
                    ViewMode::Raw => PageDoc::Raw(RawDoc::from_source_ctx(
                        &src,
                        page,
                        Some(key),
                        Some(index),
                        Some(&self.provider),
                    )),
                    ViewMode::Rendered => {
                        PageDoc::Rendered(RenderedViewerDoc::build(&src, page, key, index, width))
                    }
                };
                self.cursor_line = self.doc.display_cursor(source_cursor);
                self.scroll = self.doc.display_cursor(source_scroll);
                let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
                self.cursor_line = self.cursor_line.min(max);
                self.scroll = self.scroll.min(max);
                self.remap_search_matches();
                self.ensure_cursor_visible();
                if matches!(mode, ViewMode::Raw) {
                    self.spawn_highlight(src);
                } else {
                    self.cancel_highlight();
                }
            }
            Err(err) => {
                self.message = format!("read failed: {err}");
            }
        }
    }

    pub(crate) fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.doc.anchor_line(slug)
    }

    /// Sync side-nav visibility with terminal width (called each frame from draw).
    pub(crate) fn sync_nav_for_width(&mut self, width: u16) {
        self.term_width = width;
        let wide = width >= 80;
        match self.nav_width_regime {
            None => {
                self.nav_visible = wide;
                self.nav_width_regime = Some(wide);
            }
            Some(prev) if prev != wide => {
                self.nav_user_override = false;
                self.nav_visible = wide;
                self.nav_width_regime = Some(wide);
            }
            Some(_) if !self.nav_user_override => self.nav_visible = wide,
            Some(_) => {}
        }
    }
}
