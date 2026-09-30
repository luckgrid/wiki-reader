use super::draw::draw;
use super::events::apply_mouse;
use super::*;
use crate::tui::action::Action;
use crate::tui::focus::FocusPane;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::viewer_doc::{FocusTarget, ViewerDoc};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_reader_core::nav::{NavStop, NodeId};
use wiki_reader_core::provider::PageKey;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example")
}

fn render_at(root: &Path, width: u16, height: u16) -> String {
    let mut app = App::new(root).expect("app");
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
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
    // Tall terminal: vertical gaps above/below header and status.
    let tall = render_at(&root, 120, 40);
    insta::assert_snapshot!("shell_120x40", tall);
}

#[test]
fn viewer_text_width_matches_painted_content() {
    use crate::tui::layout::{self, VIEWER_LEFT_PAD};
    let area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    let regions = layout::split(area, true, None);
    let want = layout::viewer_text_width(regions.viewer);
    let inner_w = regions.viewer.width.saturating_sub(2);
    let painted = inner_w.saturating_sub(VIEWER_LEFT_PAD).min(100);
    assert_eq!(want, painted);
}

#[test]
fn chrome_pad_hits_header_icons_and_nav_search() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let terminal = draw_app(&mut app, 120, 24);
    let buf = terminal.backend().buffer();
    let (tx, ty) = find_glyph(buf, "◫").expect("◫");
    let (qx, qy) = find_glyph(buf, "✕").expect("✕");
    assert_eq!(app.hit_map.hit_at(tx, ty), Some(&Hit::NavToggle));
    assert_eq!(app.hit_map.hit_at(qx, qy), Some(&Hit::Quit));
    // Padded: icons sit one col inset from the raw edge.
    assert_eq!(qx, 120 - 2, "✕ at chrome_pad right edge");
    assert_eq!(tx, 120 - 4, "◫ two cols left of ✕");

    // Nav search row is below the blank gap under the title (inner.y + 1).
    let search = (0..buf.area.width)
        .find(|&x| buf[(x, 3)].symbol().contains('⌕'))
        .expect("search glyph");
    assert_eq!(
        app.hit_map.hit_at(search, 3),
        Some(&Hit::NavSearchRow),
        "search hit on padded chrome row"
    );
}

#[test]
fn cursor_row_background_fills_full_viewer_width() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let terminal = draw_app(&mut app, 100, 24);
    let buf = terminal.backend().buffer();
    let want = crate::tui::theme::Theme::default().cursor_line;
    // Last inner column of the viewer (right border is at width - 1); short lines
    // leave this cell empty, so only a row-wide style can colour it.
    let x = buf.area.width - 2;
    let rows: Vec<u16> = (0..buf.area.height)
        .filter(|&y| buf[(x, y)].bg == want)
        .collect();
    assert_eq!(rows.len(), 1, "exactly one cursor row, got {rows:?}");
    // ▌ marker in the left pad column of that row.
    let marker_x = {
        // Viewer starts after nav (26 at width 100).
        let regions = crate::tui::layout::split(
            ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 24,
            },
            true,
            None,
        );
        regions.viewer.x + 1 // after left border
    };
    let y = rows[0];
    assert!(
        buf[(marker_x, y)].symbol().contains('▌'),
        "cursor marker at ({marker_x},{y}), got {:?}",
        buf[(marker_x, y)].symbol()
    );
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
    // Find a clickable architecture crumb (Filename mode: "Readme" for architecture/README).
    let mut ax = None;
    for x in 0..buf.area.width {
        if buf[(x, 0)].symbol().starts_with('R') {
            let mut s = String::new();
            for dx in 0..6 {
                if x + dx < buf.area.width {
                    s.push_str(buf[(x + dx, 0)].symbol());
                }
            }
            if s.starts_with("Readme") {
                // Prefer the architecture crumb (second "Readme"), not the root.
                if let Some(Hit::Breadcrumb(k)) = app.hit_map.hit_at(x, 0)
                    && k.relative_path.ends_with("architecture/README.md")
                {
                    ax = Some(x);
                    break;
                }
            }
        }
    }
    let ax = ax.expect("architecture Readme crumb glyph");
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
    assert!(wide.nav_visible);
    assert!(
        wide.hit_map
            .entries()
            .iter()
            .any(|(_, h)| matches!(h, Hit::NavItem(_)))
    );
    wide.update(Action::ToggleNav);
    let _ = draw_app(&mut wide, 120, 24);
    assert!(!wide.nav_visible);
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
    assert!(!narrow.nav_visible);
    assert!(
        !narrow
            .hit_map
            .entries()
            .iter()
            .any(|(_, h)| matches!(h, Hit::NavItem(_) | Hit::NavSearchRow))
    );
}

#[test]
fn footer_prev_next_clicks_change_page() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 80, 24);
    let next = app
        .hit_map
        .entries()
        .iter()
        .rev()
        .find(|(_, h)| matches!(h, Hit::Next))
        .map(|(r, _)| (r.x, r.y))
        .expect("Next hit");
    let before = app.navigator.tab().current().page.clone();
    let expected = app.navigator.nav().tree.next(&before).expect("has next");
    if let Some(a) = click_at(&mut app, next.0, next.1) {
        app.update(a);
    }
    assert_eq!(app.navigator.tab().current().page, expected);

    let _ = draw_app(&mut app, 80, 24);
    let prev = app
        .hit_map
        .entries()
        .iter()
        .rev()
        .find(|(_, h)| matches!(h, Hit::Prev))
        .map(|(r, _)| (r.x, r.y))
        .expect("Prev hit");
    if let Some(a) = click_at(&mut app, prev.0, prev.1) {
        app.update(a);
    }
    assert_eq!(app.navigator.tab().current().page, before);
}

