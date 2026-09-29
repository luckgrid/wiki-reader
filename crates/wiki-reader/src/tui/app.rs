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

/// Owned application state.
pub struct App {
    /// Navigation session.
    pub navigator: Navigator,
    /// Collection provider.
    pub provider: FsProvider,
    /// Focused pane.
    pub focus: FocusPane,
    /// Side nav visible (or overlay forced open).
    pub nav_open: bool,
    /// Side nav scroll offset (rows below the search line).
    pub nav_scroll: u16,
    /// When true, the ⌕ Search… row is the nav cursor (not a `NodeId`).
    pub nav_on_search: bool,
    /// Raw page lines currently shown.
    pub lines: Vec<String>,
    /// Viewer cursor (0-based source line index).
    pub cursor_line: u32,
    /// Viewer scroll offset (lines from top).
    pub scroll: u32,
    /// Word count of current page.
    pub word_count: u32,
    /// Updated frontmatter string.
    pub updated: String,
    /// Status / notice message.
    pub message: String,
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
            nav_open: true,
            nav_scroll: 0,
            nav_on_search: false,
            lines: Vec::new(),
            cursor_line: 0,
            scroll: 0,
            word_count: 0,
            updated: "—".into(),
            message: String::new(),
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
                self.nav_open = !self.nav_open;
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
                let max = u32::try_from(self.lines.len().saturating_sub(1)).unwrap_or(u32::MAX);
                self.cursor_line = line.min(max);
                self.focus = FocusPane::Viewer;
            }
            Action::NavStepUp => self.nav_step(-1),
            Action::NavStepDown => self.nav_step(1),
            Action::NavJumpUp => self.nav_jump_group(-1),
            Action::NavJumpDown => self.nav_jump_group(1),
            Action::NavExpand => self.nav_expand(),
            Action::NavCollapse => self.nav_collapse(),
            Action::NavActivate => self.nav_activate(),
            // Viewer keys handled in P1-08b/c (wired in keymap already).
            Action::ViewerUp
            | Action::ViewerDown
            | Action::ViewerBlockUp
            | Action::ViewerBlockDown
            | Action::ViewerPageUp
            | Action::ViewerPageDown
            | Action::ViewerHome
            | Action::ViewerEnd
            | Action::ViewerTab
            | Action::ViewerBackTab
            | Action::None => {}
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
                return;
            }
            for pos in (1..cur).rev() {
                if rows[pos - 1].is_group {
                    self.nav_on_search = false;
                    self.navigator.set_nav_cursor(rows[pos - 1].id.clone());
                    return;
                }
            }
            self.nav_on_search = true;
        } else {
            for (idx, row) in rows.iter().enumerate() {
                let pos = idx + 1;
                if pos <= cur {
                    continue;
                }
                if row.is_group {
                    self.nav_on_search = false;
                    self.navigator.set_nav_cursor(row.id.clone());
                    return;
                }
            }
        }
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
                Effect::RevealInTree(_) => {}
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
                self.lines = src.lines().map(str::to_owned).collect();
                if let Some(page) = self.navigator.index().pages.get(key) {
                    self.word_count = page.parsed.word_count;
                    self.updated = page
                        .parsed
                        .frontmatter
                        .updated
                        .clone()
                        .unwrap_or_else(|| "—".into());
                } else {
                    self.word_count = 0;
                    self.updated = "—".into();
                }
                self.cursor_line = 0;
                self.scroll = 0;
            }
            Err(err) => {
                self.message = format!("read failed: {err}");
            }
        }
    }

    fn anchor_line(&self, slug: &str) -> Option<u32> {
        let key = &self.navigator.tab().current().page;
        let page = self.navigator.index().pages.get(key)?;
        page.parsed
            .headings
            .iter()
            .find(|h| h.slug == slug)
            .map(|h| h.source_line)
    }

    fn close_overlay_after_nav(&mut self, term_width: u16) {
        if term_width < 80 {
            self.nav_open = false;
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
            } else {
                app.scroll = if up {
                    app.scroll.saturating_sub(1)
                } else {
                    app.scroll.saturating_add(1)
                };
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
    let regions = layout::split(area, app.nav_open);
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();

    header::draw(frame, regions.header, &crumbs, &theme, &mut app.hit_map);

    viewer::draw(
        frame,
        regions.viewer,
        &app.lines,
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

    let total = u32::try_from(app.lines.len().max(1)).unwrap_or(1);
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
            words: app.word_count,
            minutes: status::reading_minutes(app.word_count),
            updated: &app.updated,
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
        if width < 80 {
            app.nav_open = false;
        }
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
        format!("{:?}", terminal.backend().buffer())
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
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let (qx, qy) = {
            let quit_hit = app
                .hit_map
                .entries()
                .iter()
                .rev()
                .find(|(_, h)| matches!(h, Hit::Quit));
            assert!(quit_hit.is_some(), "quit hit missing");
            let rect = quit_hit.unwrap().0;
            (rect.x, rect.y)
        };
        let action = click_at(&mut app, qx, qy);
        assert_eq!(action, Some(Action::Quit));
        app.update(Action::Quit);
        assert!(app.quit);
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
        let root = fixture();
        let mut app = App::new(&root).unwrap();
        app.update(Action::GoToPage(PageKey {
            collection_id: "worked-example".into(),
            relative_path: PathBuf::from("architecture/design-system/tokens.md"),
        }));
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
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
}
