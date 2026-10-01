//! App shell: state, update, render, event loop.

use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Instant;

use wiki_reader_core::Index;
use wiki_reader_core::nav::{Disposition, Effect, NavStop, Navigator, NodeId, Target, ViewState};
use wiki_reader_core::provider::{CollectionProvider, FsProvider, PageKey};
use wiki_reader_render::RenderOpts;

use super::action::Action;
use super::clipboard::{ClipboardWriter, Osc52Clipboard};
use super::editor::{EditorLauncher, SystemEditor};
use super::focus::FocusPane;
use super::help_ui::HelpOverlay;
use super::hit::HitMap;
use super::keymap::{Chord, InputMode};
use super::opener::{Opener, SystemOpener};
use super::page_doc::PageDoc;
use super::rendered_doc::RenderedViewerDoc;
use super::search_ui::{SearchMode, SearchOverlay};
use super::text_col;
use super::theme::Theme;
use super::viewer_doc::{RawDoc, ViewerDoc};
use wiki_reader_core::nav::ViewMode;
use wiki_reader_core::search;

/// Columns the raw view's line-number gutter takes from the wrap width.
const RAW_GUTTER_COLS: u16 = 6;

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
    /// User-resized nav column width (session); ignored when terminal &lt;80.
    pub nav_width: Option<u16>,
    /// Dragging the nav divider.
    pub(crate) nav_dragging: bool,
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
    /// Wanted display column of the View cursor; clamped to the row when used
    /// (so moving through short rows and back keeps your column).
    pub cursor_col: u16,
    /// Mouse selection in the View.
    pub selection: Option<crate::tui::selection::Selection>,
    /// A mouse drag is in progress.
    pub(crate) selecting: bool,
    /// Last mouse cell seen during a drag; the loop re-applies it on idle ticks
    /// so holding the pointer past the pane edge keeps scrolling.
    pub(crate) drag_at: Option<(u16, u16)>,
    /// Link under the pressed mouse button; followed on release if no drag happened.
    pub(crate) pending_link: Option<u32>,
    /// Where View text starts on screen (from last draw), for mouse → cell mapping.
    pub(crate) viewer_geom: crate::tui::regions::viewer::ViewerGeom,
    /// Viewer scroll offset (lines from top).
    pub scroll: u32,
    /// Tab-cycle focused item index into `focus_list`.
    pub focused_item: Option<usize>,
    /// Sticky footer focus kind restored after page load (P2-22).
    pub(crate) sticky_footer: Option<crate::tui::viewer_doc::FocusTarget>,
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
    /// `$EDITOR` launcher (swappable in tests).
    pub(crate) editor: Box<dyn EditorLauncher>,
    /// Configured editor command (overrides env when set).
    pub(crate) config_editor: Option<String>,
    /// Key overrides: action name → chord string (e.g. `"quit" = "Q"`).
    pub(crate) key_overrides: std::collections::BTreeMap<String, String>,
    /// Search overlay (None when closed).
    pub search: Option<SearchOverlay>,
    /// Help overlay (None when closed).
    pub help: Option<HelpOverlay>,
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
    /// The phrase of the Content search that produced `search_matches`.
    search_phrase: String,
    /// Expanded block-action ids for the current page (frontmatter / tables).
    pub(crate) expanded_blocks: HashSet<u32>,
    /// Page that `expanded_blocks` belongs to (clear only on page change).
    expanded_for_page: Option<PageKey>,
    /// Clipboard writer (OSC 52; swappable in tests).
    pub(crate) clipboard: Box<dyn ClipboardWriter>,
    /// Diagram tier preference from config.
    diagram_mode: wiki_reader_core::config::DiagramMode,
    /// When false, skip session load/save (tests).
    persist_session: bool,
    /// Last successful session save (debounce).
    session_saved_at: Option<Instant>,
    /// Session dirty since last save.
    session_dirty: bool,
}

impl App {
    /// Build app from a collection root.
    ///
    /// # Errors
    ///
    /// Propagates provider/index failures. Empty collection / missing start use
    /// process exit in [`run`].
    #[cfg(test)]
    pub fn new(root: &Path) -> Result<Self, wiki_reader_core::Error> {
        Self::for_tests(root)
    }

    /// Test constructor: no real XDG config/session.
    #[cfg(test)]
    pub fn for_tests(root: &Path) -> Result<Self, wiki_reader_core::Error> {
        Self::build(root, None, false, true)
    }