#[test]
fn nav_divider_drag_changes_width_and_clamps() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.nav_visible = true;
    let _ = draw_app(&mut app, 120, 24);
    let (dx, dy) = app
        .hit_map
        .entries()
        .iter()
        .rev()
        .find(|(_, h)| matches!(h, Hit::NavDivider))
        .map(|(r, _)| (r.x, r.y))
        .expect("NavDivider");
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: dx,
        row: dy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    assert!(apply_mouse(&mut app, down).is_none());
    assert!(app.nav_dragging);

    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 39, // → width 40
        row: dy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let _ = apply_mouse(&mut app, drag);
    assert_eq!(app.nav_width, Some(40));
    let _ = draw_app(&mut app, 120, 24);
    assert_eq!(
        crate::tui::layout::split(
            ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: 120,
                height: 24
            },
            true,
            app.nav_width
        )
        .side_nav
        .width,
        40
    );

    // Clamp high
    let drag_hi = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 200,
        row: dy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let _ = apply_mouse(&mut app, drag_hi);
    assert_eq!(app.nav_width, Some(crate::tui::layout::NAV_WIDTH_MAX));

    // Clamp low
    let drag_lo = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 0,
        row: dy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let _ = apply_mouse(&mut app, drag_lo);
    assert_eq!(app.nav_width, Some(crate::tui::layout::NAV_WIDTH_MIN));

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 0,
        row: dy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let _ = apply_mouse(&mut app, up);
    assert!(!app.nav_dragging);
}

#[test]
fn narrow_terminal_ignores_stored_nav_width() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.nav_width = Some(40);
    app.nav_visible = true;
    let _ = draw_app(&mut app, 60, 24);
    let regions = crate::tui::layout::split(
        ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 60,
            height: 24,
        },
        true,
        app.nav_width,
    );
    // Overlay uses clamp(16,30), not the stored 40.
    assert!(regions.nav_overlay);
    assert!(regions.side_nav.width <= 30);
    assert_ne!(regions.side_nav.width, 40);
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
        NavStop::Node(NodeId::Page(current)),
        "stale-cursor should jump to current page"
    );
}

#[test]
fn nav_k4_steps_and_activate() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    assert!(!matches!(app.navigator.nav().cursor, NavStop::Search));
    app.update(Action::NavJumpDown);
    let NavStop::Node(id) = app.navigator.nav().cursor.clone() else {
        panic!("expected node");
    };
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
    let _ = draw_app(&mut app, 100, 24);
    app.update(Action::FocusViewer);
    // Skip frontmatter box lines so source_map is unique for this cursor.
    for _ in 0..12 {
        app.update(Action::ViewerDown);
    }
    let source = app.doc.source_cursor(app.cursor_line);
    assert!(source > 0, "expected body source line, got {source}");
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    app.update(Action::Back);
    assert_eq!(app.doc.source_cursor(app.cursor_line), source);
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
fn leaf_readme_left_uses_parent_group() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    // tokens.md sits under architecture/design-system group.
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    app.update(Action::FocusNav);
    app.update(Action::NavCollapse); // ← on a page → parent group
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Group(PathBuf::from("architecture/design-system")))
    );
    // Folded leaf: go to a top-level-ish page and ← should not invent phantom groups.
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("README.md"),
    }));
    app.update(Action::FocusNav);
    let before = app.navigator.nav().cursor.clone();
    app.update(Action::NavCollapse);
    assert_eq!(app.navigator.nav().cursor, before);
}

#[test]
fn search_row_is_nav_stop_with_cursor() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    let _ = draw_app(&mut app, 120, 24);
    assert!(matches!(app.navigator.nav().cursor, NavStop::Search));
    // Status/search activate.
    app.update(Action::NavActivate);
    assert!(app.search.is_some());
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Overlay);
}

#[test]
fn nav_row_click_focuses_nav_pane() {
    let root = fixture();
    let target = PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("decisions/0001-stack.md"),
    };
    let mut app = App::new(&root).unwrap();
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("decisions")), true);
    let _ = draw_app(&mut app, 120, 30);
    let (x, y) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::NavItem(NodeId::Page(k)) if k == &target))
        .map(|(r, _)| (r.x, r.y))
        .expect("nav item");
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    if let Some(a) = apply_mouse(&mut app, mouse) {
        app.update(a);
    }
    assert_eq!(app.focus, FocusPane::Nav);
    // Stale-cursor path: leaving and returning should keep nav focus machinery.
    app.update(Action::FocusViewer);
    assert_eq!(app.focus, FocusPane::Viewer);
    let _ = draw_app(&mut app, 120, 30);
    let (vx, vy) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::ViewerLine(_)))
        .map(|(r, _)| (r.x, r.y))
        .expect("viewer line");
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: vx,
        row: vy,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    if let Some(a) = apply_mouse(&mut app, mouse) {
        app.update(a);
    }
    assert_eq!(app.focus, FocusPane::Viewer);
}

#[test]
fn status_message_clears_on_arrow_and_survives_narrow() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.message = "→ #heading".into();
    app.update(Action::ViewerDown);
    assert!(app.message.is_empty());

    app.update(Action::ViewerTab);
    let _ = draw_app(&mut app, 60, 24);
    // At 60 cols, status still draws (message or focus target may be empty if no links).
    assert!(
        app.hit_map
            .entries()
            .iter()
            .any(|(r, _)| r.y == 23 || r.height > 0)
    );
}

#[test]
fn nav_cursor_follows_page_when_nav_focused() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NextPage);
    let page = app.navigator.tab().current().page.clone();
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Page(page))
    );
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

#[test]
fn tab_cycle_through_app_includes_footer() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusViewer);
    let n = app.focus_list().len();
    assert!(n >= 2, "expected links+footer, got {n}");
    let mut saw_footer = false;
    for _ in 0..n {
        app.update(Action::ViewerTab);
        let items = app.focus_list();
        let i = app.focused_item.expect("focused");
        if matches!(
            items[i].kind,
            FocusTarget::FooterPrev | FocusTarget::FooterNext
        ) {
            saw_footer = true;
        }
    }
    assert!(saw_footer, "Tab cycle should reach footer buttons");
    let _ = draw_app(&mut app, 100, 24);
}

