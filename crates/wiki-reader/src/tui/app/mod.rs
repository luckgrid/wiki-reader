//! App shell: state, update, render, event loop.

use std::path::Path;

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
use super::theme::Theme;
use super::viewer_doc::{RawDoc, ViewerDoc};
use wiki_reader_core::nav::ViewMode;

mod draw;
mod events;
mod nav_ui;
mod viewer_state;

#[cfg(test)]
mod tests;

pub use events::run;

/// Owned application state.
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
            message: String::new(),
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
            cursor_line: self.cursor_line,
            scroll: self.scroll,
        }
    }

    /// Pure state update (unit-testable without a terminal).
    #[allow(clippy::too_many_lines)] // split in P1-R10
    pub fn update(&mut self, action: Action) {
        // Transient notices clear on the next key/action (Tab keeps focus target).
        if !matches!(
            action,
            Action::None | Action::ViewerTab | Action::ViewerBackTab
        ) {
            self.message.clear();
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
            Action::OpenSearch => {
                self.navigator.set_nav_stop(NavStop::Search);
                self.message = "search: coming in P1-10".into();
            }
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

    fn toggle_view_mode(&mut self) {
        let key = self.navigator.tab().current().page.clone();
        let new_mode = match self.navigator.tab().current().mode {
            ViewMode::Rendered => ViewMode::Raw,
            ViewMode::Raw => ViewMode::Rendered,
        };
        self.navigator.set_view_mode(new_mode);
        self.load_page(&key);
    }

    pub(crate) fn apply_effects(&mut self, effects: Vec<Effect>) {
        let page_changed = effects.iter().any(|e| matches!(e, Effect::LoadPage(_)));
        for effect in effects {
            match effect {
                Effect::LoadPage(key) => self.load_page(&key),
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
                        self.cursor_line = loc.cursor_line;
                        self.scroll = loc.scroll;
                    }
                }
                Effect::Notice(msg) => self.message = msg,
                Effect::ConfirmExternal(url) => {
                    self.pending_external = Some(url.clone());
                    self.input_mode = InputMode::Confirm;
                    self.message = format!("open {url}? [y/N]");
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
            let cursor = self.cursor_line;
            let scroll = self.scroll;
            self.reload_page_keeping_view(&key, cursor, scroll);
        }
    }

    fn reload_page_keeping_view(&mut self, key: &PageKey, cursor: u32, scroll: u32) {
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
                let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
                self.cursor_line = cursor.min(max);
                self.scroll = scroll.min(max);
                self.ensure_cursor_visible();
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
