//! App shell: state, update, render, event loop.

use std::io::{self, stdout};
use std::panic;
use std::path::Path;

use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use wiki_reader_core::Index;
use wiki_reader_core::nav::{Effect, Navigator, NodeId, ViewState};
use wiki_reader_core::provider::{CollectionProvider, FsProvider, PageKey};

use super::action::Action;
use super::hit::{Hit, HitMap};
use super::keymap;
use super::layout;
use super::regions::status::{FocusPane, StatusModel};
use super::regions::{footer, header, side_nav, status, viewer};
use super::theme::Theme;
use super::viewer_doc::{FocusItem, RawDoc, ViewerDoc, cycle, format_target};

/// Owned application state.
pub struct App {
    /// Navigation session.
    pub navigator: Navigator,
    /// Collection provider.
    pub provider: FsProvider,
    /// Focused pane.
    pub focus: FocusPane,
    /// Side nav visible (docked ≥80 cols, overlay &lt;80). `None` until first draw seeds from width.
    pub nav_visible: Option<bool>,
    /// Side nav scroll offset (rows below the search line).
    pub nav_scroll: u16,
    /// When true, the ⌕ Search… row is the nav cursor (not a `NodeId`).
    pub nav_on_search: bool,
    /// Current viewer document.
    pub doc: RawDoc,
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
            // Seeded from terminal width on first draw (≥80 shown, &lt;80 hidden).
            nav_visible: None,
            nav_scroll: 0,
            nav_on_search: false,
            doc: RawDoc::from_source("", None),
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
        };
        let page = app.navigator.tab().current().page.clone();
        app.apply_effects(vec![
            Effect::LoadPage(page.clone()),
            Effect::RevealInTree(page),
            Effect::ScrollTo(None),
        ]);
        Ok(app)
    }

    fn view_state(&self) -> ViewState {
        ViewState {
            cursor_line: self.cursor_line,
            scroll: self.scroll,
        }
    }

    /// Pure state update (unit-testable without a terminal).
    pub fn update(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::ToggleNav => {
                let cur = self.nav_visible.unwrap_or(false);
                self.nav_visible = Some(!cur);
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
            }
            Action::OpenSearch => {
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
                let max =
                    u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(u32::MAX);
                self.cursor_line = line.min(max);
                self.focused_item = None;
                self.focus = FocusPane::Viewer;
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
            Action::None => {}
        }
    }

    fn nav_rows(&self) -> Vec<side_nav::NavRow> {
        side_nav::visible_rows(&self.navigator.nav().tree, &self.navigator.nav().expanded)
    }

    fn nav_cursor_index(&self, rows: &[side_nav::NavRow]) -> Option<usize> {
        if self.nav_on_search {
            return None; // search is before rows
        }
        let cur = self.navigator.nav().cursor.as_ref()?;
        rows.iter().position(|r| &r.id == cur)
    }

    fn nav_step(&mut self, dir: i32) {
        let rows = self.nav_rows();
        // Positions: 0 = search, 1..len = rows[0..]
        let len = rows.len() + 1;
        let cur = if self.nav_on_search {
            0usize
        } else {
            self.nav_cursor_index(&rows).map_or(1, |i| i + 1)
        };
        let next = if dir < 0 {
            cur.saturating_sub(1)
        } else {
            (cur + 1).min(len.saturating_sub(1))
        };
        if next == 0 {
            self.nav_on_search = true;
        } else if let Some(row) = rows.get(next - 1) {
            self.nav_on_search = false;
            self.navigator.set_nav_cursor(row.id.clone());
        }
        self.ensure_nav_cursor_visible();
    }

    fn nav_jump_group(&mut self, dir: i32) {
        let rows = self.nav_rows();
        let cur = if self.nav_on_search {
            0usize
        } else {
            self.nav_cursor_index(&rows).map_or(1, |i| i + 1)
        };
        if dir < 0 {
            if cur <= 1 {
                self.nav_on_search = true;
            } else {
                let mut found = false;
                for pos in (1..cur).rev() {
                    if rows[pos - 1].is_group {
                        self.nav_on_search = false;
                        self.navigator.set_nav_cursor(rows[pos - 1].id.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    self.nav_on_search = true;
                }
            }
        } else {
            for (idx, row) in rows.iter().enumerate() {
                let pos = idx + 1;
                if pos <= cur {
                    continue;
                }
                if row.is_group {
                    self.nav_on_search = false;
                    self.navigator.set_nav_cursor(row.id.clone());
                    break;
                }
            }
        }
        self.ensure_nav_cursor_visible();
    }

    fn nav_expand(&mut self) {
        if self.nav_on_search {
            return;
        }
        let Some(id) = self.navigator.nav().cursor.clone() else {
            return;
        };
        if matches!(id, NodeId::Group(_) | NodeId::OtherPages) {
            self.navigator.set_group_expanded(id, true);
        }
    }

    fn nav_collapse(&mut self) {
        if self.nav_on_search {
            return;
        }
        let Some(id) = self.navigator.nav().cursor.clone() else {
            return;
        };
        match &id {
            NodeId::Group(_) | NodeId::OtherPages
                if self.navigator.nav().expanded.contains(&id) =>
            {
                self.navigator.set_group_expanded(id, false);
            }
            NodeId::Page(key) => {
                if let Some(parent) = key
                    .relative_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    self.navigator
                        .set_nav_cursor(NodeId::Group(parent.to_path_buf()));
                }
            }
            _ => {}
        }
    }

    fn nav_activate(&mut self) {
        if self.nav_on_search {
            self.message = "search: coming in P1-10".into();
            return;
        }
        let Some(id) = self.navigator.nav().cursor.clone() else {
            return;
        };
        match id {
            NodeId::Page(key) => {
                self.navigator.set_nav_cursor(NodeId::Page(key.clone()));
                let effects = self.navigator.go_to_page(key, self.view_state());
                self.apply_effects(effects);
            }
            other => {
                let open = !self.navigator.nav().expanded.contains(&other);
                self.navigator.set_group_expanded(other, open);
            }
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) {
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
                    self.message = format!("open {url}? (confirm later)");
                }
            }
        }
        if let Some(n) = self.navigator.notice() {
            self.message = n.to_owned();
        }
    }

    fn load_page(&mut self, key: &PageKey) {
        match self.provider.read(key) {
            Ok(src) => {
                let page = self.navigator.index().pages.get(key);
                self.doc = RawDoc::from_source(&src, page);
                self.cursor_line = 0;
                self.scroll = 0;
                self.focused_item = None;
            }
            Err(err) => {
                self.message = format!("read failed: {err}");
            }
        }
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        self.doc.anchor_line(slug)
    }

    fn viewer_move_line(&mut self, delta: i32) {
        self.focused_item = None;
        let max = i64::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        let cur = i64::from(self.cursor_line);
        let next = (cur + i64::from(delta)).clamp(0, max);
        self.cursor_line = u32::try_from(next).unwrap_or(0);
        self.ensure_cursor_visible();
    }

    fn viewer_block(&mut self, dir: i32) {
        self.focused_item = None;
        let blocks = self.doc.block_starts();
        // blocks are 1-based; cursor is 0-based
        let cur_src = self.cursor_line.saturating_add(1);
        if dir < 0 {
            let prev = blocks.iter().rev().find(|&&b| b < cur_src).copied();
            if let Some(b) = prev {
                self.cursor_line = b.saturating_sub(1);
            } else {
                self.cursor_line = 0;
            }
        } else {
            let next = blocks.iter().find(|&&b| b > cur_src).copied();
            if let Some(b) = next {
                self.cursor_line = b.saturating_sub(1);
            }
        }
        self.ensure_cursor_visible();
    }

    fn ensure_cursor_visible(&mut self) {
        let page_h = u32::from(self.viewer_rows.max(1));
        if self.cursor_line < self.scroll {
            self.scroll = self.cursor_line;
        } else if self.cursor_line >= self.scroll.saturating_add(page_h) {
            self.scroll = self.cursor_line.saturating_sub(page_h.saturating_sub(1));
        }
        self.clamp_viewer_scroll();
    }

    fn clamp_viewer_scroll(&mut self) {
        let max = u32::try_from(self.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        self.scroll = self.scroll.min(max);
    }

    fn reveal_page_in_nav(&mut self, page: &PageKey) {
        let rows = self.nav_rows();
        if let Some(idx) = rows.iter().position(|r| r.id == NodeId::Page(page.clone())) {
            self.scroll_nav_to_index(idx);
        }
    }

    fn ensure_nav_cursor_visible(&mut self) {
        if self.nav_on_search {
            self.nav_scroll = 0;
            return;
        }
        let rows = self.nav_rows();
        if let Some(idx) = self.nav_cursor_index(&rows) {
            self.scroll_nav_to_index(idx);
        }
    }

    fn scroll_nav_to_index(&mut self, idx: usize) {
        let vh = usize::from(self.nav_viewport.max(1));
        let scroll = usize::from(self.nav_scroll);
        if idx < scroll {
            self.nav_scroll = u16::try_from(idx).unwrap_or(0);
        } else if idx >= scroll.saturating_add(vh) {
            self.nav_scroll = u16::try_from(idx.saturating_add(1).saturating_sub(vh)).unwrap_or(0);
        }
        self.clamp_nav_scroll();
    }

    fn clamp_nav_scroll(&mut self) {
        let n = self.nav_rows().len();
        let vh = usize::from(self.nav_viewport.max(1));
        let max = n.saturating_sub(vh);
        self.nav_scroll = self.nav_scroll.min(u16::try_from(max).unwrap_or(0));
    }

    fn focus_list(&self) -> Vec<FocusItem> {
        let mut items = self.doc.focus_items();
        let current = &self.navigator.tab().current().page;
        let tree = &self.navigator.nav().tree;
        if tree.prev(current).is_some() {
            items.push(FocusItem {
                line: u32::MAX - 1,
                cols: (0, 1),
                kind: super::viewer_doc::FocusKind::FooterPrev,
                target: String::new(),
            });
        }
        if tree.next(current).is_some() {
            items.push(FocusItem {
                line: u32::MAX,
                cols: (0, 1),
                kind: super::viewer_doc::FocusKind::FooterNext,
                target: String::new(),
            });
        }
        items
    }

    fn viewer_tab(&mut self, backward: bool) {
        let items = self.focus_list();
        if self.focused_item.is_none() {
            self.focused_item = cycle::next_after(&items, self.cursor_line, backward);
        } else {
            self.focused_item = cycle::step(&items, self.focused_item, backward);
        }
        let Some(i) = self.focused_item else {
            return;
        };
        let Some(it) = items.get(i) else {
            return;
        };
        if it.line < u32::MAX - 1 {
            self.cursor_line = it.line;
            self.ensure_cursor_visible();
        }
        let page = &self.navigator.tab().current().page;
        self.message = match it.kind {
            super::viewer_doc::FocusKind::Link => {
                format_target(&it.target, page, self.navigator.index())
            }
            super::viewer_doc::FocusKind::FooterPrev => "‹ prev".into(),
            super::viewer_doc::FocusKind::FooterNext => "next ›".into(),
        };
    }

    fn close_overlay_after_nav(&mut self, term_width: u16) {
        if term_width < 80 {
            self.nav_visible = Some(false);
        }
    }
}