#[test]
fn nav_visibility_reseeds_on_resize_without_user_toggle() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 60, 24);
    assert!(!app.nav_visible);
    let _ = draw_app(&mut app, 120, 24);
    assert!(app.nav_visible);
    let _ = draw_app(&mut app, 60, 24);
    assert!(!app.nav_visible);
}

#[test]
fn overlay_closes_after_nav_activate_at_narrow_width() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 60, 24);
    app.update(Action::ToggleNav);
    assert!(app.nav_visible);
    app.update(Action::FocusNav);
    app.navigator.set_nav_cursor(NodeId::Page(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    app.update(Action::NavActivate);
    assert!(!app.nav_visible);
}

#[test]
fn nav_scroll_clamped_after_collapse_long_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/deep-tree");
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 120, 12);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1")), true);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1/l2")), true);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1/l2/l3")), true);
    app.nav_scroll = 100;
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1/l2/l3")), false);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1/l2")), false);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("l1")), false);
    app.clamp_nav_scroll();
    let n = app.nav_rows().len();
    let vh = usize::from(app.nav_viewport.max(1));
    let max = n.saturating_sub(vh);
    assert!(usize::from(app.nav_scroll) <= max);
}

#[test]
fn view_mode_toggle_round_trips_raw_and_rendered() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    assert!(matches!(
        app.doc,
        crate::tui::page_doc::PageDoc::Rendered(_)
    ));
    for _ in 0..12 {
        app.update(Action::ViewerDown);
    }
    let source = app.doc.source_cursor(app.cursor_line);
    assert!(source > 0);
    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    assert_eq!(app.doc.source_cursor(app.cursor_line), source);
    app.update(Action::ToggleViewMode);
    assert!(matches!(
        app.doc,
        crate::tui::page_doc::PageDoc::Rendered(_)
    ));
    assert_eq!(app.doc.source_cursor(app.cursor_line), source);
}

#[test]
fn toggling_raw_does_not_sync_highlight_whole_page() {
    use crate::tui::highlight::highlight_markdown;
    // Warm syntect assets so the worker isn't racing a multi-second first load on CI.
    let _ = highlight_markdown("warm");
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    if let crate::tui::page_doc::PageDoc::Raw(doc) = &app.doc {
        assert!(
            doc.highlights.is_empty(),
            "raw must paint plain until the worker finishes"
        );
    }
    for _ in 0..200 {
        app.poll_watcher();
        if let crate::tui::page_doc::PageDoc::Raw(doc) = &app.doc
            && !doc.highlights.is_empty()
        {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("background highlight should apply after poll");
}

fn wait_raw_highlights(app: &mut App) -> usize {
    for _ in 0..300 {
        app.poll_watcher();
        if let crate::tui::page_doc::PageDoc::Raw(doc) = &app.doc
            && !doc.highlights.is_empty()
        {
            return doc.highlights.len();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("background highlight should apply after poll");
}

#[test]
fn rapid_raw_navigation_applies_only_final_page_highlight() {
    use crate::tui::highlight::highlight_markdown;
    let _ = highlight_markdown("warm");
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    // Supersede the landing-page job with a different page before it can apply.
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    let expected_lines = app.doc.lines().len();
    let hl_lines = wait_raw_highlights(&mut app);
    assert_eq!(
        hl_lines, expected_lines,
        "highlights must match the final page, not a superseded request"
    );
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("architecture/design-system/tokens.md")
    );
}

#[test]
fn superseded_highlight_does_not_overwrite_after_leaving_raw() {
    use crate::tui::highlight::highlight_markdown;
    let _ = highlight_markdown("warm");
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    app.update(Action::ToggleViewMode); // start raw highlight
    app.update(Action::ToggleViewMode); // leave raw (cancel token)
    assert!(matches!(
        app.doc,
        crate::tui::page_doc::PageDoc::Rendered(_)
    ));
    // Give any in-flight worker time to finish and attempt apply.
    for _ in 0..50 {
        app.poll_watcher();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        matches!(app.doc, crate::tui::page_doc::PageDoc::Rendered(_)),
        "superseded highlight must not change the current rendered doc"
    );
}

fn link_chain_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/link-chain")
}

#[test]
fn link_chain_ten_links_ten_backs_one_tab() {
    let root = link_chain_root();
    let mut app = App::new(&root).unwrap();
    // Land on the intro paragraph (display index shifts with block gaps).
    app.cursor_line = app
        .doc
        .lines()
        .iter()
        .position(|l| l.contains("Multi-link"))
        .map_or(1, |i| u32::try_from(i).unwrap_or(0));
    let saved_source = app.doc.source_cursor(app.cursor_line);
    app.scroll = 0;
    for i in 1..=10 {
        app.follow_link_target(&format!("{i:02}.md"));
    }
    assert_eq!(app.navigator.tab_count(), 1);
    assert_eq!(app.navigator.tab().history.len(), 11);
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("10.md")
    );
    for _ in 0..10 {
        app.update(Action::Back);
    }
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("README.md")
    );
    assert_eq!(app.doc.source_cursor(app.cursor_line), saved_source);
    assert_eq!(app.scroll, 0);
}

#[test]
fn click_link_glyph_follows_page() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let tokens_id = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.contains("tokens.md"))
        .expect("tokens link")
        .id
        .0;
    let link_rect = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Link(id) if *id == tokens_id))
        .map(|(r, _)| *r)
        .expect("tokens link hit rect");
    assert!(link_rect.width > 0, "link hit width {link_rect:?}");
    let col = link_rect.x.saturating_add(link_rect.width / 2);
    assert!(matches!(
        app.hit_map.hit_at(col, link_rect.y),
        Some(Hit::Link(_))
    ));
    let action = apply_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row: link_rect.y,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        },
    );
    let action = action.expect("mouse action");
    assert!(matches!(action, Action::FollowLinkId(id) if id == tokens_id));
    app.update(action);
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("architecture/design-system/tokens.md")
    );
}