    /// Build app from a collection root with an optional `--config` path.
    ///
    /// # Errors
    ///
    /// Propagates provider/index failures.
    pub fn new_with_config(
        root: &Path,
        config_path: Option<&Path>,
    ) -> Result<Self, wiki_reader_core::Error> {
        Self::build(root, config_path, true, false)
    }

    #[allow(clippy::too_many_lines)] // config + session bootstrap
    fn build(
        root: &Path,
        config_path: Option<&Path>,
        persist_session: bool,
        skip_xdg: bool,
    ) -> Result<Self, wiki_reader_core::Error> {
        let mut config = if skip_xdg {
            wiki_reader_core::config::Config::load_with_xdg(root, config_path, None)
        } else {
            wiki_reader_core::config::Config::load(root, config_path)
        };
        let (exclude_set, exclude_diags) =
            wiki_reader_core::config::build_exclude_set(&config.exclude);
        config.diagnostics.extend(exclude_diags);
        let provider = FsProvider::open_with_exclude(root, &config.exclude)?;
        let index = Index::build(&provider)?;
        let navigator = Navigator::new_with_labels(index, None, config.nav.labels)?;
        let (watcher, watch_msg) =
            match wiki_reader_core::watch::Watcher::start_with_exclude(root, exclude_set) {
                Ok(w) => (Some(w), String::new()),
                Err(err) => (None, format!("live reload off: {err}")),
            };
        let mut message = watch_msg;
        if let Some(diag) = config.status_message() {
            if message.is_empty() {
                message = diag;
            } else {
                message = format!("{message}; {diag}");
            }
        }
        let opener: Box<dyn Opener> = match config.opener.clone() {
            Some(cmd) => Box::new(crate::tui::opener::CommandOpener { command: cmd }),
            None => Box::new(SystemOpener),
        };
        let mut app = Self {
            navigator,
            provider,
            focus: FocusPane::Viewer,
            nav_visible: false,
            nav_width: None,
            nav_dragging: false,
            nav_user_override: false,
            term_width: 80,
            layout_width: 0,
            nav_width_regime: None,
            nav_scroll: 0,
            doc: PageDoc::Raw(RawDoc::from_source("", None)),
            cursor_line: 0,
            scroll: 0,
            focused_item: None,
            sticky_footer: None,
            message,
            // ponytail: defaults until first draw; layout overwrites each frame
            viewer_rows: 20,
            cursor_col: 0,
            selection: None,
            selecting: false,
            drag_at: None,
            pending_link: None,
            viewer_geom: crate::tui::regions::viewer::ViewerGeom::default(),
            nav_viewport: 20,
            hit_map: HitMap::default(),
            theme: Theme::default(),
            quit: false,
            chord: Chord::None,
            input_mode: InputMode::Normal,
            pending_external: None,
            opener,
            editor: Box::new(SystemEditor),
            config_editor: config.editor.clone(),
            key_overrides: config.keys.clone(),
            search: None,
            help: None,
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
            search_phrase: String::new(),
            expanded_blocks: HashSet::new(),
            expanded_for_page: None,
            clipboard: Box::new(Osc52Clipboard),
            diagram_mode: config.diagrams,
            persist_session,
            session_saved_at: None,
            session_dirty: false,
        };
        if persist_session
            && let Some(loaded) = wiki_reader_core::session::load(app.provider.root())
        {
            if let Some(n) = loaded.notice {
                if app.message.is_empty() {
                    app.message = n;
                } else {
                    app.message = format!("{}; {n}", app.message);
                }
            }
            let cid = app.navigator.index().collection_id.clone();
            if let Some(n) = loaded.state.apply_to(&mut app.navigator, &cid)
                && app.message.is_empty()
            {
                app.message = n;
            }
            app.focus = match loaded.state.focus {
                wiki_reader_core::session::FocusPaneState::Nav => FocusPane::Nav,
                wiki_reader_core::session::FocusPaneState::Viewer => FocusPane::Viewer,
            };
            if let Some(v) = loaded.state.nav_visible {
                app.nav_visible = v;
                app.nav_user_override = true;
            }
            app.nav_width = loaded.state.nav_width;
        }
        let page = app.navigator.tab().current().page.clone();
        app.apply_effects(vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(None),
        ]);
        Ok(app)
    }

    /// Persist session for the next launch.
    pub(crate) fn save_session(&mut self) {
        if !self.persist_session {
            return;
        }
        let focus = match self.focus {
            FocusPane::Nav => wiki_reader_core::session::FocusPaneState::Nav,
            FocusPane::Viewer => wiki_reader_core::session::FocusPaneState::Viewer,
        };
        let state = wiki_reader_core::session::SessionState::from_navigator(
            &self.navigator,
            focus,
            Some(self.nav_visible),
            self.nav_width,
        );
        if wiki_reader_core::session::save(self.provider.root(), &state).is_ok() {
            self.session_saved_at = Some(Instant::now());
            self.session_dirty = false;
        }
    }

    /// Mark session dirty; save at most every ~2s.
    pub(crate) fn note_session_change(&mut self) {
        if !self.persist_session {
            return;
        }
        self.session_dirty = true;
        let due = self
            .session_saved_at
            .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(2));
        if due {
            self.save_session();
        }
    }

    /// Flush a pending save (`force` ignores the 2s debounce).
    pub(crate) fn flush_session(&mut self, force: bool) {
        if !self.session_dirty {
            return;
        }
        let due = force
            || self
                .session_saved_at
                .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(2));
        if due {
            self.save_session();
        }
    }

    fn copy_page_path(&mut self) {
        let path = self
            .navigator
            .tab()
            .current()
            .page
            .relative_path
            .to_string_lossy()
            .into_owned();
        match self.clipboard.copy(&path) {
            Ok(()) => self.message = "sent to clipboard (OSC 52)".into(),
            Err(err) => self.message = format!("copy failed: {err}"),
        }
    }

    fn copy_link_target(&mut self) {
        let items = self.focus_list();
        let Some(i) = self.focused_item else {
            self.message = "no focused link".into();
            return;
        };
        let Some(item) = items.get(i) else {
            self.message = "no focused link".into();
            return;
        };
        if item.kind != crate::tui::viewer_doc::FocusTarget::Link {
            self.message = "no focused link".into();
            return;
        }
        match self.clipboard.copy(&item.target) {
            Ok(()) => self.message = "sent to clipboard (OSC 52)".into(),
            Err(err) => self.message = format!("copy failed: {err}"),
        }
    }

    fn render_opts(&self) -> RenderOpts {
        RenderOpts {
            expanded: self.expanded_blocks.clone(),
            diagram_mode: self.diagram_mode,
        }
    }

    pub(crate) fn view_state(&self) -> ViewState {
        ViewState {
            cursor_line: self.doc.source_cursor(self.cursor_line),
            scroll: self.doc.source_cursor(self.scroll),
        }
    }

    /// Pure state update (unit-testable without a terminal).
    #[allow(clippy::too_many_lines)] // action match grows with tabs/copy
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
                | Action::ViewerHeadingUp
                | Action::ViewerHeadingDown
                | Action::ViewerPageUp
                | Action::ViewerPageDown
                | Action::ViewerHome
                | Action::ViewerEnd
                | Action::SetCursorLine(_)
                | Action::SelectStart(..)
                | Action::ViewerLeft
                | Action::ViewerRight
                | Action::GoToPage(_)
                | Action::Back
                | Action::Forward
                | Action::PrevPage
                | Action::NextPage
                | Action::ViewerTab
                | Action::ViewerBackTab
                | Action::FollowLinkId(_)
                | Action::ActivateBlock(_)
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
            Action::OpenHelp => self.open_help(),
            Action::CloseHelp => self.close_help(),
            Action::HelpSelectDelta(d) => self.help_select(d),
            Action::HelpPageDelta(d) => self.help_page(d),
            Action::HelpHome => self.help_jump(true),
            Action::HelpEnd => self.help_jump(false),
            Action::HelpActivate => self.help_activate(None),
            Action::HelpScroll(d) => {
                if let Some(help) = self.help.as_mut() {
                    help.scroll_by(d);
                }
            }
            Action::SearchChar(c) => self.search_type(c),
            Action::SearchBackspace => self.search_backspace(),
            Action::SearchSelectDelta(d) => self.search_select(d),
            Action::SearchJump(home) => self.search_jump(home),
            Action::SearchPageDelta(d) => self.search_page(d),
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
                self.clear_item_focus();
            }
            Action::ViewerLeft => self.viewer_move_col(-1),
            Action::ViewerRight => self.viewer_move_col(1),
            Action::SelectStart(line, col) => self.select_start(line, col),
            Action::SelectExtend(line, col) => self.select_extend(line, col),
            Action::SelectEnd => self.select_end(),
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
            Action::ViewerHeadingUp => self.viewer_heading(-1),
            Action::ViewerHeadingDown => self.viewer_heading(1),
            Action::ViewerPageUp => {
                let step = i32::from(self.viewer_rows.saturating_sub(1).max(1));
                self.viewer_move_line(-step);
            }
            Action::ViewerPageDown => {
                let step = i32::from(self.viewer_rows.saturating_sub(1).max(1));
                self.viewer_move_line(step);
            }
            Action::ViewerHome => {
                self.clear_item_focus();
                self.cursor_line = 0;
                self.ensure_cursor_visible();
            }
            Action::ViewerEnd => {
                self.clear_item_focus();
                let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
                self.cursor_line = max;
                self.ensure_cursor_visible();
            }
            Action::ViewerTab => self.viewer_tab(false),
            Action::ViewerBackTab => self.viewer_tab(true),
            Action::ViewerActivate => self.viewer_activate(),
            Action::FocusFooter => self.focus_footer(),
            Action::FollowLinkId(id) => self.follow_link_id(id),
            Action::ActivateBlock(id) => self.activate_block(&format!("block:{id}")),
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
            Action::OpenInEditor => self.open_in_editor(),
            Action::CopyPagePath => self.copy_page_path(),
            Action::CopyLinkTarget => self.copy_link_target(),
            Action::NewTab => self.open_new_tab(),
            Action::NewTabFocusView => self.open_new_tab_focus_view(),
            Action::NextTab => self.cycle_tab(1),
            Action::PrevTab => self.cycle_tab(-1),
            Action::CloseTab => self.close_active_tab(),
            Action::SwitchTab(i) => self.switch_to_tab(i),
            Action::None => {}
        }
    }

    fn open_new_tab(&mut self) {
        match self.focus {
            FocusPane::Nav => match self.navigator.nav().cursor.clone() {
                NavStop::Node(NodeId::Page(key)) => self.open_page_new_tab(key),
                NavStop::Node(id) => {
                    let open = !self.navigator.nav().expanded.contains(&id);
                    self.navigator.set_group_expanded(id, open);
                    self.clamp_nav_scroll();
                }
                NavStop::Search => {}
            },
            FocusPane::Viewer => {
                if let Some(raw) = self.new_tab_link_target() {
                    self.open_raw_new_tab(&raw);
                } else {
                    self.duplicate_current_tab();
                }
            }
        }
    }

    /// `Ctrl+→` in the nav: open the page in a new tab and hand focus to its View.
    /// Anything else behaves like [`Self::open_new_tab`].
    fn open_new_tab_focus_view(&mut self) {
        if self.focus == FocusPane::Nav
            && let NavStop::Node(NodeId::Page(key)) = self.navigator.nav().cursor.clone()
        {
            self.open_page_new_tab(key);
            self.navigator.nav_focus_lost();
            self.focus = FocusPane::Viewer;
        } else {
            self.open_new_tab();
        }
    }

    /// Focused link, or the sole link on the cursor line.
    fn new_tab_link_target(&self) -> Option<String> {
        let items = self.focus_list();
        if let Some(i) = self.focused_item
            && let Some(it) = items.get(i)
            && it.kind == crate::tui::viewer_doc::FocusTarget::Link
        {
            return Some(it.target.clone());
        }
        let line = self.cursor_line;
        let on_line: Vec<_> = self
            .doc
            .link_spans()
            .iter()
            .filter(|s| s.segments.iter().any(|(l, _)| *l == line))
            .collect();
        (on_line.len() == 1).then(|| on_line[0].raw_target.clone())
    }

    fn duplicate_current_tab(&mut self) {
        let loc = self.navigator.tab().current();
        let page = loc.page.clone();
        let anchor = loc.anchor.clone();
        self.open_target_new_tab(Target::Page(page, anchor));
    }

    pub(crate) fn open_page_new_tab(&mut self, key: PageKey) {
        self.open_target_new_tab(Target::Page(key, None));
    }

    fn open_raw_new_tab(&mut self, raw: &str) {
        let from = self.navigator.tab().current().page.clone();
        let outcome = wiki_reader_core::nav::resolve(raw, &from, self.navigator.index());
        self.open_target_new_tab(outcome.target);
    }

    fn open_target_new_tab(&mut self, target: Target) {
        let effects = self
            .navigator
            .navigate(target, Disposition::NewTab, self.view_state());
        self.apply_effects(effects);
    }

    /// Middle-click: open navigable hits in a new tab (same targets as left-click).
    pub(crate) fn middle_click_hit(&mut self, hit: crate::tui::hit::Hit) {
        use crate::tui::hit::Hit;
        match hit {
            Hit::Link(id) => {
                self.update(Action::FocusViewer);
                if let Some(raw) = self
                    .doc
                    .link_target(wiki_reader_render::LinkId(id))
                    .map(str::to_owned)
                {
                    self.open_raw_new_tab(&raw);
                }
            }
            Hit::NavItem(NodeId::Page(key)) => {
                self.update(Action::FocusNav);
                self.navigator.set_nav_cursor(NodeId::Page(key.clone()));
                self.open_page_new_tab(key);
            }
            Hit::Breadcrumb(key) => self.open_page_new_tab(key),
            Hit::Prev => {
                let cur = self.navigator.tab().current().page.clone();
                if let Some(key) = self.navigator.nav().tree.prev(&cur) {
                    self.open_page_new_tab(key);
                }
            }
            Hit::Next => {
                let cur = self.navigator.tab().current().page.clone();
                if let Some(key) = self.navigator.nav().tree.next(&cur) {
                    self.open_page_new_tab(key);
                }
            }
            Hit::SearchResult(i) => {
                let Some(overlay) = self.search.as_ref() else {
                    return;
                };
                let key = match overlay.mode {
                    SearchMode::Files => overlay.page_hits.get(i).map(|h| h.page.clone()),
                    SearchMode::Content => overlay.text_hits.get(i).map(|h| h.page.clone()),
                };
                if let Some(key) = key {
                    self.open_page_new_tab(key);
                }
            }
            Hit::NavItem(_)
            | Hit::NavGroupToggle(_)
            | Hit::NavSearchRow
            | Hit::NavToggle
            | Hit::Quit
            | Hit::Block(_)
            | Hit::ViewerLine(_)
            | Hit::SearchDismiss
            | Hit::HelpDismiss
            | Hit::HelpRow(_)
            | Hit::FocusNav
            | Hit::FocusViewer
            | Hit::Tab(_)
            | Hit::TabClose(_)
            | Hit::NavDivider => {}
        }
    }

    fn cycle_tab(&mut self, delta: i32) {
        let n = self.navigator.tab_count();
        if n < 2 {
            return;
        }
        let cur = i32::try_from(self.navigator.active()).unwrap_or(0);
        let next = (cur + delta).rem_euclid(i32::try_from(n).unwrap_or(1));
        let i = usize::try_from(next).unwrap_or(0);
        self.switch_to_tab(i);
    }

    fn switch_to_tab(&mut self, i: usize) {
        if i == self.navigator.active() {
            return;
        }
        self.navigator.save_view(self.view_state());
        if !self.navigator.switch_tab(i) {
            return;
        }
        let page = self.navigator.tab().current().page.clone();
        self.apply_effects(vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(None),
        ]);
    }

    pub(crate) fn close_tab_at(&mut self, i: usize) {
        self.navigator.save_view(self.view_state());
        if let Err(msg) = self.navigator.close_tab(i) {
            self.message = msg.into();
            return;
        }
        let page = self.navigator.tab().current().page.clone();
        self.apply_effects(vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(None),
        ]);
    }

    fn close_active_tab(&mut self) {
        let i = self.navigator.active();
        self.close_tab_at(i);
    }

    fn open_search(&mut self) {
        self.help = None;
        self.navigator.set_nav_stop(NavStop::Search);
        self.search = Some(SearchOverlay {
            query: String::new(),
            mode: SearchMode::Files,
            selected: 0,
            scroll: 0,
            list_height: 10,
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

    fn open_help(&mut self) {
        if self.search.is_some() {
            self.close_search(false);
        }
        self.help = Some(HelpOverlay::new(&self.key_overrides));
        self.input_mode = InputMode::Help;
        self.message.clear();
    }

    fn close_help(&mut self) {
        self.help = None;
        if self.search.is_none() {
            self.input_mode = InputMode::Normal;
        }
        self.message.clear();
    }

    fn help_select(&mut self, delta: i32) {
        let Some(help) = self.help.as_mut() else {
            return;
        };
        help.select_delta(delta);
    }

    fn help_page(&mut self, dir: i32) {
        let step =
            i32::try_from(self.help.as_ref().map_or(10, |h| h.list_height.max(1))).unwrap_or(10);
        self.help_select(dir * step);
    }

    fn help_jump(&mut self, home: bool) {
        let Some(help) = self.help.as_mut() else {
            return;
        };
        help.select_edge(home);
    }

    pub(crate) fn help_activate(&mut self, index: Option<usize>) {
        let Some(help) = self.help.as_ref() else {
            return;
        };
        let i = index.unwrap_or(help.selected);
        let Some(row) = help.rows.get(i) else {
            return;
        };
        let Some(action) = row.action.clone() else {
            return;
        };
        // Don't re-open help from a help row.
        if matches!(action, Action::OpenHelp) {
            return;
        }
        self.close_help();
        self.update(action);
    }

    fn search_refresh(&mut self) {
        let Some(overlay) = self.search.as_mut() else {
            return;
        };
        let index = self.navigator.index();
        match overlay.mode {
            SearchMode::Files => {
                overlay.page_hits = search::search_pages(&overlay.query, index);
                overlay.text_hits.clear();
            }
            SearchMode::Content => {
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
        overlay.selected = usize::try_from(next).unwrap_or(0);
        overlay.ensure_selection_visible();
    }

    fn search_jump(&mut self, home: bool) {
        let Some(overlay) = self.search.as_mut() else {
            return;
        };
        if overlay.result_len() == 0 {
            return;
        }
        overlay.selected = if home {
            0
        } else {
            overlay.result_len().saturating_sub(1)
        };
        overlay.ensure_selection_visible();
    }

    fn search_page(&mut self, dir: i32) {
        let step =
            i32::try_from(self.search.as_ref().map_or(10, |o| o.list_height.max(1))).unwrap_or(10);
        self.search_select(dir * step);
    }

    fn search_toggle_mode(&mut self) {
        if let Some(overlay) = self.search.as_mut() {
            overlay.mode = match overlay.mode {
                SearchMode::Files => SearchMode::Content,
                SearchMode::Content => SearchMode::Files,
            };
            overlay.selected = 0;
            overlay.scroll = 0;
        }
        self.search_refresh();
    }

    fn clear_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_page = None;
        self.search_match_idx = 0;
        self.match_highlight = None;
        self.search_phrase.clear();
    }

    /// Display-column spans of the active Content-search phrase. A phrase may
    /// cross soft-wrapped rows; virtual boundary spaces have no painted span.
    pub(crate) fn match_spans(&self) -> Vec<(u32, u16, u16)> {
        type MappedGlyph = (char, Option<(u32, u16, u16)>);

        let Some(&source) = self.search_matches.get(self.search_match_idx) else {
            return Vec::new();
        };
        if self.search_phrase.is_empty() {
            return Vec::new();
        }

        // (case-folded glyph, display location); `None` is a virtual space
        // between rendered rows, whose wrapping removes source whitespace.
        let mut flat: Vec<MappedGlyph> = Vec::new();
        for (line, row) in self.doc.lines().iter().enumerate() {
            let line = u32::try_from(line).unwrap_or(u32::MAX);
            if self.doc.source_cursor(line) != source {
                continue;
            }
            if !flat.is_empty()
                && !flat.last().is_some_and(|(ch, _)| ch.is_whitespace())
                && !row.starts_with(char::is_whitespace)
            {
                flat.push((' ', None));
            }
            let mut col = 0u16;
            for ch in row.chars() {
                let folded: Vec<char> = ch.to_lowercase().collect();
                if folded.len() != 1 {
                    return Vec::new();
                }
                let width = text_col::char_width(ch);
                flat.push((folded[0], Some((line, col, width))));
                col = col.saturating_add(width);
            }
        }
        let want: Vec<char> = self.search_phrase.to_lowercase().chars().collect();
        if want.is_empty() || want.len() > flat.len() {
            return Vec::new();
        }
        let Some(at) = (0..=flat.len() - want.len()).find(|&i| {
            flat[i..i + want.len()]
                .iter()
                .map(|p| p.0)
                .eq(want.iter().copied())
        }) else {
            return Vec::new();
        };

        let mut spans: Vec<(u32, u16, u16)> = Vec::new();
        for &(_, pos) in &flat[at..at + want.len()] {
            let Some((line, col, width)) = pos else {
                continue;
            };
            if let Some((last_line, _, last_end)) = spans.last_mut()
                && *last_line == line
                && *last_end == col
            {
                *last_end = col.saturating_add(width);
            } else {
                spans.push((line, col, col.saturating_add(width)));
            }
        }
        spans
    }

    /// First painted span of the active Content-search phrase.
    pub(crate) fn match_span(&self) -> Option<(u32, u16, u16)> {
        self.match_spans().into_iter().next()
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
        let first = self.match_span();
        self.match_highlight = Some(first.map_or(display, |(line, _, _)| line));
        self.cursor_line = first.map_or(display, |(line, _, _)| line);
        self.cursor_col = first.map_or(0, |(_, c0, _)| c0);
        // Keep the page where it is; scroll (centred) only if the match is off-screen.
        let rows = u32::from(self.viewer_rows.max(1));
        let target = self.cursor_line;
        if target < self.scroll || target >= self.scroll.saturating_add(rows) {
            let last = u32::try_from(self.doc.lines().len()).unwrap_or(u32::MAX);
            self.scroll = target
                .saturating_sub(rows / 2)
                .min(last.saturating_sub(rows));
        }
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
            let display = self.doc.display_cursor(src);
            self.match_highlight = Some(self.match_span().map_or(display, |(line, _, _)| line));
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
            SearchMode::Files => {
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
            SearchMode::Content => {
                let Some(hit) = overlay.text_hits.get(selected) else {
                    return;
                };
                let key = hit.page.clone();
                let phrase = overlay.query.trim().to_owned();
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
                self.search_phrase = phrase;
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

    /// Open the current page in `$VISUAL`/`$EDITOR` at the cursor's source line.
    ///
    /// Callers that own the terminal must suspend/restore around this (see
    /// `events`); tests inject a recording launcher and call
    /// [`Self::open_in_editor_with`].
    pub(crate) fn open_in_editor(&mut self) {
        self.open_in_editor_with(crate::tui::editor::resolve_editor_with_config(
            self.config_editor.as_deref(),
        ));
    }

    pub(crate) fn open_in_editor_with(&mut self, editor: Option<String>) {
        let Some(editor) = editor else {
            self.message = "no $VISUAL or $EDITOR set".into();
            return;
        };
        let key = self.navigator.tab().current().page.clone();
        let path = crate::tui::editor::page_abs_path(self.provider.root(), &key.relative_path);
        let source = self.doc.source_cursor(self.cursor_line);
        let source_scroll = self.doc.source_cursor(self.scroll);
        let line_1based = source.saturating_add(1);
        let cmd = crate::tui::editor::build_editor_command(&editor, &path, line_1based);
        let meta_before = std::fs::metadata(&path).ok();
        match self.editor.launch(&cmd) {
            Ok(exit) => {
                let meta_after = std::fs::metadata(&path).ok();
                let saved = match (&meta_before, &meta_after) {
                    (Some(b), Some(a)) => {
                        let mtime_changed = match (b.modified(), a.modified()) {
                            (Ok(t0), Ok(t1)) => t0 != t1,
                            _ => false,
                        };
                        mtime_changed || b.len() != a.len()
                    }
                    _ => false,
                };
                if saved {
                    // Index may lag the watcher; force a fresh parse for this paint and
                    // kick a rebuild so backlinks/nav catch up.
                    self.note_watcher_dirty(true);
                    self.reload_page_keeping_view_ex(&key, source, source_scroll, true);
                }
                if !exit.success {
                    self.message = if saved {
                        "editor exited non-zero (file saved)".into()
                    } else {
                        "editor exited non-zero".into()
                    };
                }
            }
            Err(err) => self.message = format!("editor failed: {err}"),
        }
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
        // ponytail: spawn-per-request + token; mailbox worker if rapid nav piles threads
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
                    if self.expanded_for_page.as_ref() != Some(&key) {
                        self.expanded_blocks.clear();
                        self.expanded_for_page = Some(key.clone());
                    }
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
                    self.clear_item_focus();
                }
            }
        }
        if let Some(n) = self.navigator.notice() {
            self.message = n.to_owned();
        }
        // The nav highlight follows the current page however we got there (links,
        // footer, search, history), and stays put while the nav is focused.
        if page_changed || self.focus == FocusPane::Nav {
            let page = self.navigator.tab().current().page.clone();
            self.navigator.set_nav_cursor(NodeId::Page(page.clone()));
            self.reveal_page_in_nav(&page);
        }
        if page_changed && self.term_width < 80 {
            self.nav_visible = false;
        }
        if page_changed {
            self.note_session_change();
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
                    ViewMode::Raw => PageDoc::Raw(
                        RawDoc::from_source_ctx(
                            &src,
                            page,
                            Some(key),
                            Some(index),
                            Some(&self.provider),
                        )
                        .wrapped(width.saturating_sub(RAW_GUTTER_COLS)),
                    ),
                    ViewMode::Rendered => {
                        let opts = self.render_opts();
                        PageDoc::Rendered(RenderedViewerDoc::build_with(
                            &src, page, key, index, width, &opts,
                        ))
                    }
                };
                self.cursor_line = 0;
                self.cursor_col = 0;
                self.scroll = 0;
                self.focused_item = None;
                self.selection = None;
                self.restore_sticky_footer();
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
        // Both views lay out to the pane width (raw soft-wraps source lines).
        let key = self.navigator.tab().current().page.clone();
        let source = self.doc.source_cursor(self.cursor_line);
        let source_scroll = self.doc.source_cursor(self.scroll);
        // The selection is stored in display coordinates, so a re-wrap invalidates it.
        self.selection = None;
        self.selecting = false;
        if let PageDoc::Raw(doc) = &mut self.doc {
            // Re-wrap in place: no disk read, and the syntax colours stay.
            doc.rewrap(w.saturating_sub(RAW_GUTTER_COLS));
            self.restore_view_after_relayout(source, source_scroll);
        } else {
            self.reload_page_keeping_view(&key, source, source_scroll);
        }
    }

    /// Put the cursor and scroll back on their source lines after the doc was re-laid out.
    fn restore_view_after_relayout(&mut self, source_cursor: u32, source_scroll: u32) {
        self.cursor_line = self.doc.display_cursor(source_cursor);
        self.scroll = self.doc.display_cursor(source_scroll);
        let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        self.cursor_line = self.cursor_line.min(max);
        self.scroll = self.scroll.min(max);
        self.remap_search_matches();
        self.ensure_cursor_visible();
    }

    fn reload_page_keeping_view(&mut self, key: &PageKey, source_cursor: u32, source_scroll: u32) {
        self.reload_page_keeping_view_ex(key, source_cursor, source_scroll, false);
    }

    /// Reload the current page, optionally ignoring the indexed `Page` so parse
    /// data matches the fresh file (post-editor) before async reindex finishes.
    fn reload_page_keeping_view_ex(
        &mut self,
        key: &PageKey,
        source_cursor: u32,
        source_scroll: u32,
        fresh_parse: bool,
    ) {
        match self.provider.read(key) {
            Ok(src) => {
                let index = self.navigator.index();
                let page = if fresh_parse {
                    None
                } else {
                    index.pages.get(key)
                };
                let mode = self.navigator.tab().current().mode;
                let width = self.layout_width.max(20);
                self.doc = match mode {
                    ViewMode::Raw => PageDoc::Raw(
                        RawDoc::from_source_ctx(
                            &src,
                            page,
                            Some(key),
                            Some(index),
                            Some(&self.provider),
                        )
                        .wrapped(width.saturating_sub(RAW_GUTTER_COLS)),
                    ),
                    ViewMode::Rendered => {
                        let opts = self.render_opts();
                        PageDoc::Rendered(RenderedViewerDoc::build_with(
                            &src, page, key, index, width, &opts,
                        ))
                    }
                };
                self.selection = None;
                self.selecting = false;
                self.restore_view_after_relayout(source_cursor, source_scroll);
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

    /// Set preferred nav width from a drag column (no-op when terminal &lt;80).
    pub(crate) fn resize_nav_to_column(&mut self, column: u16) {
        if self.term_width < 80 {
            return;
        }
        let cap = crate::tui::layout::NAV_WIDTH_MAX.min(self.term_width.saturating_sub(20));
        let w = column.saturating_add(1).clamp(
            crate::tui::layout::NAV_WIDTH_MIN,
            cap.max(crate::tui::layout::NAV_WIDTH_MIN),
        );
        self.nav_width = Some(w);
    }
}