/// Run the TUI until quit. Restores the terminal on every exit path.
///
/// # Errors
///
/// Returns when terminal init/draw fails or the collection cannot be indexed.
pub fn run(root: &Path) -> io::Result<()> {
    let mut app = match App::new(root) {
        Ok(app) => app,
        Err(wiki_reader_core::Error::EmptyCollection) => {
            eprintln!(
                "error: collection has no markdown pages: {}",
                root.display()
            );
            std::process::exit(1);
        }
        Err(wiki_reader_core::Error::PageNotFound(key)) => {
            eprintln!(
                "error: start page not found: {}",
                key.relative_path.display()
            );
            std::process::exit(1);
        }
        Err(err) => return Err(io::Error::other(err)),
    };

    install_panic_hook();
    let mut terminal = ratatui::try_init()?;
    execute!(stdout(), EnableMouseCapture)?;
    let result = run_loop(&mut terminal, &mut app);
    restore_terminal();
    result
}

fn install_panic_hook() {
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));
}

fn restore_terminal() {
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let mut last_width = 80u16;
    loop {
        terminal.draw(|frame| {
            last_width = frame.area().width;
            draw(frame, app);
        })?;
        if app.quit {
            return Ok(());
        }
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action =
                        keymap::map_global(key).or_else(|| keymap::map_pane(key, app.focus));
                    if let Some(action) = action {
                        let nav_action = matches!(
                            action,
                            Action::GoToPage(_) | Action::PrevPage | Action::NextPage
                        );
                        app.update(action);
                        if nav_action {
                            app.close_overlay_after_nav(last_width);
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let Some(action) = apply_mouse(app, mouse) {
                        let nav_action = matches!(
                            action,
                            Action::GoToPage(_) | Action::PrevPage | Action::NextPage
                        );
                        app.update(action);
                        if nav_action {
                            app.close_overlay_after_nav(last_width);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn apply_mouse(app: &mut App, mouse: ratatui::crossterm::event::MouseEvent) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let hit = app.hit_map.hit_at(mouse.column, mouse.row)?;
            Some(HitMap::action_for(hit))
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let over_nav = app.hit_map.entries().iter().any(|(r, h)| {
                mouse.column >= r.x
                    && mouse.column < r.x.saturating_add(r.width)
                    && mouse.row >= r.y
                    && mouse.row < r.y.saturating_add(r.height)
                    && matches!(
                        h,
                        Hit::FocusNav
                            | Hit::NavItem(_)
                            | Hit::NavGroupToggle(_)
                            | Hit::NavSearchRow
                    )
            });
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
            if over_nav {
                app.nav_scroll = if up {
                    app.nav_scroll.saturating_sub(1)
                } else {
                    app.nav_scroll.saturating_add(1)
                };
                app.clamp_nav_scroll();
            } else {
                app.scroll = if up {
                    app.scroll.saturating_sub(1)
                } else {
                    app.scroll.saturating_add(1)
                };
                app.clamp_viewer_scroll();
            }
            None
        }
        _ => None,
    }
}

/// Draw all regions and rebuild the hit map.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    app.hit_map.clear();
    let area = frame.area();
    if app.nav_visible.is_none() {
        app.nav_visible = Some(area.width >= 80);
    }
    let regions = layout::split(area, app.nav_visible.unwrap_or(false));
    app.viewer_rows = regions.viewer.height.max(1);
    // Borders (2) + search row (1); remaining rows show the tree.
    app.nav_viewport = regions.side_nav.height.saturating_sub(3).max(1);
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();

    header::draw(frame, regions.header, &crumbs, &theme, &mut app.hit_map);

    viewer::draw(
        frame,
        regions.viewer,
        app.doc.lines(),
        app.scroll,
        app.cursor_line,
        app.focus == FocusPane::Viewer,
        &theme,
        &mut app.hit_map,
    );

    let prev = nav.tree.prev(&page);
    let next = nav.tree.next(&page);
    let prev_label = prev
        .as_ref()
        .map(|k| wiki_reader_core::nav::page_label(app.navigator.index(), k));
    let next_label = next
        .as_ref()
        .map(|k| wiki_reader_core::nav::page_label(app.navigator.index(), k));
    footer::draw(
        frame,
        regions.footer,
        prev_label.as_deref(),
        next_label.as_deref(),
        &theme,
        &mut app.hit_map,
    );

    if regions.side_nav.width > 0 {
        side_nav::draw(
            frame,
            regions.side_nav,
            &nav.tree,
            &nav.expanded,
            &page,
            nav.cursor.as_ref(),
            app.nav_scroll,
            app.focus == FocusPane::Nav,
            &theme,
            &mut app.hit_map,
        );
    }

    let total = u32::try_from(app.doc.lines().len().max(1)).unwrap_or(1);
    let pct = ((app.scroll.saturating_add(1)) * 100) / total;
    let path = page.relative_path.display().to_string();
    status::draw(
        frame,
        regions.status,
        &StatusModel {
            focus: app.focus,
            path: &path,
            line: app.cursor_line.saturating_add(1),
            pct,
            words: app.doc.word_count(),
            minutes: status::reading_minutes(app.doc.word_count()),
            updated: app.doc.updated(),
            message: &app.message,
        },
        &theme,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::MouseEvent;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example")
    }

    fn render_at(root: &Path, width: u16, height: u16) -> String {
        let mut app = App::new(root).expect("app");
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
        format!("{:?}", terminal.backend().buffer())
    }

    /// Find the first cell whose symbol contains `needle`; return its (x, y).
    fn find_glyph(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if buf[(x, y)].symbol().contains(needle) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    fn draw_app(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, app)).expect("draw");
        terminal
    }

    fn click_at(app: &mut App, x: u16, y: u16) -> Option<Action> {
        let hit = app.hit_map.hit_at(x, y)?;
        Some(HitMap::action_for(hit))
    }

    #[test]
    fn snapshots_responsive_widths() {
        let root = fixture();
        for w in [60u16, 80, 120] {
            let out = render_at(&root, w, 24);
            insta::assert_snapshot!(format!("shell_{w}"), out);
        }
    }

    #[test]
    fn hit_map_rebuilds_and_quit_click() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        let terminal = draw_app(&mut app, 100, 24);
        let (qx, qy) = find_glyph(terminal.backend().buffer(), "✕").expect("✕ glyph");
        assert_eq!(
            app.hit_map.hit_at(qx, qy),
            Some(&Hit::Quit),
            "✕ column must hit Quit"
        );
        let action = click_at(&mut app, qx, qy);
        assert_eq!(action, Some(Action::Quit));
        app.update(Action::Quit);
        assert!(app.quit);
    }

    #[test]
    fn header_icon_glyphs_hit_their_actions() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        let terminal = draw_app(&mut app, 120, 24);
        let buf = terminal.backend().buffer();
        let (tx, ty) = find_glyph(buf, "◫").expect("◫");
        let (qx, qy) = find_glyph(buf, "✕").expect("✕");
        assert_eq!(app.hit_map.hit_at(tx, ty), Some(&Hit::NavToggle));
        assert_eq!(app.hit_map.hit_at(qx, qy), Some(&Hit::Quit));
    }

    #[test]
    fn breadcrumb_glyph_click_navigates() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::GoToPage(PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        }));
        let terminal = draw_app(&mut app, 120, 24);
        let buf = terminal.backend().buffer();
        // Find "Architecture" crumb text in the header row (y=0).
        let mut ax = None;
        for x in 0..buf.area.width {
            if buf[(x, 0)].symbol().starts_with('A') {
                // Walk to confirm "Architecture" run.
                let mut s = String::new();
                for dx in 0..12 {
                    if x + dx < buf.area.width {
                        s.push_str(buf[(x + dx, 0)].symbol());
                    }
                }
                if s.starts_with("Architecture") {
                    ax = Some(x);
                    break;
                }
            }
        }
        let ax = ax.expect("Architecture crumb glyph");
        let hit = app.hit_map.hit_at(ax, 0).expect("crumb hit");
        assert!(
            matches!(hit, Hit::Breadcrumb(k) if k.relative_path.ends_with("architecture/README.md")),
            "got {hit:?}"
        );
        app.update(HitMap::action_for(hit));
        assert_eq!(
            app.navigator.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
    }

    #[test]
    fn nav_toggle_hides_at_120_and_fresh_60_has_no_overlay() {
        let root = fixture();
        let mut wide = App::new(&root).unwrap();
        let _ = draw_app(&mut wide, 120, 24);
        assert_eq!(wide.nav_visible, Some(true));
        assert!(
            wide.hit_map
                .entries()
                .iter()
                .any(|(_, h)| matches!(h, Hit::NavItem(_)))
        );
        wide.update(Action::ToggleNav);
        let _ = draw_app(&mut wide, 120, 24);
        assert_eq!(wide.nav_visible, Some(false));
        assert!(
            !wide
                .hit_map
                .entries()
                .iter()
                .any(|(_, h)| matches!(h, Hit::NavItem(_) | Hit::NavSearchRow))
        );
        // Viewer should be wider with nav hidden (no FocusNav pane hits spanning left).
        let viewer_w = wide
            .hit_map
            .entries()
            .iter()
            .find_map(|(r, h)| matches!(h, Hit::FocusViewer).then_some(r.width))
            .unwrap_or(0);
        assert!(viewer_w >= 110, "viewer width {viewer_w}");

        let mut narrow = App::new(&root).unwrap();
        let _ = draw_app(&mut narrow, 60, 24);
        assert_eq!(narrow.nav_visible, Some(false));
        assert!(
            !narrow
                .hit_map
                .entries()
                .iter()
                .any(|(_, h)| matches!(h, Hit::NavItem(_) | Hit::NavSearchRow))
        );
    }

    #[test]
    fn mouse_and_key_prev_agree_on_history() {
        let root = fixture();
        let mut via_key = App::new(&root).unwrap();
        via_key.update(Action::NextPage);
        via_key.update(Action::NextPage);
        via_key.update(Action::PrevPage);
        let page_key = via_key.navigator.tab().current().page.clone();
        let hist_key: Vec<_> = via_key
            .navigator
            .tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect();

        let mut via_mouse = App::new(&root).unwrap();
        via_mouse.update(Action::NextPage);
        via_mouse.update(Action::NextPage);
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut via_mouse)).unwrap();
        let (px, py) = {
            let prev = via_mouse
                .hit_map
                .entries()
                .iter()
                .rev()
                .find(|(_, h)| matches!(h, Hit::Prev));
            assert!(prev.is_some(), "prev hit missing");
            let r = prev.unwrap().0;
            (r.x, r.y)
        };
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: px,
            row: py,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        };
        if let Some(a) = apply_mouse(&mut via_mouse, mouse) {
            via_mouse.update(a);
        }
        assert_eq!(via_mouse.navigator.tab().current().page, page_key);
        let hist_mouse: Vec<_> = via_mouse
            .navigator
            .tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect();
        assert_eq!(hist_key, hist_mouse);
    }

    #[test]
    fn breadcrumb_click_navigates() {
        // Kept as hit-map variant lookup; glyph-coordinate coverage is in
        // `breadcrumb_glyph_click_navigates`.
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::GoToPage(PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        }));
        let _ = draw_app(&mut app, 120, 24);
        let action = {
            let crumb = app.hit_map.entries().iter().find(|(_, h)| {
                matches!(h, Hit::Breadcrumb(k) if k.relative_path.ends_with("architecture/README.md"))
            });
            assert!(crumb.is_some(), "architecture crumb missing");
            HitMap::action_for(&crumb.unwrap().1)
        };
        app.update(action);
        assert_eq!(
            app.navigator.tab().current().page.relative_path,
            PathBuf::from("architecture/README.md")
        );
    }

    #[test]
    fn nav_item_click_matches_go_to_page() {
        let root = fixture();
        let target = PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("decisions/0001-stack.md"),
        };
        let mut via_action = App::new(&root).unwrap();
        via_action.update(Action::GoToPage(target.clone()));
        let hist_a: Vec<_> = via_action
            .navigator
            .tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect();

        let mut via_click = App::new(&root).unwrap();
        // Expand decisions so the page row is visible.
        via_click
            .navigator
            .set_group_expanded(NodeId::Group(PathBuf::from("decisions")), true);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut via_click)).unwrap();
        let action = {
            let hit = via_click
                .hit_map
                .entries()
                .iter()
                .find(|(_, h)| matches!(h, Hit::NavItem(NodeId::Page(k)) if k == &target));
            assert!(hit.is_some(), "nav item hit missing");
            HitMap::action_for(&hit.unwrap().1)
        };
        via_click.update(action);
        let hist_b: Vec<_> = via_click
            .navigator
            .tab()
            .history
            .iter()
            .map(|l| l.page.relative_path.clone())
            .collect();
        assert_eq!(hist_a, hist_b);
        assert!(
            via_click
                .navigator
                .nav()
                .expanded
                .contains(&NodeId::Group(PathBuf::from("decisions")))
        );
    }

    #[test]
    fn prev_next_auto_expands_ancestors() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::NextPage); // architecture README
        app.update(Action::NextPage); // design-system README
        assert!(
            app.navigator
                .nav()
                .expanded
                .contains(&NodeId::Group(PathBuf::from("architecture")))
        );
        assert!(
            app.navigator
                .nav()
                .expanded
                .contains(&NodeId::Group(PathBuf::from("architecture/design-system")))
        );
    }

    #[test]
    fn focus_stale_cursor_round_trip() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::FocusNav);
        app.update(Action::NavStepDown);
        let remembered = app.navigator.nav().cursor.clone();
        app.update(Action::FocusViewer);
        app.update(Action::FocusNav);
        assert_eq!(app.navigator.nav().cursor, remembered);

        app.update(Action::FocusViewer);
        app.update(Action::NextPage);
        app.update(Action::NextPage);
        let current = app.navigator.tab().current().page.clone();
        app.update(Action::FocusNav);
        assert_eq!(
            app.navigator.nav().cursor,
            Some(NodeId::Page(current)),
            "stale-cursor should jump to current page"
        );
    }

    #[test]
    fn nav_k4_steps_and_activate() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::FocusNav);
        app.nav_on_search = true;
        app.update(Action::NavStepDown);
        assert!(!app.nav_on_search);
        app.update(Action::NavJumpDown);
        let id = app.navigator.nav().cursor.clone().unwrap();
        assert!(matches!(id, NodeId::Group(_)));
        app.update(Action::NavExpand);
        assert!(app.navigator.nav().expanded.contains(&id));
        app.update(Action::NavActivate);
        assert!(!app.navigator.nav().expanded.contains(&id));
    }

    #[test]
    fn viewer_cursor_scroll_and_back_restore() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::FocusViewer);
        app.update(Action::ViewerDown);
        app.update(Action::ViewerDown);
        assert_eq!(app.cursor_line, 2);
        let scroll_before = app.scroll;
        app.update(Action::GoToPage(PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/README.md"),
        }));
        // Leave with view state saved on navigate from README... we navigated from root
        // with cursor 2; go back.
        app.update(Action::Back);
        assert_eq!(app.cursor_line, 2);
        assert_eq!(app.scroll, scroll_before);
    }

    #[test]
    fn viewer_tab_clears_on_arrow() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::FocusViewer);
        app.update(Action::ViewerTab);
        // May or may not find items; either way arrow clears.
        app.focused_item = Some(0);
        app.update(Action::ViewerDown);
        assert_eq!(app.focused_item, None);
    }

    #[test]
    fn viewer_cursor_stays_in_viewport_at_short_height() {
        let root = fixture();
        for height in [12u16, 24] {
            let mut app = App::new(&root).unwrap();
            let _ = draw_app(&mut app, 100, height);
            let rows = u32::from(app.viewer_rows);
            assert!(rows >= 1);
            for _ in 0..30 {
                app.update(Action::ViewerDown);
            }
            assert!(
                app.cursor_line >= app.scroll && app.cursor_line < app.scroll.saturating_add(rows),
                "h={height}: cursor {} not in [{}, {})",
                app.cursor_line,
                app.scroll,
                app.scroll.saturating_add(rows)
            );
        }
    }

    #[test]
    fn wheel_scroll_clamps_to_doc() {
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        let _ = draw_app(&mut app, 100, 24);
        let max = u32::try_from(app.doc.lines().len().saturating_sub(1)).unwrap_or(0);
        let mouse = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 10,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        };
        for _ in 0..500 {
            let _ = apply_mouse(&mut app, mouse);
        }
        assert!(app.scroll <= max, "scroll {} > max {max}", app.scroll);
    }

    #[test]
    fn next_page_keeps_current_row_visible_in_nav() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki");
        if !root.exists() {
            return;
        }
        let mut app = App::new(&root).unwrap();
        let _ = draw_app(&mut app, 120, 24);
        for _ in 0..20 {
            app.update(Action::NextPage);
            let _ = draw_app(&mut app, 120, 24);
            let page = app.navigator.tab().current().page.clone();
            let rows = app.nav_rows();
            let Some(idx) = rows.iter().position(|r| r.id == NodeId::Page(page.clone())) else {
                continue;
            };
            let scroll = usize::from(app.nav_scroll);
            let vh = usize::from(app.nav_viewport);
            assert!(
                idx >= scroll && idx < scroll + vh,
                "page {:?} at idx {idx} not in nav viewport [{scroll}, {})",
                page.relative_path,
                scroll + vh
            );
        }
    }
}