#[test]
fn linked_from_backlink_in_tab_cycle_and_activate() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    assert!(
        app.doc.lines().iter().any(|l| l.contains("Linked from")),
        "rendered design-system should list Linked from"
    );
    let bl = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.ends_with("tokens.md") && s.raw_target.starts_with('/'))
        .expect("root-relative tokens backlink")
        .clone();
    // Backlink sits after body links in the Tab cycle (before footer).
    let items = app.focus_list();
    let bl_idx = items
        .iter()
        .position(|it| it.link_id == Some(bl.id))
        .expect("backlink in focus list");
    let footer_idx = items
        .iter()
        .position(|it| matches!(it.kind, FocusTarget::FooterPrev | FocusTarget::FooterNext))
        .expect("footer");
    assert!(bl_idx < footer_idx, "backlink before footer in Tab cycle");
    // Enter on the backlink matches tree-select history shape (replace, one tab).
    app.update(Action::FocusViewer);
    app.focused_item = Some(bl_idx);
    app.update(Action::ViewerActivate);
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("architecture/design-system/tokens.md")
    );
    assert_eq!(app.navigator.tab_count(), 1);
}

#[test]
fn linked_from_absent_in_raw_mode() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    assert!(
        !app.doc.lines().iter().any(|l| l.contains("Linked from")),
        "raw mode is source-only"
    );
}

#[test]
fn heading_jump_skips_body_and_stops_at_ends() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let headings = app.doc.heading_lines();
    assert!(headings.len() >= 2, "need ≥2 headings, got {headings:?}");
    app.cursor_line = 0;
    app.update(Action::ViewerHeadingDown);
    assert_eq!(app.cursor_line, headings[0].saturating_sub(1));
    // Body rows between headings are skipped.
    let between = headings[0]; // 1-based; land past first heading
    app.cursor_line = between; // 0-based line after first heading start
    app.update(Action::ViewerHeadingDown);
    assert_eq!(app.cursor_line, headings[1].saturating_sub(1));
    // Stop at last heading.
    for _ in 0..headings.len() + 2 {
        app.update(Action::ViewerHeadingDown);
    }
    assert_eq!(
        app.cursor_line,
        headings.last().copied().unwrap().saturating_sub(1)
    );
    // Stop at first heading going up.
    for _ in 0..headings.len() + 2 {
        app.update(Action::ViewerHeadingUp);
    }
    assert_eq!(app.cursor_line, headings[0].saturating_sub(1));
}

#[test]
fn heading_jump_works_in_raw_and_after_toggle() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let rendered_headings = app.doc.heading_lines();
    app.update(Action::ViewerHeadingDown);
    let rendered_line = app.cursor_line;
    assert_eq!(rendered_line, rendered_headings[0].saturating_sub(1));
    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    let raw_headings = app.doc.heading_lines();
    assert!(!raw_headings.is_empty());
    app.cursor_line = 0;
    app.update(Action::ViewerHeadingDown);
    assert_eq!(app.cursor_line, raw_headings[0].saturating_sub(1));
    app.update(Action::ToggleViewMode);
    assert!(matches!(
        app.doc,
        crate::tui::page_doc::PageDoc::Rendered(_)
    ));
    app.cursor_line = 0;
    app.update(Action::ViewerHeadingDown);
    assert_eq!(
        app.cursor_line,
        app.doc.heading_lines()[0].saturating_sub(1)
    );
}

#[test]
fn heading_jump_brace_keys_work_in_rendered_and_raw() {
    use crate::tui::focus::FocusPane;
    use crate::tui::keymap::{self, Chord, InputMode};
    use ratatui::crossterm::event::{KeyCode, KeyEvent};

    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let (down, _) = keymap::map(
        KeyEvent::from(KeyCode::Char('}')),
        FocusPane::Viewer,
        InputMode::Normal,
        Chord::None,
    );
    assert_eq!(down, Some(Action::ViewerHeadingDown));
    app.cursor_line = 0;
    app.update(down.unwrap());
    assert_eq!(
        app.cursor_line,
        app.doc.heading_lines()[0].saturating_sub(1)
    );

    app.update(Action::ToggleViewMode);
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    let (up, _) = keymap::map(
        KeyEvent::from(KeyCode::Char('{')),
        FocusPane::Viewer,
        InputMode::Normal,
        Chord::None,
    );
    assert_eq!(up, Some(Action::ViewerHeadingUp));
    let raw_headings = app.doc.heading_lines();
    app.cursor_line = raw_headings.last().copied().unwrap_or(1).saturating_sub(1);
    app.update(up.unwrap());
    assert_eq!(app.cursor_line, raw_headings[0].saturating_sub(1));
}

#[test]
fn open_in_editor_no_env_sets_message() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.open_in_editor_with(None);
    assert!(
        app.message.contains("$VISUAL") || app.message.contains("$EDITOR"),
        "expected no-editor message, got {:?}",
        app.message
    );
}

#[test]
fn open_in_editor_records_command_and_keeps_source_line() {
    use crate::tui::editor::RecordingEditor;
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    for _ in 0..8 {
        app.update(Action::ViewerDown);
    }
    let source = app.doc.source_cursor(app.cursor_line);
    let recorder = RecordingEditor::default();
    let log = recorder.launched.clone();
    app.editor = Box::new(recorder);
    app.open_in_editor_with(Some("nvim".into()));
    let cmds = log.lock().expect("lock");
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].program, "nvim");
    assert_eq!(cmds[0].args[0], format!("+{}", source.saturating_add(1)));
    assert!(
        cmds[0].args[1].ends_with("architecture/README.md"),
        "path={}",
        cmds[0].args[1]
    );
    drop(cmds);
    assert_eq!(app.doc.source_cursor(app.cursor_line), source);
}

#[test]
fn open_in_editor_splits_args_and_reloads_on_mtime_even_if_nonzero() {
    use crate::tui::editor::{EditorCmd, EditorExit, EditorLauncher};
    use std::sync::{Arc, Mutex};

    struct SaveThenFail {
        path: PathBuf,
        launched: Arc<Mutex<Vec<EditorCmd>>>,
    }
    impl EditorLauncher for SaveThenFail {
        fn launch(&self, cmd: &EditorCmd) -> std::io::Result<EditorExit> {
            self.launched.lock().expect("lock").push(cmd.clone());
            let mut body = std::fs::read_to_string(&self.path).unwrap();
            body.push_str("\n\nedited-marker\n");
            std::fs::write(&self.path, &body).unwrap();
            Ok(EditorExit { success: false })
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("README.md"), "# Hi\n\nbody\n").unwrap();
    let mut app = App::new(root).unwrap();
    let path = crate::tui::editor::page_abs_path(app.provider.root(), Path::new("README.md"));
    let log = Arc::new(Mutex::new(Vec::new()));
    app.editor = Box::new(SaveThenFail {
        path: path.clone(),
        launched: log.clone(),
    });
    app.open_in_editor_with(Some("code --wait".into()));
    let cmds = log.lock().unwrap();
    assert_eq!(cmds[0].program, "code");
    assert_eq!(cmds[0].args[0], "--wait");
    assert_eq!(cmds[0].args[1], "-g");
    assert!(
        cmds[0].args[2].ends_with("README.md:1"),
        "path:line = {}",
        cmds[0].args[2]
    );
    drop(cmds);
    let after = app.doc.lines().join("\n");
    assert!(
        after.contains("edited-marker"),
        "page should reload saved content, got {after:?}"
    );
    assert!(
        app.message.contains("non-zero"),
        "expected non-zero message, got {:?}",
        app.message
    );
}

#[test]
fn open_in_editor_fresh_parse_sees_new_heading_before_reindex() {
    use crate::tui::editor::{EditorCmd, EditorExit, EditorLauncher};

    struct AppendHeading {
        path: PathBuf,
    }
    impl EditorLauncher for AppendHeading {
        fn launch(&self, _cmd: &EditorCmd) -> std::io::Result<EditorExit> {
            let mut body = std::fs::read_to_string(&self.path).unwrap();
            body.push_str("\n## Brand New Heading\n\npara\n");
            std::fs::write(&self.path, body).unwrap();
            Ok(EditorExit { success: true })
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("README.md"), "# Hi\n\nbody\n").unwrap();
    let mut app = App::new(root).unwrap();
    app.update(Action::ToggleViewMode); // raw — headings come from indexed Page when stale
    assert!(matches!(app.doc, crate::tui::page_doc::PageDoc::Raw(_)));
    let before = app.doc.heading_lines();
    let path = crate::tui::editor::page_abs_path(app.provider.root(), Path::new("README.md"));
    app.editor = Box::new(AppendHeading { path });
    app.open_in_editor_with(Some("nvim".into()));
    let after = app.doc.heading_lines();
    assert!(
        after.len() > before.len(),
        "expected fresh parse to see new heading; before={before:?} after={after:?}"
    );
    assert!(
        app.doc
            .lines()
            .iter()
            .any(|l| l.contains("Brand New Heading")),
        "raw lines should include new heading text"
    );
}

#[derive(Default)]
struct LogOpener(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

impl crate::tui::opener::Opener for LogOpener {
    fn open(&self, url: &str) -> std::io::Result<()> {
        self.0.lock().unwrap().push(url.to_owned());
        Ok(())
    }
}

#[test]
fn external_confirm_opens_with_recording_opener() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    app.opener = Box::new(LogOpener(log.clone()));
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("README.md"),
    }));
    app.follow_link_target("https://example.com/wiki");
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Confirm);
    app.update(Action::ConfirmOpen);
    assert_eq!(
        log.lock().unwrap().as_slice(),
        &["https://example.com/wiki".to_owned()]
    );
}

#[test]
fn unsupported_scheme_never_opens() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    app.opener = Box::new(LogOpener(log.clone()));
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("README.md"),
    }));
    app.follow_link_target("chatgpt-conversation://abc");
    assert_ne!(app.input_mode, crate::tui::keymap::InputMode::Confirm);
    assert!(app.pending_external.is_none());
    assert!(
        app.message.contains("unsupported link scheme"),
        "message={}",
        app.message
    );
    app.update(Action::ConfirmOpen);
    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn page_removed_placeholder_survives_keypress() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.apply_effects(vec![wiki_reader_core::nav::Effect::PageRemoved]);
    assert!(app.page_missing);
    assert_eq!(app.message, "page removed");
    let body = app.doc.lines().join("\n");
    assert!(body.to_lowercase().contains("page removed"), "{body}");
    app.update(Action::ViewerDown);
    assert!(
        app.page_missing,
        "page_missing must stick across cursor moves"
    );
    assert_eq!(app.message, "page removed");
    assert!(
        app.doc
            .lines()
            .join("\n")
            .to_lowercase()
            .contains("page removed")
    );
}

#[test]
fn dirty_during_rebuild_queues_second() {
    use std::sync::mpsc;
    use wiki_reader_core::Index;

    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let (tx, rx) = mpsc::channel();
    app.rebuild_rx = Some(rx);
    app.note_watcher_dirty(true);
    assert!(app.rebuild_pending, "dirty while rebuilding must queue");
    assert!(
        app.rebuild_rx.is_some(),
        "must not spawn a second channel yet"
    );

    let index = Index::build(&app.provider).unwrap();
    tx.send(Ok(index)).unwrap();
    app.poll_watcher();
    assert!(
        !app.rebuild_pending,
        "pending flag cleared when second rebuild starts"
    );
    assert!(
        app.rebuild_rx.is_some(),
        "second rebuild must start after the first finishes"
    );
    if let Some(rx) = app.rebuild_rx.take() {
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
    }
}

#[test]
fn search_matches_survive_toggle_and_resize() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    app.update(Action::SearchToggleMode); // Text
    for c in "token".chars() {
        app.update(Action::SearchChar(c));
    }
    assert!(app.search.as_ref().is_some_and(|s| !s.text_hits.is_empty()));
    app.update(Action::SearchActivate);
    assert!(!app.search_matches.is_empty());
    let sources = app.search_matches.clone();
    let idx = app.search_match_idx;
    let before = app.search_matches[idx];

    app.update(Action::ToggleViewMode);
    assert_eq!(app.search_matches, sources, "source matches must persist");
    assert_eq!(app.search_match_idx, idx);
    assert_eq!(
        app.match_highlight,
        Some(app.doc.display_cursor(before)),
        "highlight remapped to display after toggle"
    );
    app.update(Action::SearchNextMatch);
    let after_n = app.search_matches[app.search_match_idx];
    assert_eq!(
        app.match_highlight,
        Some(app.doc.display_cursor(after_n)),
        "n lands on remapped display line"
    );

    // Resize re-layout keeps source matches.
    app.layout_width = 0;
    app.ensure_layout_width(40);
    assert_eq!(app.search_matches, sources);
    assert_eq!(
        app.match_highlight,
        Some(
            app.doc
                .display_cursor(app.search_matches[app.search_match_idx])
        )
    );
}

#[test]
fn search_matches_dedupe_collapsed_display_lines() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    // Force two source lines that map to the same display line.
    app.search_matches = vec![0, 0];
    app.search_match_page = Some(app.navigator.tab().current().page.clone());
    app.store_search_matches(vec![5, 6, 6, 7], 5);
    let displays: Vec<u32> = app
        .search_matches
        .iter()
        .map(|&s| app.doc.display_cursor(s))
        .collect();
    assert!(
        displays.windows(2).all(|w| w[0] != w[1]),
        "consecutive identical display lines must be deduped: {displays:?}"
    );
}

#[test]
fn reindex_anchored_keeps_cursor_via_scroll_none() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("README.md"),
    }));
    let max = u32::try_from(app.doc.lines().len().saturating_sub(1)).unwrap_or(0);
    app.cursor_line = app.cursor_line.saturating_add(5).min(max);
    let view = app.view_state();
    let saved_source = view.cursor_line;
    let index = app.navigator.index().clone();
    let effects = app.navigator.reindex(index, view);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, wiki_reader_core::nav::Effect::ScrollTo(None))),
        "{effects:?}"
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, wiki_reader_core::nav::Effect::ScrollTo(Some(_))))
    );
    app.apply_effects(effects);
    assert_eq!(
        app.navigator.tab().current().cursor_line,
        saved_source,
        "location keeps saved source cursor"
    );
    assert_eq!(
        app.cursor_line,
        app.doc.display_cursor(saved_source),
        "viewer restores from saved source cursor"
    );
}

#[test]
fn search_n_n_wrap_and_highlight() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    app.update(Action::SearchToggleMode); // Text
    for c in "token".chars() {
        app.update(Action::SearchChar(c));
    }
    assert!(
        app.search.as_ref().is_some_and(|s| !s.text_hits.is_empty()),
        "expected text hits"
    );
    app.update(Action::SearchActivate);
    assert!(!app.search_matches.is_empty());
    assert!(app.match_highlight.is_some());
    let n = app.search_matches.len();
    assert!(app.message.contains(&format!("1/{n}")) || app.message.contains(&format!("/{n}")));
    // Wrap around with n/N
    for _ in 0..n {
        app.update(Action::SearchNextMatch);
    }
    assert_eq!(app.search_match_idx, 0);
    app.update(Action::SearchPrevMatch);
    assert_eq!(app.search_match_idx, n.saturating_sub(1));
    // Cursor move clears highlight
    app.update(Action::ViewerDown);
    assert!(app.match_highlight.is_none());
    assert!(app.search_matches.is_empty());
}

#[test]
fn search_click_result_and_outside() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let prev_cursor = app.cursor_line;
    app.update(Action::OpenSearch);
    for c in "token".chars() {
        app.update(Action::SearchChar(c));
    }
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw(f, &mut app)).unwrap();
    let result_hit = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::SearchResult(0)));
    assert!(result_hit.is_some(), "expected SearchResult hit");
    // Click outside closes and restores cursor.
    app.update(Action::CloseSearch);
    assert!(app.search.is_none());
    assert_eq!(app.cursor_line, prev_cursor);
}

#[test]
fn search_pages_projection_token_opens_tokens() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    for c in "projection token".chars() {
        app.update(Action::SearchChar(c));
    }
    let hits = app.search.as_ref().map(|s| s.page_hits.clone()).unwrap();
    assert!(
        hits.iter()
            .any(|h| h.page.relative_path.to_string_lossy().contains("tokens")),
        "{hits:?}"
    );
    app.update(Action::SearchActivate);
    assert!(
        app.navigator
            .tab()
            .current()
            .page
            .relative_path
            .to_string_lossy()
            .contains("tokens")
    );
}

#[test]
fn block_action_toggle_frontmatter() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let items = app.focus_list();
    let fm = items
        .iter()
        .position(|it| it.kind == FocusTarget::BlockAction)
        .expect("frontmatter block action");
    let joined = app.doc.lines().join("\n");
    assert!(
        joined.contains("frontmatter ▶"),
        "expected collapsed cue: {joined}"
    );
    app.focused_item = Some(fm);
    app.update(Action::ViewerActivate);
    assert!(
        !app.expanded_blocks.is_empty(),
        "toggle should expand a block"
    );
    let joined = app.doc.lines().join("\n");
    assert!(
        joined.contains("title:"),
        "expanded frontmatter should show fields: {joined}"
    );
}

#[test]
fn for_tests_starts_on_collection_root_not_saved_session() {
    let root = fixture();
    let app = App::for_tests(&root).unwrap();
    // Worked-example starts at README.md regardless of any real XDG session.
    assert!(
        app.navigator
            .tab()
            .current()
            .page
            .relative_path
            .ends_with("README.md"),
        "got {:?}",
        app.navigator.tab().current().page.relative_path
    );
}

#[test]
fn block_action_copy_code_via_clipboard() {
    use crate::tui::clipboard::RecordingClipboard;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("wiki-reader-ba-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("README.md"),
        "---\ntitle: Copy\n---\n\n```\nsecret-payload\n```\n",
    )
    .unwrap();
    let mut app = App::new(&dir).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);

    let items = app.focus_list();
    let copy = items
        .iter()
        .position(|it| {
            it.kind == FocusTarget::BlockAction
                && app
                    .doc
                    .lines()
                    .get(it.line.unwrap_or(0) as usize)
                    .is_some_and(|l| l.contains("```"))
        })
        .expect("copy code action");
    app.focused_item = Some(copy);
    app.update(Action::ViewerActivate);
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied, vec!["secret-payload".to_owned()]);
    assert_eq!(app.message, "sent to clipboard (OSC 52)");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn new_tab_doubles_tabs_close_last_refused() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    assert_eq!(app.navigator.tab_count(), 1);
    app.update(Action::CloseTab);
    assert_eq!(app.navigator.tab_count(), 1);
    assert!(app.message.contains("last tab"));

    app.cursor_line = 5;
    app.scroll = 2;
    let saved = app.view_state();
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    assert_eq!(app.navigator.active(), 1);
    // New tab starts at top; outgoing tab kept its view in history.
    assert_eq!(
        app.navigator.tabs()[0].current().cursor_line,
        saved.cursor_line
    );
    assert_eq!(app.navigator.tabs()[0].current().scroll, saved.scroll);
}

#[test]
fn switch_tab_restores_cursors() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.cursor_line = 7;
    app.scroll = 3;
    let saved = app.view_state();
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    assert_eq!(
        app.navigator.tabs()[0].current().cursor_line,
        saved.cursor_line,
        "outgoing tab must keep source cursor"
    );
    // Move around on the new tab (pick a real content line, not a block gap).
    let tab1_line = app
        .doc
        .lines()
        .iter()
        .position(|l| !l.trim().is_empty())
        .map_or(0, |i| u32::try_from(i).unwrap_or(0));
    app.cursor_line = tab1_line;
    app.scroll = 0;
    let tab1 = app.view_state();
    app.update(Action::SwitchTab(0));
    assert_eq!(app.navigator.active(), 0);
    assert_eq!(app.navigator.tab().current().cursor_line, saved.cursor_line);
    assert_eq!(
        app.cursor_line,
        app.doc.display_cursor(saved.cursor_line),
        "display remap for cursor"
    );
    assert_eq!(
        app.scroll,
        app.doc.display_cursor(saved.scroll),
        "display remap for scroll"
    );
    app.update(Action::SwitchTab(1));
    assert_eq!(app.navigator.active(), 1);
    assert_eq!(app.cursor_line, app.doc.display_cursor(tab1.cursor_line));
}

#[test]
fn tab_bar_hit_by_coordinate() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    let _ = draw_app(&mut app, 120, 24);
    let tab_hit = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Tab(0)));
    let Some((rect, _)) = tab_hit else {
        panic!("expected Hit::Tab(0) after NewTab");
    };
    // Padded label: 1 + stem + 1.
    assert!(rect.width >= 3, "padded tab hit width, got {}", rect.width);
    assert_eq!(app.hit_map.hit_at(rect.x, rect.y), Some(&Hit::Tab(0)));
    assert_eq!(
        app.hit_map.hit_at(rect.x + rect.width - 1, rect.y),
        Some(&Hit::Tab(0))
    );
    let close_x = rect.x + rect.width;
    assert_eq!(app.hit_map.hit_at(close_x, rect.y), Some(&Hit::TabClose(0)));
    app.update(Action::SwitchTab(0));
    assert_eq!(app.navigator.active(), 0);
}

#[test]
fn middle_click_link_opens_new_tab() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let link = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Link(_)))
        .expect("link hit");
    let (col, row) = (link.0.x, link.0.y);
    let before = app.navigator.tab_count();
    let action = apply_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Middle),
            column: col,
            row,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        },
    );
    assert_eq!(action, None);
    assert_eq!(app.navigator.tab_count(), before + 1);
}

fn middle_at(app: &mut App, x: u16, y: u16) {
    let _ = apply_mouse(
        app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Middle),
            column: x,
            row: y,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        },
    );
}

#[test]
fn t_on_focused_link_opens_target_tab() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let items = app.focus_list();
    let link_idx = items
        .iter()
        .position(|it| it.kind == FocusTarget::Link)
        .expect("body link");
    let target_raw = items[link_idx].target.clone();
    app.update(Action::FocusViewer);
    app.focused_item = Some(link_idx);
    let from = app.navigator.tab().current().page.clone();
    let expected = wiki_reader_core::nav::resolve(&target_raw, &from, app.navigator.index());
    let wiki_reader_core::nav::Target::Page(expected_key, _) = expected.target else {
        panic!("expected page target for {target_raw}");
    };
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    assert_eq!(app.navigator.tab().current().page, expected_key);
    assert_eq!(app.navigator.tab().history.len(), 1);
}

#[test]
fn t_on_nav_row_opens_that_page_tab() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let target = PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    };
    app.update(Action::FocusNav);
    app.navigator.set_nav_cursor(NodeId::Page(target.clone()));
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    assert_eq!(app.navigator.tab().current().page, target);
}

#[test]
fn t_without_focus_duplicates_current_page() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusViewer);
    app.focused_item = None;
    let cur = app.navigator.tab().current().page.clone();
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), 2);
    assert_eq!(app.navigator.tab().current().page, cur);
}

#[test]
fn ordinary_navigation_does_not_create_tabs() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    }));
    assert_eq!(app.navigator.tab_count(), 1);
    app.update(Action::NextPage);
    assert_eq!(app.navigator.tab_count(), 1);
    app.update(Action::Back);
    assert_eq!(app.navigator.tab_count(), 1);
}

#[test]
fn middle_click_nav_breadcrumb_prev_next_search_open_new_tab() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);

    // Nav page row
    let target = PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/README.md"),
    };
    let (nx, ny) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::NavItem(NodeId::Page(k)) if k == &target))
        .map(|(r, _)| (r.x, r.y))
        .expect("nav page hit");
    let before = app.navigator.tab_count();
    middle_at(&mut app, nx, ny);
    assert_eq!(app.navigator.tab_count(), before + 1);
    assert_eq!(app.navigator.tab().current().page, target);

    // Breadcrumb
    let (cx, cy, crumb_key) = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::Breadcrumb(k) => Some((r.x, r.y, k.clone())),
            _ => None,
        })
        .expect("breadcrumb");
    let before = app.navigator.tab_count();
    middle_at(&mut app, cx, cy);
    assert_eq!(app.navigator.tab_count(), before + 1);
    assert_eq!(app.navigator.tab().current().page, crumb_key);

    // Prev / Next footer
    let _ = draw_app(&mut app, 120, 24);
    if let Some((x, y)) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Next))
        .map(|(r, _)| (r.x, r.y))
    {
        let before = app.navigator.tab_count();
        let cur = app.navigator.tab().current().page.clone();
        let expected = app.navigator.nav().tree.next(&cur);
        middle_at(&mut app, x, y);
        if let Some(expected) = expected {
            assert_eq!(app.navigator.tab_count(), before + 1);
            assert_eq!(app.navigator.tab().current().page, expected);
        }
    }
    let _ = draw_app(&mut app, 120, 24);
    if let Some((x, y)) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Prev))
        .map(|(r, _)| (r.x, r.y))
    {
        let before = app.navigator.tab_count();
        let cur = app.navigator.tab().current().page.clone();
        let expected = app.navigator.nav().tree.prev(&cur);
        middle_at(&mut app, x, y);
        if let Some(expected) = expected {
            assert_eq!(app.navigator.tab_count(), before + 1);
            assert_eq!(app.navigator.tab().current().page, expected);
        }
    }

    // Search result
    app.update(Action::OpenSearch);
    for c in "token".chars() {
        app.update(Action::SearchChar(c));
    }
    let _ = draw_app(&mut app, 120, 24);
    let (sx, sy) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::SearchResult(0)))
        .map(|(r, _)| (r.x, r.y))
        .expect("search result hit");
    let expected_page = app
        .search
        .as_ref()
        .and_then(|s| s.page_hits.first())
        .map(|h| h.page.clone())
        .expect("page hit");
    let before = app.navigator.tab_count();
    middle_at(&mut app, sx, sy);
    assert_eq!(app.navigator.tab_count(), before + 1);
    assert_eq!(app.navigator.tab().current().page, expected_page);
}

#[test]
fn new_tab_current_matches_replace_for_same_target() {
    let root = fixture();
    let target = PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    };
    let mut via_tab = App::new(&root).unwrap();
    via_tab.open_page_new_tab(target.clone());
    let mut via_replace = App::new(&root).unwrap();
    via_replace.update(Action::GoToPage(target.clone()));
    assert_eq!(
        via_tab.navigator.tab().current().page,
        via_replace.navigator.tab().current().page
    );
    assert_eq!(via_tab.navigator.tab().current().page, target);
}

#[test]
fn nav_right_on_page_opens_and_focuses_viewer() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("decisions")), true);
    let target = PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("decisions/0001-stack.md"),
    };
    app.update(Action::FocusNav);
    app.navigator.set_nav_cursor(NodeId::Page(target.clone()));
    assert_eq!(app.focus, FocusPane::Nav);
    app.update(Action::NavExpand);
    assert_eq!(app.focus, FocusPane::Viewer);
    assert_eq!(app.navigator.tab().current().page, target);
}

#[test]
fn nav_right_on_expanded_group_steps_to_first_child() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let group = NodeId::Group(PathBuf::from("decisions"));
    app.navigator.set_group_expanded(group.clone(), true);
    app.update(Action::FocusNav);
    app.navigator.set_nav_cursor(group);
    app.update(Action::NavExpand);
    let rows = app.nav_rows();
    let idx = app.nav_cursor_index(&rows).expect("cursor");
    assert!(
        !rows[idx].is_group,
        "should land on first child, got {:?}",
        rows[idx].id
    );
}

#[test]
fn focus_footer_then_enter_keeps_focus_across_pages() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusViewer);
    app.update(Action::FocusFooter);
    let items = app.focus_list();
    let i = app.focused_item.expect("footer focused");
    assert_eq!(items[i].kind, FocusTarget::FooterNext);
    let start = app.navigator.tab().current().page.clone();
    for _ in 0..3 {
        app.update(Action::ViewerActivate);
        let items = app.focus_list();
        let i = app.focused_item.expect("sticky footer");
        assert_eq!(items[i].kind, FocusTarget::FooterNext);
    }
    assert_ne!(app.navigator.tab().current().page, start);
    // Arrow clears sticky.
    app.update(Action::ViewerDown);
    assert_eq!(app.focused_item, None);
    assert_eq!(app.sticky_footer, None);
}

#[test]
fn sticky_footer_falls_back_at_end_of_tree() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    // Walk to the last page via next.
    app.update(Action::FocusViewer);
    app.update(Action::FocusFooter);
    for _ in 0..64 {
        let items = app.focus_list();
        let Some(i) = app.focused_item else { break };
        if items[i].kind != FocusTarget::FooterNext {
            break;
        }
        app.update(Action::ViewerActivate);
    }
    let items = app.focus_list();
    let i = app.focused_item.expect("fallback");
    assert_eq!(
        items[i].kind,
        FocusTarget::FooterPrev,
        "last page should fall back to prev"
    );
}

#[test]
fn mouse_footer_next_keeps_sticky_focus() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    let (x, y) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Next))
        .map(|(r, _)| (r.x, r.y))
        .expect("next hit");
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    if let Some(a) = apply_mouse(&mut app, mouse) {
        app.update(a);
    }
    let items = app.focus_list();
    let i = app.focused_item.expect("sticky after click");
    assert_eq!(items[i].kind, FocusTarget::FooterNext);
}
