use super::draw::draw;
use super::events::apply_mouse;
use super::*;
use crate::tui::action::Action;
use crate::tui::focus::FocusPane;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::{FocusTarget, ViewerDoc};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiki_reader_core::config::NavPosition;
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
fn narrow_nav_overlay_is_opaque_and_outside_clicks_only_dismiss() {
    for width in [40, 60] {
        let mut app = App::new(&fixture()).unwrap();
        let _ = draw_app(&mut app, width, 24);
        app.update(Action::ToggleNav);
        let terminal = draw_app(&mut app, width, 24);
        let rows = screen_rows(&mut app, width, 24);
        insta::assert_snapshot!(format!("nav_overlay_{width}"), rows.join("\n"));
        let regions = crate::tui::layout::split(
            terminal.backend().buffer().area,
            true,
            app.nav_width,
            app.nav_position,
        );
        // The seam beneath the search bar must replace article cells and inherit the theme.
        for x in 1..regions.side_nav.width - 1 {
            let cell = &terminal.backend().buffer()[(x, 3)];
            assert_eq!(cell.symbol(), "─");
            assert_eq!(cell.bg, app.theme.base_style().bg.unwrap());
        }
        // Below the short fixture tree, no article text may survive either.
        for x in 1..regions.side_nav.width - 1 {
            assert_eq!(terminal.backend().buffer()[(x, 7)].symbol(), " ");
        }
        let page = app.navigator.tab().current().page.clone();
        // The layout footer gear normally opens options, but only dismisses the overlay.
        let (x, y) = find_glyph(terminal.backend().buffer(), "⚙").unwrap();
        mouse_at(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
        assert!(!app.nav_visible);
        assert!(app.options.is_none());
        // A queued second press using the stale hit map must not reopen the nav.
        mouse_at(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
        mouse_at(&mut app, MouseEventKind::Up(MouseButton::Left), x, y);
        assert!(!app.nav_visible);
        assert_eq!(app.navigator.tab().current().page, page);
        assert!(!app.selecting);

        app.update(Action::ToggleNav);
        let _ = draw_app(&mut app, width, 24);
        mouse_at(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            width - 3,
            8,
        );
        assert!(!app.nav_visible);
        assert!(!app.selecting);
        assert!(app.pending_link.is_none());
    }
}

#[test]
fn narrow_nav_inherits_theme_and_popups_keep_mouse_precedence() {
    for theme in [Theme::dark(), Theme::light(), Theme::herdr("vesper").0] {
        let mut app = App::new(&fixture()).unwrap();
        app.theme = theme;
        let _ = draw_app(&mut app, 60, 24);
        app.update(Action::ToggleNav);
        let terminal = draw_app(&mut app, 60, 24);
        assert_eq!(terminal.backend().buffer()[(2, 7)].symbol(), " ");
        assert_eq!(
            terminal.backend().buffer()[(2, 7)].bg,
            theme.base_style().bg.unwrap()
        );
        app.update(Action::OpenHelp);
        let _ = draw_app(&mut app, 60, 24);
        mouse_at(&mut app, MouseEventKind::Down(MouseButton::Left), 59, 0);
        assert!(app.help.is_none());
        assert!(app.nav_visible, "popup dismissal must not also dismiss nav");
        let _ = draw_app(&mut app, 60, 24);
        mouse_at(&mut app, MouseEventKind::Down(MouseButton::Middle), 59, 8);
        assert!(!app.nav_visible);
        assert_eq!(app.navigator.tabs().len(), 1);
    }
}

#[test]
fn long_breadcrumbs_keep_header_controls_visible_and_hits_clipped() {
    use wiki_reader_core::nav::Crumb;
    for width in [40, 60, 80, 120] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        let mut hits = HitMap::default();
        let key = PageKey {
            collection_id: "test".into(),
            relative_path: "README.md".into(),
        };
        let crumbs = vec![
            Crumb {
                label: "Root".into(),
                target: Some(key),
            },
            Crumb {
                label: "Very long current title 界".repeat(10),
                target: None,
            },
        ];
        terminal
            .draw(|frame| {
                crate::tui::regions::header::draw(
                    frame,
                    frame.area(),
                    &crumbs,
                    &Theme::dark(),
                    &mut hits,
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        assert!(find_glyph(buf, "⚙").is_none(), "options moved to footer");
        assert_eq!(buf[(width - 4, 0)].symbol(), "◫");
        assert_eq!(buf[(width - 2, 0)].symbol(), "✕");
        assert!(find_glyph(buf, "…").is_some());
        assert!(matches!(hits.hit_at(width - 4, 0), Some(Hit::NavToggle)));
        for (rect, hit) in hits.entries() {
            if matches!(hit, Hit::Breadcrumb(_)) {
                assert!(rect.right() <= width - 5);
            }
        }
    }
}

#[test]
fn snapshots_responsive_widths() {
    let root = fixture();
    for w in [40u16, 60, 80, 120] {
        let out = render_at(&root, w, 24);
        insta::assert_snapshot!(format!("shell_{w}"), out);
    }
    // Tall terminal: vertical gaps above/below header and status.
    let tall = render_at(&root, 120, 40);
    insta::assert_snapshot!("shell_120x40", tall);
}

#[test]
fn filename_mode_only_changes_side_nav_not_header_or_footer() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    let before = screen_rows(&mut app, 120, 24);
    app.navigator
        .set_label_mode(wiki_reader_core::config::LabelMode::Filename);
    let after = screen_rows(&mut app, 120, 24);
    assert_eq!(before[0], after[0], "header labels must not change");
    assert_eq!(before[22], after[22], "footer labels must not change");
    assert!(after[0].contains("Worked Example Wiki"));
    assert!(after[0].contains("Token Projection"));
    assert!(after.iter().any(|row| row.contains("tokens.md")));
    app.navigator
        .set_label_mode(wiki_reader_core::config::LabelMode::Title);
    let restored = screen_rows(&mut app, 120, 24);
    assert_eq!(before[0], restored[0]);
    assert_eq!(before[22], restored[22]);
}

#[test]
fn filename_nav_snapshots_responsive_widths() {
    let mut app = App::new(&fixture()).unwrap();
    app.navigator
        .set_label_mode(wiki_reader_core::config::LabelMode::Filename);
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    app.update(Action::FocusNav);
    for w in [40u16, 60, 80, 120] {
        app.sync_nav_for_width(w);
        app.nav_visible = true;
        app.nav_user_override = true;
        let out = screen_rows(&mut app, w, 24).join("\n");
        insta::assert_snapshot!(format!("filename_nav_{w}"), out);
    }
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
    let regions = layout::split(area, true, None, NavPosition::Left);
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
    let (gx, gy) = find_glyph(buf, "⚙").expect("⚙");
    assert_eq!(app.hit_map.hit_at(tx, ty), Some(&Hit::NavToggle));
    assert_eq!(app.hit_map.hit_at(qx, qy), Some(&Hit::Quit));
    assert_eq!(app.hit_map.hit_at(gx, gy), Some(&Hit::OpenOptions));
    // Padded: icons sit one col inset from the raw edge.
    assert_eq!(qx, 120 - 2, "✕ at chrome_pad right edge");
    assert_eq!(tx, 120 - 4, "◫ two cols left of ✕");
    assert_eq!((gx, gy), (120 - 2, 23), "⚙ at layout footer right edge");
    assert_eq!(buf[(gx - 2, gy)].symbol(), "?");
    assert_eq!(app.hit_map.hit_at(gx - 2, gy), Some(&Hit::OpenHelp));
    assert!(
        find_glyph(buf, "○").is_none(),
        "no eye toggle in the header"
    );

    // Nav search row is the first inner row (header row 0, nav border row 1).
    let search = (0..buf.area.width)
        .find(|&x| buf[(x, 2)].symbol() == "/")
        .expect("search glyph");
    assert_eq!(
        app.hit_map.hit_at(search, 2),
        Some(&Hit::NavSearchRow),
        "search hit on padded chrome row"
    );
}

#[test]
fn footer_buttons_open_popups_on_both_nav_sides_and_responsive_widths() {
    for position in [NavPosition::Left, NavPosition::Right] {
        for width in [40, 60, 80, 120] {
            for (hit, glyph) in [(Hit::OpenHelp, "?"), (Hit::OpenOptions, "⚙")] {
                let mut app = App::new(&fixture()).unwrap();
                app.nav_position = position;
                let terminal = draw_app(&mut app, width, 24);
                let (rect, _) = app
                    .hit_map
                    .entries()
                    .iter()
                    .find(|(_, candidate)| *candidate == hit)
                    .expect("footer control hit");
                let (x, y) = (rect.x, rect.y);
                assert_eq!(y, 23);
                assert_eq!(x, width - if hit == Hit::OpenHelp { 4 } else { 2 });
                assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), glyph);
                mouse_at(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
                if hit == Hit::OpenHelp {
                    let help = app.help.as_ref().expect("help opens");
                    let row = help
                        .rows
                        .iter()
                        .find(|row| row.action == Some(Action::OpenHelp))
                        .unwrap();
                    assert_eq!(row.icon, Some("?"));
                } else {
                    assert!(app.options.is_some());
                }
            }
        }
    }
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
            NavPosition::Left,
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
    // Find the clickable architecture crumb (Filename mode: folder name, not "Readme").
    let mut ax = None;
    for x in 0..buf.area.width {
        let end = (x + 12).min(buf.area.width);
        let word: String = (x..end).map(|xx| buf[(xx, 0)].symbol()).collect();
        if word == "architecture"
            && let Some(Hit::Breadcrumb(k)) = app.hit_map.hit_at(x, 0)
            && k.relative_path.ends_with("architecture/README.md")
        {
            ax = Some(x);
            break;
        }
    }
    let ax = ax.expect("architecture crumb glyph");
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
            app.nav_width,
            NavPosition::Left,
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
        NavPosition::Left,
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
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Page(current.clone())),
        "nav highlight follows the page even while the viewer is focused"
    );
    app.update(Action::FocusNav);
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Page(current)),
        "and stays on it when the nav gains focus"
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
    let _ = highlight_markdown("warm", "base16-ocean.dark");
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
    let _ = highlight_markdown("warm", "base16-ocean.dark");
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
    let _ = highlight_markdown("warm", "base16-ocean.dark");
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
    let start_page = app.navigator.tab().current().page.clone();
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
    // Press starts a selection; the link is followed on release (no drag).
    let action = action.expect("mouse action");
    assert!(matches!(action, Action::SelectStart(..)));
    app.update(action);
    assert_eq!(
        app.navigator.tab().current().page,
        start_page,
        "not followed on press"
    );
    let up = apply_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row: link_rect.y,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        },
    )
    .expect("release action");
    assert!(matches!(up, Action::SelectEnd));
    app.update(up);
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("architecture/design-system/tokens.md")
    );
}

fn mouse_at(app: &mut App, kind: MouseEventKind, x: u16, y: u16) {
    let ev = MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    if let Some(a) = apply_mouse(app, ev) {
        app.update(a);
    }
}

/// Screen cell of document (line, col) after a draw.
fn cell_xy(app: &App, line: u32, col: u16) -> (u16, u16) {
    let g = app.viewer_geom;
    (
        g.text_x + col,
        g.top_y + u16::try_from(line - app.scroll).unwrap(),
    )
}

fn drag_select(app: &mut App, from: (u32, u16), to: (u32, u16)) {
    let (x0, y0) = cell_xy(app, from.0, from.1);
    let (x1, y1) = cell_xy(app, to.0, to.1);
    mouse_at(app, MouseEventKind::Down(MouseButton::Left), x0, y0);
    mouse_at(app, MouseEventKind::Drag(MouseButton::Left), x1, y1);
    mouse_at(app, MouseEventKind::Up(MouseButton::Left), x1, y1);
}

fn app_with_page(md: &str) -> (tempfile::TempDir, App, Arc<std::sync::Mutex<Vec<String>>>) {
    use crate::tui::clipboard::RecordingClipboard;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), md).unwrap();
    let mut app = App::new(dir.path()).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    (dir, app, log)
}

fn row_of(app: &App, needle: &str) -> u32 {
    u32::try_from(
        app.doc
            .lines()
            .iter()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no row containing {needle:?}")),
    )
    .unwrap()
}

fn last_col(app: &App, line: u32) -> u16 {
    crate::tui::text_col::clamp_col(&app.doc.lines()[line as usize], u16::MAX)
}

const SELECT_MD: &str = "# Title\n\nFirst paragraph that is long enough to wrap across several rows of the pane when drawn at a narrow width so the copy has to rejoin the soft wrapped rows.\n\n| A | B |\n|---|---|\n| one | two |\n| three | four |\n\n```\ncode line 1\ncode line 2\n```\n";

#[test]
fn drag_select_copies_a_wrapped_paragraph_as_one_line() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    let _ = draw_app(&mut app, 60, 40);
    let first = row_of(&app, "First paragraph");
    let last = row_of(&app, "wrapped rows.");
    assert!(last > first, "paragraph must wrap at this width");
    let end = last_col(&app, last);
    drag_select(&mut app, (first, 0), (last, end));
    let copied = log.lock().unwrap().clone();
    assert_eq!(
        copied,
        [
            "First paragraph that is long enough to wrap across several rows of the pane when drawn at a narrow width so the copy has to rejoin the soft wrapped rows."
        ]
    );
    assert!(app.message.contains("copied"), "{}", app.message);
}

#[test]
fn drag_select_partial_row_copies_exact_cells() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    let _ = draw_app(&mut app, 60, 40);
    let first = row_of(&app, "First paragraph");
    // Cells 6..=14 of "First paragraph ..." are "paragraph".
    drag_select(&mut app, (first, 6), (first, 14));
    assert_eq!(log.lock().unwrap().clone(), ["paragraph"]);
    // Dragging backwards selects the same cells.
    log.lock().unwrap().clear();
    drag_select(&mut app, (first, 14), (first, 6));
    assert_eq!(log.lock().unwrap().clone(), ["paragraph"]);
}

#[test]
fn drag_select_table_copies_cells_not_borders() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    let _ = draw_app(&mut app, 60, 40);
    let head = row_of(&app, "│ A");
    let tail = row_of(&app, "three");
    let end = last_col(&app, tail);
    drag_select(&mut app, (head, 0), (tail, end));
    assert_eq!(log.lock().unwrap().clone(), ["A\tB\none\ttwo\nthree\tfour"]);
}

const WRAP_TABLE_MD: &str = "# T\n\n| Name | Note | Kind |\n|---|---|---|\n| alpha |  | x |\n| beta | a long note that has to wrap inside its narrow column | y |\n| gamma | short | z |\n";

fn select_all_table(app: &mut App) {
    let head = row_of(app, "│ Name");
    let tail = row_of(app, "gamma");
    let end = last_col(app, tail);
    drag_select(app, (head, 0), (tail, end));
}

#[test]
fn drag_select_table_keeps_empty_cells_aligned() {
    let (_d, mut app, log) = app_with_page(WRAP_TABLE_MD);
    let _ = draw_app(&mut app, 100, 40);
    select_all_table(&mut app);
    let copied = log.lock().unwrap().clone();
    assert_eq!(
        copied,
        [
            "Name\tNote\tKind\nalpha\t\tx\nbeta\ta long note that has to wrap inside its narrow column\ty\ngamma\tshort\tz"
        ]
    );
}

#[test]
fn drag_select_table_rejoins_a_wrapped_cell() {
    let (_d, mut app, log) = app_with_page(WRAP_TABLE_MD);
    let _ = draw_app(&mut app, 40, 40);
    let beta = row_of(&app, "beta");
    let gamma = row_of(&app, "gamma");
    assert!(gamma - beta > 1, "the note cell must wrap at this width");
    select_all_table(&mut app);
    let copied = log.lock().unwrap().clone();
    assert_eq!(
        copied,
        [
            "Name\tNote\tKind\nalpha\t\tx\nbeta\ta long note that has to wrap inside its narrow column\ty\ngamma\tshort\tz"
        ]
    );
}

#[test]
fn drag_select_table_partial_cell_copies_only_the_selected_text() {
    let (_d, mut app, log) = app_with_page(WRAP_TABLE_MD);
    let _ = draw_app(&mut app, 100, 40);
    let alpha = row_of(&app, "alpha");
    // Cells 3..=5 of "│ alpha │ ..." are "lph".
    drag_select(&mut app, (alpha, 3), (alpha, 5));
    assert_eq!(log.lock().unwrap().clone(), ["lph"]);
}

#[test]
fn held_drag_past_the_bottom_edge_keeps_scrolling_on_idle_ticks() {
    let md = "line\n\n".repeat(80);
    let (_d, mut app, _log) = app_with_page(&md);
    let _ = draw_app(&mut app, 80, 20);
    let g = app.viewer_geom;
    let (x, y) = cell_xy(&app, 0, 0);
    mouse_at(&mut app, MouseEventKind::Down(MouseButton::Left), x, y);
    // One drag event below the pane, then the pointer stays still.
    let below = g.top_y + g.rows + 2;
    mouse_at(&mut app, MouseEventKind::Drag(MouseButton::Left), x, below);
    let after_drag = app.scroll;
    let head_after_drag = app.selection.unwrap().head.line;
    for _ in 0..5 {
        app.drag_autoscroll();
    }
    assert_eq!(app.scroll, after_drag + 5, "one row per idle tick");
    assert!(app.selection.unwrap().head.line > head_after_drag);
    // Inside the pane, ticks do nothing; releasing ends the drag.
    let (_, inside) = cell_xy(&app, app.scroll, 0);
    mouse_at(&mut app, MouseEventKind::Drag(MouseButton::Left), x, inside);
    let still = app.scroll;
    app.drag_autoscroll();
    assert_eq!(app.scroll, still);
    mouse_at(&mut app, MouseEventKind::Up(MouseButton::Left), x, inside);
    assert!(app.drag_at.is_none());
}

#[test]
fn drag_select_code_drops_the_gutter() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    let _ = draw_app(&mut app, 60, 40);
    let one = row_of(&app, "code line 1");
    let two = row_of(&app, "code line 2");
    let end = last_col(&app, two);
    drag_select(&mut app, (one, 0), (two, end));
    assert_eq!(log.lock().unwrap().clone(), ["code line 1\ncode line 2"]);
}

#[test]
fn drag_select_backlink_drops_side_borders() {
    use crate::tui::clipboard::RecordingClipboard;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("src.md"),
        "---\ntitle: Src Title\nsummary: A short summary line.\n---\n\n# Src\n\nSee [dst](dst.md).\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("dst.md"), "# Dst\n\nBody.\n").unwrap();
    let mut app = App::new(dir.path()).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.tab().current().page.collection_id.clone(),
        relative_path: PathBuf::from("dst.md"),
    }));
    let _ = draw_app(&mut app, 80, 40);
    let title = row_of(&app, "Src Title");
    let end = last_col(&app, title);
    drag_select(&mut app, (title, 0), (title, end));
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied.len(), 1, "{copied:?}");
    assert!(
        !copied[0].contains('│'),
        "side borders stripped: {:?}",
        copied[0]
    );
    assert_eq!(
        copied[0].trim_end(),
        copied[0].as_str(),
        "no trailing pad spaces: {:?}",
        copied[0]
    );
    assert!(
        copied[0].contains("Src Title"),
        "title kept: {:?}",
        copied[0]
    );
}

#[test]
fn drag_select_in_raw_view_copies_source_text() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    app.update(Action::ToggleViewMode);
    let _ = draw_app(&mut app, 60, 40);
    let title = row_of(&app, "# Title");
    drag_select(&mut app, (title, 2), (title, 6));
    assert_eq!(log.lock().unwrap().clone(), ["Title"]);
}

#[test]
fn selection_and_cursor_cell_are_painted() {
    use ratatui::style::Modifier;
    for raw in [false, true] {
        let (_d, mut app, _log) = app_with_page(SELECT_MD);
        if raw {
            app.update(Action::ToggleViewMode);
        }
        let _ = draw_app(&mut app, 60, 40);
        let first = row_of(&app, if raw { "# Title" } else { "Title" });
        drag_select(&mut app, (first, 2), (first, 4));
        let terminal = draw_app(&mut app, 60, 40);
        let buf = terminal.backend().buffer();
        let sel_bg = app.theme.selection;
        let (x, y) = cell_xy(&app, first, 0);
        let bg = |c: u16| buf[(x + c, y)].bg;
        assert_ne!(bg(1), sel_bg, "raw={raw}: before the selection");
        for c in 2..=3 {
            assert_eq!(bg(c), sel_bg, "raw={raw}: col {c} selected");
        }
        assert_ne!(bg(5), sel_bg, "raw={raw}: after the selection");
        // The cursor (at the drag head, col 4) inverts the selected cell
        // without discarding the selection background.
        let cursor = &buf[(x + 4, y)];
        assert!(
            cursor.modifier.contains(Modifier::REVERSED),
            "raw={raw}: cursor cell"
        );
        assert_eq!(
            cursor.bg, sel_bg,
            "raw={raw}: cursor preserves the selection background"
        );
        app.update(Action::FocusNav);
        let terminal = draw_app(&mut app, 60, 40);
        assert!(
            !terminal.backend().buffer()[(x + 4, y)]
                .modifier
                .contains(Modifier::REVERSED),
            "raw={raw}: no cursor cell when the nav has focus"
        );
    }
}

#[test]
fn bare_click_and_new_press_clear_the_selection() {
    let (_d, mut app, log) = app_with_page(SELECT_MD);
    let _ = draw_app(&mut app, 60, 40);
    let first = row_of(&app, "First paragraph");
    drag_select(&mut app, (first, 0), (first, 4));
    assert!(app.selection.is_some());
    // Plain click elsewhere: no new copy, selection gone, cursor placed.
    let before = log.lock().unwrap().len();
    drag_select(&mut app, (first, 8), (first, 8));
    assert!(app.selection.is_none());
    assert_eq!(log.lock().unwrap().len(), before);
    assert_eq!((app.cursor_line, app.effective_col()), (first, 8));
    // Keyboard movement also drops a selection.
    drag_select(&mut app, (first, 0), (first, 4));
    app.update(Action::ViewerDown);
    assert!(app.selection.is_none());
}

#[test]
fn drag_from_a_link_selects_instead_of_following() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 40);
    let (rect, _) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, h)| matches!(h, Hit::Link(_)))
        .map(|(r, h)| (*r, h.clone()))
        .expect("a link");
    let page = app.navigator.tab().current().page.clone();
    mouse_at(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        rect.x,
        rect.y,
    );
    mouse_at(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        rect.x + rect.width.min(3),
        rect.y + 1,
    );
    mouse_at(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        rect.x + rect.width.min(3),
        rect.y + 1,
    );
    assert_eq!(
        app.navigator.tab().current().page,
        page,
        "stayed on the page"
    );
    assert!(app.selection.is_some_and(|s| !s.is_empty()));
}

#[test]
fn nav_on_the_right_draws_after_the_viewer() {
    let (mut app, _tmp) = app_with_config("[nav]\nposition = \"right\"\n");
    assert_eq!(app.nav_position, NavPosition::Right);
    let _ = draw_app(&mut app, 120, 24);
    let regions = crate::tui::layout::split(
        ratatui::layout::Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 24,
        },
        true,
        app.nav_width,
        app.nav_position,
    );
    assert!(regions.viewer.x < regions.side_nav.x);
    assert_eq!(regions.viewer.x, 0);
}

#[test]
fn viewer_right_at_end_focuses_nav_when_nav_is_right() {
    let (mut app, _tmp) = app_with_config("[nav]\nposition = \"right\"\n");
    let _ = draw_app(&mut app, 100, 40);
    let line = row_of(&app, "Worked Example Wiki");
    app.update(Action::SetCursorLine(line));
    assert_eq!(app.focus, FocusPane::Viewer);
    // Walk to the end of the row, then one more Right hands focus to the nav.
    for _ in 0..200 {
        app.update(Action::ViewerRight);
        if app.focus == FocusPane::Nav {
            break;
        }
    }
    assert_eq!(app.focus, FocusPane::Nav);
}

#[test]
fn column_cursor_moves_and_left_at_zero_focuses_nav() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 40);
    let line = row_of(&app, "Worked Example Wiki");
    app.update(Action::SetCursorLine(line));
    assert_eq!(app.effective_col(), 0);
    for _ in 0..3 {
        app.update(Action::ViewerRight);
    }
    assert_eq!(app.effective_col(), 3);
    app.update(Action::ViewerLeft);
    assert_eq!(app.effective_col(), 2);
    app.update(Action::ViewerLeft);
    app.update(Action::ViewerLeft);
    assert_eq!((app.focus, app.effective_col()), (FocusPane::Viewer, 0));
    // At column 0, Left hands focus to the nav...
    app.update(Action::ViewerLeft);
    assert_eq!(app.focus, FocusPane::Nav);
    // ...and both panes remember where they were.
    app.update(Action::FocusViewer);
    assert_eq!((app.cursor_line, app.effective_col()), (line, 0));
}

#[test]
fn column_is_sticky_across_short_rows_and_stops_at_row_end() {
    let (_d, mut app, _log) = app_with_page("# Title\n\nA reasonably long line here\n\nshort\n");
    let _ = draw_app(&mut app, 100, 40);
    let long = row_of(&app, "reasonably");
    app.update(Action::SetCursorLine(long));
    for _ in 0..10 {
        app.update(Action::ViewerRight);
    }
    assert_eq!(app.effective_col(), 10);
    app.update(Action::ViewerDown); // blank row
    assert_eq!(app.effective_col(), 0);
    app.update(Action::ViewerUp);
    assert_eq!(app.effective_col(), 10, "wanted column restored");
    // Right stops on the last char of the row.
    for _ in 0..100 {
        app.update(Action::ViewerRight);
    }
    let last = last_col(&app, long);
    assert_eq!(app.effective_col(), last);
}

#[test]
fn status_bar_shows_line_and_column() {
    let (_d, mut app, _log) = app_with_page("# Title\n\nhello world\n");
    let hello = {
        let _ = draw_app(&mut app, 100, 24);
        row_of(&app, "hello world")
    };
    app.update(Action::SetCursorLine(hello));
    for _ in 0..4 {
        app.update(Action::ViewerRight);
    }
    let terminal = draw_app(&mut app, 100, 24);
    let buf = terminal.backend().buffer();
    let y = buf.area.height - 1;
    let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
    assert!(
        row.contains(&format!("L{}:C5", hello + 1)),
        "status shows 1-based line:col: {row}"
    );
}

#[test]
fn left_at_column_zero_reveals_a_hidden_nav() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    app.update(Action::ToggleNav);
    assert!(!app.nav_visible);
    app.update(Action::ViewerLeft);
    assert!(app.nav_visible);
    assert_eq!(app.focus, FocusPane::Nav);
}

#[test]
fn inline_code_does_not_full_row_shade() {
    let md = "- item with `alpha` here\n- next with `beta` too\n\n```\nfenced\n```\n";
    let (_d, mut app, _) = app_with_page(md);
    let terminal = draw_app(&mut app, 80, 30);
    let buf = terminal.backend().buffer();
    let code_bg = app.theme.code_bg;
    let line = row_of(&app, "alpha");
    let (x0, y) = cell_xy(&app, line, 0);
    // List marker / plain text before the code span must not carry code_bg.
    assert_ne!(
        buf[(x0, y)].bg,
        code_bg,
        "list marker cell must not be full-row shaded"
    );
    // A cell after the closing backtick text on the same row.
    let plain_after = app.doc.lines()[line as usize]
        .find("here")
        .expect("plain after code");
    let (ax, ay) = cell_xy(&app, line, u16::try_from(plain_after).unwrap_or(0));
    assert_ne!(
        buf[(ax, ay)].bg,
        code_bg,
        "plain text after inline code must not be shaded"
    );
    // Fenced code still pads the row with code_bg.
    let fenced = row_of(&app, "fenced");
    let (fx, fy) = cell_xy(&app, fenced, 0);
    let row_w = buf.area.width;
    let mut found_pad = false;
    for x in fx..row_w {
        if buf[(x, fy)].symbol() == " " && buf[(x, fy)].bg == code_bg {
            found_pad = true;
            break;
        }
    }
    assert!(found_pad, "fenced block still full-row shades");
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
    // Tab onto the backlink so the cursor lands on the title row (real path).
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = 0;
    for _ in 0..items.len().saturating_add(2) {
        app.update(Action::ViewerTab);
        if app.focused_item == Some(bl_idx) {
            break;
        }
    }
    assert_eq!(app.focused_item, Some(bl_idx), "Tab reaches backlink");
    let title_line = bl.segments[0].0;
    assert_eq!(
        app.cursor_line, title_line,
        "cursor lands on title after Tab"
    );
    let terminal = draw_app(&mut app, 120, 40);
    let buf = terminal.backend().buffer();
    let sel_bg = app.theme.selection;
    // Teal ▌ replaces the left │ at content col 0.
    let (bx, title_y) = cell_xy(&app, title_line, 0);
    assert!(
        buf[(bx, title_y)].symbol().contains('▌'),
        "teal ▌ on focused backlink title at ({bx},{title_y})"
    );
    assert_eq!(buf[(bx, title_y)].fg, app.theme.link, "▌ uses link teal");
    for &(line, (c0, _)) in &bl.segments {
        let (sx, sy) = cell_xy(&app, line, c0);
        assert_eq!(
            buf[(sx, sy)].bg,
            sel_bg,
            "selection bg on focused entry line {line} col {c0}"
        );
    }
    // Wide view: selection must stop at the 100-col text cap, not fill the margin.
    app.nav_visible = false;
    let terminal = draw_app(&mut app, 160, 40);
    let buf = terminal.backend().buffer();
    let (past_x, past_y) = cell_xy(&app, title_line, 100);
    assert!(
        past_x < buf.area.width,
        "col 100 must be on-screen in a 160-col view"
    );
    assert_ne!(
        buf[(past_x, past_y)].bg,
        sel_bg,
        "selection must not spill past pane text width at ({past_x},{past_y})"
    );
    // Unfocused: title stays link teal, summary is normal text (not link-teal).
    app.focused_item = None;
    // Keep the entry on-screen but cursor off it (cursor_line would recolour the row).
    app.cursor_line = title_line.saturating_sub(3);
    app.scroll = title_line.saturating_sub(5);
    let terminal = draw_app(&mut app, 120, 40);
    let buf = terminal.backend().buffer();
    let (tx, ty) = cell_xy(&app, title_line, bl.segments[0].1.0);
    assert_eq!(
        buf[(tx, ty)].fg,
        app.theme.link,
        "unfocused title is link teal"
    );
    if let Some(&(sum_line, (c0, _))) = bl.segments.get(1) {
        let (sx, sy) = cell_xy(&app, sum_line, c0);
        assert_eq!(
            buf[(sx, sy)].fg,
            app.theme.text,
            "unfocused summary is text colour"
        );
    }
    // Enter on the backlink matches tree-select history shape (replace, one tab).
    app.focused_item = Some(bl_idx);
    app.update(Action::ViewerActivate);
    assert_eq!(
        app.navigator.tab().current().page.relative_path,
        PathBuf::from("architecture/design-system/tokens.md")
    );
    assert_eq!(app.navigator.tab_count(), 1);
}

#[test]
fn focused_block_action_uses_peach_bg_and_dark_text() {
    // Frontmatter / code-title / expand / copy targets: the active tab and footer link
    // colours (dark text on peach), not yellow with white text.
    let md = "---\ntitle: T\n---\n\n# T\n\n```rust\nfn main() {}\n```\n";
    let (_d, mut app, _) = app_with_page(md);
    let _ = draw_app(&mut app, 80, 30);
    let items = app.focus_list();
    let idx = items
        .iter()
        .position(|it| it.kind == FocusTarget::BlockAction)
        .expect("a block action");
    let line = items[idx].line.expect("block action line");
    let col = items[idx].cols.0;
    app.update(Action::FocusViewer);
    for on_cursor_row in [false, true] {
        app.focused_item = Some(idx);
        app.cursor_line = if on_cursor_row { line } else { 0 };
        let terminal = draw_app(&mut app, 80, 30);
        let buf = terminal.backend().buffer();
        let (x, y) = cell_xy(&app, line, col);
        assert_eq!(
            buf[(x, y)].bg,
            app.theme.peach,
            "bg on_cursor_row={on_cursor_row}"
        );
        assert_eq!(
            buf[(x, y)].fg,
            app.theme.on_peach,
            "fg on_cursor_row={on_cursor_row}"
        );
    }
}

#[test]
fn focused_ordinary_link_keeps_focus_bg_on_cursor_row() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# T\n\nSee [here](other.md) please.\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("other.md"), "# Other\n").unwrap();
    let mut app = App::new(dir.path()).unwrap();
    let _ = draw_app(&mut app, 80, 24);
    let link = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.contains("other"))
        .expect("body link")
        .clone();
    let items = app.focus_list();
    let idx = items
        .iter()
        .position(|it| it.link_id == Some(link.id))
        .expect("link in focus list");
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = 0;
    for _ in 0..items.len().saturating_add(2) {
        app.update(Action::ViewerTab);
        if app.focused_item == Some(idx) {
            break;
        }
    }
    assert_eq!(app.focused_item, Some(idx));
    let link_line = link.segments[0].0;
    assert_eq!(app.cursor_line, link_line, "cursor on focused link row");
    let terminal = draw_app(&mut app, 80, 24);
    let buf = terminal.backend().buffer();
    let (x, y) = cell_xy(&app, link_line, link.segments[0].1.0);
    assert_eq!(
        buf[(x, y)].bg,
        app.theme.selection,
        "selection bg survives cursor row"
    );
    assert_eq!(buf[(x, y)].fg, app.theme.link, "focused link keeps teal fg");
    // Teal ▌ in the marker column (one cell left of text_x).
    let marker_x = app.viewer_geom.text_x.saturating_sub(1);
    assert!(
        buf[(marker_x, y)].symbol().contains('▌'),
        "teal ▌ on focused ordinary link marker"
    );
    assert_eq!(buf[(marker_x, y)].fg, app.theme.link, "marker ▌ is teal");
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
    // Raw rows are soft-wrapped at the (narrow, undrawn) layout width; they
    // rejoin to the source line exactly.
    assert!(
        app.doc.lines().concat().contains("## Brand New Heading"),
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
    app.update(Action::SearchToggleMode); // Content
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
        Some(
            app.match_span()
                .map_or_else(|| app.doc.display_cursor(before), |(line, _, _)| line)
        ),
        "highlight remapped to the painted phrase after toggle"
    );
    app.update(Action::SearchNextMatch);
    let after_n = app.search_matches[app.search_match_idx];
    assert_eq!(
        app.match_highlight,
        Some(
            app.match_span()
                .map_or_else(|| app.doc.display_cursor(after_n), |(line, _, _)| line)
        ),
        "n lands on the remapped phrase"
    );

    // Resize re-layout keeps source matches.
    app.layout_width = 0;
    app.ensure_layout_width(40);
    assert_eq!(app.search_matches, sources);
    let source = app.search_matches[app.search_match_idx];
    assert_eq!(
        app.match_highlight,
        Some(
            app.match_span()
                .map_or_else(|| app.doc.display_cursor(source), |(line, _, _)| line)
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
    app.update(Action::SearchToggleMode); // Content
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
fn search_selection_scrolls_into_view_and_jumps() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    // Single-letter query yields many file hits.
    app.update(Action::SearchChar('a'));
    let _ = draw_app(&mut app, 100, 24);
    let n = app.search.as_ref().unwrap().result_len();
    assert!(n > 5, "expected several hits, got {n}");
    let vh = app.search.as_ref().unwrap().list_height;
    // Move past the first viewport.
    for _ in 0..(vh + 2) {
        app.update(Action::SearchSelectDelta(1));
    }
    let overlay = app.search.as_ref().unwrap();
    assert!(
        overlay.selected >= overlay.scroll
            && overlay.selected < overlay.scroll + overlay.list_height.max(1),
        "selected {} not visible in [{}, {})",
        overlay.selected,
        overlay.scroll,
        overlay.scroll + overlay.list_height
    );
    app.update(Action::SearchJump(false));
    let overlay = app.search.as_ref().unwrap();
    assert_eq!(overlay.selected, overlay.result_len() - 1);
    app.update(Action::SearchJump(true));
    assert_eq!(app.search.as_ref().unwrap().selected, 0);
    // A page step is one list height; selection wraps around the result count.
    let overlay = app.search.as_ref().unwrap();
    let (step, n) = (overlay.list_height.max(1), overlay.result_len());
    app.update(Action::SearchPageDelta(1));
    assert_eq!(app.search.as_ref().unwrap().selected, step % n);
}

#[test]
fn search_click_after_scroll_uses_absolute_index() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    app.update(Action::SearchChar('a'));
    let _ = draw_app(&mut app, 100, 24);
    let vh = app.search.as_ref().unwrap().list_height.max(1);
    for _ in 0..=vh {
        app.update(Action::SearchSelectDelta(1));
    }
    let want = app.search.as_ref().unwrap().selected;
    let _ = draw_app(&mut app, 100, 24);
    let abs = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::SearchResult(i) if *i == want => Some((r.x, r.y)),
            _ => None,
        })
        .expect("absolute SearchResult hit after scroll");
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: abs.0,
        row: abs.1,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    if let Some(a) = apply_mouse(&mut app, mouse) {
        app.update(a);
    }
    assert!(app.search.is_none(), "click should activate and close");
}

#[test]
fn search_mode_files_content_header() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenSearch);
    let terminal = draw_app(&mut app, 80, 24);
    let buf = terminal.backend().buffer();
    let mut row = String::new();
    for x in 0..buf.area.width {
        row.push_str(buf[(x, 0)].symbol());
    }
    // Find Files / Content somewhere in the buffer.
    let mut all = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            all.push_str(buf[(x, y)].symbol());
        }
    }
    assert!(all.contains("[Files]"), "title missing mode tag: {all}");
    assert!(all.contains("Tab: toggle mode"), "title missing Tab hint");
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        crate::tui::search_ui::SearchMode::Files
    );
    app.update(Action::SearchToggleMode);
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        crate::tui::search_ui::SearchMode::Content
    );
    let terminal = draw_app(&mut app, 80, 24);
    let buf = terminal.backend().buffer();
    let all: String = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_owned())
        .collect();
    assert!(all.contains("[Content]"), "title follows the mode");
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
        joined.contains("frontmatter ▸"),
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
fn frontmatter_toggle_is_reachable_on_first_tab_and_enter() {
    let root = fixture();
    // First Tab lands on the frontmatter toggle (line 0), not the next item.
    let mut app = App::new(&root).unwrap();
    app.update(Action::ViewerTab);
    let items = app.focus_list();
    let it = &items[app.focused_item.expect("focused after first Tab")];
    assert_eq!(it.kind, FocusTarget::BlockAction);
    app.update(Action::ViewerActivate);
    assert!(!app.expanded_blocks.is_empty());
    assert!(app.doc.lines().join("\n").contains("frontmatter ▾"));

    // Enter with nothing focused, cursor on the toggle line, also toggles.
    let mut app = App::new(&root).unwrap();
    assert_eq!(app.cursor_line, 0);
    app.update(Action::ViewerActivate);
    assert!(!app.expanded_blocks.is_empty());
}

#[test]
fn frontmatter_toggle_is_clickable() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    let (rect, id) = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::Block(id) => Some((*r, *id)),
            _ => None,
        })
        .expect("block hit for frontmatter toggle");
    assert_eq!(app.hit_map.hit_at(rect.x, rect.y), Some(&Hit::Block(id)));
    let click = ratatui::crossterm::event::MouseEvent {
        kind: ratatui::crossterm::event::MouseEventKind::Down(
            ratatui::crossterm::event::MouseButton::Left,
        ),
        column: rect.x,
        row: rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let action = crate::tui::app::events::apply_mouse(&mut app, click).expect("action");
    app.update(action);
    assert!(app.doc.lines().join("\n").contains("frontmatter ▾"));
}

#[test]
fn frontmatter_rules_share_one_width() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::ViewerTab);
    app.update(Action::ViewerActivate);
    let lines = app.doc.lines();
    let top = lines
        .iter()
        .find(|l| l.contains("frontmatter"))
        .expect("top rule");
    let bottom = lines
        .iter()
        .skip_while(|l| !l.contains("frontmatter"))
        .skip(1)
        .find(|l| l.chars().all(|c| c == '─') && !l.is_empty())
        .expect("bottom rule");
    // Rules are single-width box glyphs, so char count is column count.
    assert_eq!(top.chars().count(), bottom.chars().count());
}

#[test]
fn nav_highlight_follows_viewer_link() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    assert_eq!(app.focus, FocusPane::Viewer);
    app.follow_link_target("architecture/README.md");
    let page = app.navigator.tab().current().page.clone();
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Page(page)),
        "link followed from the View moves the nav highlight"
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
                    .is_some_and(|l| l.contains("── code ──"))
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
    let rect_of = |app: &App, want: Hit| {
        app.hit_map
            .entries()
            .iter()
            .find(|(_, h)| *h == want)
            .map(|(r, _)| *r)
    };
    let tab = rect_of(&app, Hit::Tab(0)).expect("Hit::Tab(0) after NewTab");
    let close = rect_of(&app, Hit::TabClose(0)).expect("Hit::TabClose(0)");
    // Tab labels live on the middle row of the three-row bar (header is row 0).
    assert_eq!(tab.y, 2);
    assert_eq!(tab.y, close.y);
    assert!(tab.width >= 7, "outlined button, got {}", tab.width);
    assert_eq!(app.hit_map.hit_at(tab.x, tab.y), Some(&Hit::Tab(0)));
    assert_eq!(
        app.hit_map.hit_at(tab.x + tab.width - 1, tab.y),
        Some(&Hit::Tab(0))
    );
    assert!(close.x > tab.x && close.x < tab.x + tab.width - 1);
    assert_eq!(
        app.hit_map.hit_at(close.x, close.y),
        Some(&Hit::TabClose(0))
    );
    app.update(Action::SwitchTab(0));
    assert_eq!(app.navigator.active(), 0);
}

#[test]
fn narrow_tab_bar_keeps_the_active_tab_visible() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    for _ in 0..7 {
        app.update(Action::NewTab);
    }
    let active = app.navigator.active();
    assert!(active > 0);
    let _ = draw_app(&mut app, 60, 24);
    assert!(
        app.hit_map
            .entries()
            .iter()
            .any(|(_, h)| *h == Hit::Tab(active)),
        "active tab {active} must not be displaced by earlier tabs"
    );
}

#[test]
fn tabs_use_focus_colour_text_and_dim_inactive_labels() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    assert_eq!(app.navigator.tab_count(), 1);
    let theme = app.theme;
    let tab_color = |app: &mut App| {
        let terminal = draw_app(app, 120, 24);
        let buf = terminal.backend().buffer();
        let y = app
            .hit_map
            .entries()
            .iter()
            .find(|(_, h)| *h == Hit::Tab(0))
            .map(|(r, _)| (r.x, r.y))
            .expect("the current page's tab is always drawn");
        assert_eq!(buf[(y.0 + 2, y.1)].bg, app.theme.surface);
        buf[(y.0 + 2, y.1)].fg
    };
    app.update(Action::FocusViewer);
    assert_eq!(
        tab_color(&mut app),
        theme.border_focus,
        "active tab uses the focus colour"
    );
    app.update(Action::FocusNav);
    assert_eq!(
        tab_color(&mut app),
        theme.text_secondary,
        "Nav focus retains dimmer active text"
    );
    app.update(Action::OpenHelp);
    let mut hits = HitMap::default();
    let muted = crate::tui::regions::tabs::cells(
        ratatui::layout::Rect::new(0, 0, 120, 3),
        app.navigator.tabs(),
        0,
        false,
        true,
        &theme,
        &mut hits,
    );
    assert!(
        muted.iter().all(|cell| cell.line.spans.iter().all(|span| {
            !span
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        })),
        "popup mutes the active tab"
    );
    app.update(Action::CloseHelp);
    app.update(Action::NewTab);
    app.update(Action::FocusViewer);
    let terminal = draw_app(&mut app, 120, 24);
    let buf = terminal.backend().buffer();
    let (rect, _) = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, hit)| *hit == Hit::Tab(0))
        .unwrap();
    assert_eq!(buf[(rect.x + 1, rect.y)].fg, theme.text_muted);
    assert_eq!(
        buf[(rect.right(), rect.y)].symbol(),
        "│",
        "a divider separates the tab cells"
    );
}

/// Every box-drawing glyph of the View's bars and the nav's search bar uses the
/// border style of the pane it belongs to, and nothing is drawn heavy.
#[test]
fn chrome_bars_match_pane_borders_and_never_go_heavy() {
    const THIN: &str = "─│┌┐└┘├┤┬┴";
    const HEAVY: &str = "━┃┏┓┗┛┣┫┳┻";
    for theme in [Theme::dark(), Theme::light(), Theme::herdr("vesper").0] {
        for width in [40, 60, 80, 120] {
            for view_focused in [true, false] {
                let mut app = App::new(&fixture()).unwrap();
                app.theme = theme;
                app.update(if view_focused {
                    Action::FocusViewer
                } else {
                    Action::FocusNav
                });
                app.update(Action::NewTab);
                app.update(if view_focused {
                    Action::FocusViewer
                } else {
                    Action::FocusNav
                });
                let terminal = draw_app(&mut app, width, 24);
                let buf = terminal.backend().buffer();
                let regions = crate::tui::layout::split(
                    buf.area,
                    app.nav_visible,
                    app.nav_width,
                    app.nav_position,
                );
                for y in 0..24 {
                    for x in 0..width {
                        assert!(
                            !HEAVY.contains(buf[(x, y)].symbol()),
                            "{width}x24 heavy glyph at {x},{y}"
                        );
                    }
                }
                let v = regions.viewer;
                let view_border = app.theme.border(view_focused).fg;
                for y in [v.y, v.y + 2, v.bottom() - 3, v.bottom() - 1] {
                    for x in v.x..v.right() {
                        let cell = &buf[(x, y)];
                        if THIN.contains(cell.symbol()) {
                            assert_eq!(
                                cell.fg,
                                view_border.unwrap(),
                                "{width}: view bar glyph at {x},{y}"
                            );
                            assert_eq!(
                                cell.modifier.contains(ratatui::style::Modifier::BOLD),
                                view_focused,
                                "{width}: view bar weight at {x},{y} follows the pane"
                            );
                        }
                    }
                }
                let n = regions.side_nav;
                if n.width > 0 {
                    let nav_border = app.theme.border(!view_focused).fg.unwrap();
                    for x in n.x..n.right() {
                        let cell = &buf[(x, n.y + 2)];
                        assert!(THIN.contains(cell.symbol()), "search seam at {x}");
                        assert_eq!(cell.fg, nav_border, "{width}: search seam at {x}");
                    }
                }
            }
        }
    }
}

#[test]
fn search_bar_lines_up_with_the_tab_bar() {
    let mut app = App::new(&fixture()).unwrap();
    let terminal = draw_app(&mut app, 120, 24);
    let buf = terminal.backend().buffer();
    let regions =
        crate::tui::layout::split(buf.area, app.nav_visible, app.nav_width, app.nav_position);
    let hits = app.hit_map.entries();
    let tab_y = hits
        .iter()
        .find(|(_, h)| *h == Hit::Tab(0))
        .map(|(r, _)| r.y)
        .unwrap();
    let search_y = hits
        .iter()
        .find(|(_, h)| *h == Hit::NavSearchRow)
        .map(|(r, _)| r.y)
        .unwrap();
    assert_eq!(search_y, tab_y, "search label row = tab label row");
    let row = |y: u16| -> String {
        (regions.side_nav.x..regions.side_nav.right())
            .map(|x| buf[(x, y)].symbol())
            .collect()
    };
    assert!(row(search_y).contains("/ Search…"));
    for x in [regions.side_nav.x, regions.viewer.x] {
        assert_eq!(buf[(x, search_y + 1)].symbol(), "├", "seams share a row");
    }
    // No shaded search fill any more.
    for x in regions.side_nav.x + 1..regions.side_nav.right() - 1 {
        assert_eq!(buf[(x, search_y)].bg, app.theme.surface, "no fill at {x}");
    }
}

#[test]
fn chrome_bars_have_no_fills_and_only_label_rows_are_clickable() {
    for position in [NavPosition::Left, NavPosition::Right] {
        for width in [40, 60, 80, 120] {
            let mut app = App::new(&fixture()).unwrap();
            app.nav_position = position;
            app.update(Action::GoToPage(PageKey {
                collection_id: app.navigator.index().collection_id.clone(),
                relative_path: "architecture/design-system/tokens.md".into(),
            }));
            let terminal = draw_app(&mut app, width, 24);
            let buf = terminal.backend().buffer();
            let regions =
                crate::tui::layout::split(buf.area, app.nav_visible, app.nav_width, position);
            let v = regions.viewer;
            // Header row + three bar rows above the article.
            assert_eq!(app.viewer_geom.top_y, v.y + 3);
            assert_eq!(app.viewer_geom.rows, 16);
            for (rect, hit) in app.hit_map.entries() {
                let label_y = match hit {
                    Hit::Tab(_) | Hit::TabClose(_) => v.y + 1,
                    Hit::Prev | Hit::Next => v.bottom() - 2,
                    _ => continue,
                };
                assert_eq!(rect.y, label_y, "{hit:?} sits on its bar's label row");
                for y in [label_y - 1, label_y + 1] {
                    assert!(!matches!(
                        app.hit_map.hit_at(rect.x, y),
                        Some(Hit::Tab(_) | Hit::TabClose(_) | Hit::Prev | Hit::Next)
                    ));
                }
            }
            for (y, glyph) in [
                (v.y, "┌"),
                (v.y + 2, "├"),
                (v.bottom() - 3, "├"),
                (v.bottom() - 1, "└"),
            ] {
                assert_eq!(buf[(v.x, y)].symbol(), glyph, "{width}: corner at row {y}");
            }
            for y in [
                v.y,
                v.y + 1,
                v.y + 2,
                v.bottom() - 3,
                v.bottom() - 2,
                v.bottom() - 1,
            ] {
                for x in v.x..v.right() {
                    assert_eq!(
                        buf[(x, y)].bg,
                        app.theme.surface,
                        "no background fills at {x},{y}"
                    );
                }
            }
            let tab_row: String = (0..width).map(|x| buf[(x, v.y + 1)].symbol()).collect();
            assert!(tab_row.contains("tokens.md ×"));
        }
    }
}

#[test]
fn tiny_chrome_widths_keep_active_tabs_and_non_overlapping_footer_hits() {
    let mut app = App::new(&fixture()).unwrap();
    for _ in 0..7 {
        app.update(Action::NewTab);
    }
    for width in [1, 4, 8, 9, 12, 18, 40, 60, 120] {
        let mut terminal = Terminal::new(TestBackend::new(width, 6)).unwrap();
        let mut hits = HitMap::default();
        terminal
            .draw(|frame| {
                crate::tui::regions::tabs::draw(
                    frame,
                    ratatui::layout::Rect::new(0, 0, width, 3),
                    app.navigator.tabs(),
                    app.navigator.active(),
                    true,
                    false,
                    &app.theme,
                    &mut hits,
                );
                crate::tui::regions::footer::draw(
                    frame,
                    ratatui::layout::Rect::new(0, 3, width, 3),
                    Some("Long previous 界 title"),
                    Some("Long next 界 title"),
                    None,
                    true,
                    &app.theme,
                    &mut hits,
                );
            })
            .unwrap();
        assert_eq!(
            hits.entries()
                .iter()
                .any(|(_, hit)| *hit == Hit::Tab(app.navigator.active())),
            width >= 7
        );
        for (rect, hit) in hits.entries() {
            assert!(rect.x > 0 && rect.right() < width);
            assert!(rect.y == 1 || rect.y == 4, "label rows only");
            if matches!(hit, Hit::TabClose(_)) {
                assert_eq!(terminal.backend().buffer()[(rect.x, rect.y)].symbol(), "×");
            }
        }
        let footer: Vec<_> = hits
            .entries()
            .iter()
            .filter(|(_, hit)| matches!(hit, Hit::Prev | Hit::Next))
            .collect();
        if footer.len() == 2 {
            assert!(footer[0].0.intersection(footer[1].0).is_empty());
            assert!(
                footer[0].0.right() < footer[1].0.x,
                "a divider column separates the cells"
            );
        }
    }
}

#[test]
fn short_terminals_clip_whole_chrome_bars_and_keep_layout_controls() {
    for height in 1..=10 {
        let mut app = App::new(&fixture()).unwrap();
        let terminal = draw_app(&mut app, 60, height);
        let area = terminal.backend().buffer().area;
        let regions = crate::tui::layout::split(area, false, None, NavPosition::Left);
        if height >= 2 {
            assert_eq!(regions.header.height, 1);
            assert_eq!(regions.status.height, 1);
        }
        assert_eq!(
            app.viewer_geom.rows,
            crate::tui::layout::viewer_visible_rows(regions.viewer.height)
        );
        for (rect, hit) in app.hit_map.entries() {
            assert!(rect.right() <= area.right() && rect.bottom() <= area.bottom());
            if matches!(hit, Hit::Tab(_) | Hit::TabClose(_)) {
                assert!(regions.viewer.height >= 4, "top bar needs three rows");
                assert_eq!(rect.y, regions.viewer.y + 1);
            }
            if matches!(hit, Hit::Prev | Hit::Next) {
                assert!(regions.viewer.height >= 6, "both bars need six rows");
                assert_eq!(rect.y, regions.viewer.bottom() - 2);
            }
        }
    }
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
fn help_overlay_open_close_and_activate() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenHelp);
    assert!(app.help.is_some());
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Help);
    let _ = draw_app(&mut app, 80, 24);
    // Select Quit row and activate.
    if let Some(help) = app.help.as_mut() {
        let quit_idx = help
            .rows
            .iter()
            .position(|r| matches!(r.action, Some(Action::Quit)))
            .expect("quit row");
        help.selected = quit_idx;
    }
    app.update(Action::HelpActivate);
    assert!(app.quit);
    assert!(app.help.is_none());
}

#[test]
fn options_overlay_applies_choices_and_persists() {
    use wiki_reader_core::config::ThemeName;
    let root = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(&root).unwrap();
    app.config_write_path = Some(tmp.path().join("config.toml"));
    app.update(Action::OpenOptions);
    assert!(app.options.is_some());
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Options);
    let buf = draw_app(&mut app, 80, 40);
    let text: String = buf
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    for want in [
        "Theme",
        "Mermaid",
        "Max image rows",
        "Copy path (y)",
        "Navigate",
        "Apply",
    ] {
        assert!(text.contains(want), "missing {want:?}");
    }
    assert!(text.contains('●') && text.contains('○'), "radios drawn");
    // Opens on the current theme; moving does not change it until applied.
    assert_eq!(app.theme_name, ThemeName::Dark);
    app.update(Action::OptionsDown);
    assert_eq!(app.theme_name, ThemeName::Dark);
    app.update(Action::OptionsApply);
    assert_eq!(app.theme_name, ThemeName::Light);
    let light = Theme::from_name(ThemeName::Light).text_secondary;
    assert_eq!(app.theme.text_secondary, light);
    assert_ne!(light, Theme::from_name(ThemeName::Dark).text_secondary);
    let saved = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(saved.contains("theme = \"light\""), "saved={saved}");
    // Light (1) -> Nav right (4).
    for _ in 0..3 {
        app.update(Action::OptionsDown);
    }
    app.update(Action::OptionsApply);
    assert_eq!(
        app.nav_position,
        wiki_reader_core::config::NavPosition::Right
    );
    app.update(Action::CloseOptions);
    assert!(app.options.is_none());
    assert_eq!(app.input_mode, crate::tui::keymap::InputMode::Normal);
}

#[test]
fn options_overlay_snapshot() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenOptions);
    let buf = draw_app(&mut app, 80, 40);
    let b = buf.backend().buffer();
    let mut out = String::new();
    for y in 0..b.area.height {
        for x in 0..b.area.width {
            out.push_str(b[(x, y)].symbol());
        }
        out.push('\n');
    }
    insta::assert_snapshot!("options_80x40", out);
}

#[test]
fn options_titles_and_cursor_use_different_colours() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenOptions);
    let buf = draw_app(&mut app, 80, 40);
    let b = buf.backend().buffer();
    let find = |needle: &str| {
        for y in 0..b.area.height {
            let row: String = (0..b.area.width).map(|x| b[(x, y)].symbol()).collect();
            if let Some(col) = row.find(needle) {
                let x = u16::try_from(row[..col].chars().count()).unwrap();
                return b[(x, y)].fg;
            }
        }
        panic!("{needle:?} not drawn");
    };
    let theme = &app.theme;
    assert_eq!(find("Mermaid"), theme.accent, "group titles use the accent");
    assert_eq!(
        find("> "),
        theme.border_focus,
        "cursor uses the chrome colour"
    );
    assert_ne!(
        theme.accent, theme.border_focus,
        "the two roles are distinct in the default theme"
    );
}

#[test]
fn luckgrid_themes_paint_every_cell_including_popups() {
    use ratatui::style::Color;
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let all_painted = |buf: &Terminal<TestBackend>| {
        buf.backend()
            .buffer()
            .content
            .iter()
            .all(|c| c.bg != Color::Reset)
    };
    let buf = draw_app(&mut app, 80, 30);
    assert!(all_painted(&buf), "dark: every cell has a background");
    assert_eq!(buf.backend().buffer()[(79, 29)].bg, app.theme.surface);
    // A popup's `Clear` must not punch holes of the terminal's colour into the painted screen.
    for open in [Action::OpenOptions, Action::OpenHelp, Action::OpenSearch] {
        app.update(open);
        let buf = draw_app(&mut app, 80, 30);
        assert!(all_painted(&buf), "popup leaves unpainted cells");
        app.update(Action::CloseOptions);
        app.update(Action::CloseHelp);
        app.update(Action::CloseSearch);
    }
    // Presets that follow the terminal paint nothing.
    app.theme = Theme::herdr("vesper").0;
    let buf = draw_app(&mut app, 80, 30);
    assert_eq!(buf.backend().buffer()[(79, 29)].bg, Color::Reset);
}

#[test]
fn options_radios_follow_the_active_value() {
    use crate::tui::options_ui::{OptionChoice, choices};
    use wiki_reader_core::config::{CopyPathMode, ThemeName};
    let root = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(&root).unwrap();
    app.config_write_path = Some(tmp.path().join("config.toml"));
    assert!(app.option_is_selected(OptionChoice::Theme(ThemeName::Dark)));
    assert!(!app.option_is_selected(OptionChoice::Theme(ThemeName::Light)));
    app.update(Action::OpenOptions);
    let copy_abs = choices()
        .iter()
        .position(|c| *c == OptionChoice::CopyPath(CopyPathMode::Absolute))
        .unwrap();
    app.options_activate(copy_abs);
    assert!(app.option_is_selected(OptionChoice::CopyPath(CopyPathMode::Absolute)));
    assert!(!app.option_is_selected(OptionChoice::CopyPath(CopyPathMode::Relative)));
}

#[test]
fn options_scroll_keeps_the_selected_row_and_its_heading_visible() {
    let root = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(&root).unwrap();
    app.config_write_path = Some(tmp.path().join("config.toml"));
    app.update(Action::OpenOptions);
    for _ in 0..crate::tui::options_ui::choices().len() - 1 {
        app.update(Action::OptionsDown);
    }
    let buf = draw_app(&mut app, 80, 16);
    let text: String = buf
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(text.contains("Absolute path"), "last row on screen");
    assert!(text.contains("Copy path (y)"), "its heading stays with it");
    assert!(app.options.as_ref().unwrap().scroll > 0);
}

#[test]
fn options_warns_when_collection_config_overrides_saved_key() {
    let root = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let shadow = tmp.path().join(".wiki-reader.toml");
    std::fs::write(&shadow, "theme = \"herdr\"\n").unwrap();
    let mut app = App::new(&root).unwrap();
    app.config_write_path = Some(tmp.path().join("config.toml"));
    app.config_shadow_path = Some(shadow);
    app.update(Action::OpenOptions);
    app.update(Action::OptionsDown); // Light (the Dark row is already the active value)
    app.update(Action::OptionsApply);
    assert!(
        app.message.contains(".wiki-reader.toml") && app.message.contains("theme"),
        "message={}",
        app.message
    );
    // A row the collection file does not set stays quiet.
    app.message.clear();
    for _ in 0..2 {
        app.update(Action::OptionsDown); // Herdr, then Nav left (not set in the collection file)
    }
    app.update(Action::OptionsApply);
    assert!(app.message.is_empty(), "message={}", app.message);
}

#[test]
fn options_write_error_sets_status_message() {
    let root = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(&root).unwrap();
    // A directory is not a writable file.
    app.config_write_path = Some(tmp.path().to_path_buf());
    app.update(Action::OpenOptions);
    app.update(Action::OptionsApply);
    assert!(!app.message.is_empty());
}

#[test]
fn help_overlay_snapshots() {
    let root = fixture();
    for w in [60u16, 80, 120] {
        let mut app = App::new(&root).expect("app");
        app.update(Action::OpenHelp);
        let backend = TestBackend::new(w, 24);
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
        insta::assert_snapshot!(format!("help_{w}"), out);
    }
}

fn scroll_event(app: &mut App, up: bool) {
    let kind = if up {
        MouseEventKind::ScrollUp
    } else {
        MouseEventKind::ScrollDown
    };
    let _ = apply_mouse(
        app,
        MouseEvent {
            kind,
            column: 40,
            row: 10,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        },
    );
}

#[test]
fn help_groups_are_full_width_heading_rows() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenHelp);
    let help = app.help.as_ref().unwrap();
    assert!(help.rows[0].heading, "first row is the Global heading");
    assert!(!help.rows[help.selected].heading);
    let terminal = draw_app(&mut app, 80, 24);
    let buf = terminal.backend().buffer();
    let row = |y: u16| {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
    };
    let heading = (0..buf.area.height)
        .map(row)
        .find(|r| r.contains("── Global"))
        .expect("heading row");
    assert!(
        !heading.contains("Back") && !heading.contains("Alt+"),
        "heading owns its row: {heading}"
    );
    let after = &heading[heading.find("Global").unwrap()..];
    assert!(
        after.contains('│'),
        "popup border closes the row: {heading}"
    );
    assert!(heading.contains("─────"), "rule fills the row: {heading}");
}

#[test]
fn help_shows_icon_beside_key_and_v_toggles_view() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenHelp);
    let help = app.help.as_ref().unwrap();
    let nav = help
        .rows
        .iter()
        .find(|r| matches!(r.action, Some(Action::ToggleNav)))
        .unwrap();
    assert_eq!((nav.keys.as_str(), nav.icon), ("b", Some("◫")));
    let quit = help
        .rows
        .iter()
        .find(|r| matches!(r.action, Some(Action::Quit)))
        .unwrap();
    assert_eq!((quit.keys.as_str(), quit.icon), ("q", Some("✕")));
    assert!(
        !help.rows.iter().any(|r| r.help.contains("formatted")),
        "the eye / formatted toggle is gone"
    );
}

#[test]
fn help_selection_skips_headings() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::OpenHelp);
    app.update(Action::HelpHome);
    let first = app.help.as_ref().unwrap().selected;
    assert!(!app.help.as_ref().unwrap().rows[first].heading);
    // Walk the whole list both ways: never rests on a heading.
    for _ in 0..80 {
        app.update(Action::HelpSelectDelta(1));
        let h = app.help.as_ref().unwrap();
        assert!(!h.rows[h.selected].heading);
    }
    for _ in 0..80 {
        app.update(Action::HelpSelectDelta(-1));
        let h = app.help.as_ref().unwrap();
        assert!(!h.rows[h.selected].heading);
    }
}

#[test]
fn wheel_scrolls_popups_not_the_page_behind() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let _ = draw_app(&mut app, 80, 12);
    app.update(Action::OpenHelp);
    let _ = draw_app(&mut app, 80, 12);
    let before = app.help.as_ref().unwrap().scroll;
    let page_scroll = app.scroll;
    for _ in 0..3 {
        scroll_event(&mut app, false);
    }
    let _ = draw_app(&mut app, 80, 12);
    assert!(app.help.as_ref().unwrap().scroll > before, "help scrolled");
    assert_eq!(app.scroll, page_scroll, "page behind did not move");
    for _ in 0..10 {
        scroll_event(&mut app, true);
    }
    let _ = draw_app(&mut app, 80, 12);
    assert_eq!(app.help.as_ref().unwrap().scroll, 0);
}

fn footer_border(root: &Path, page: &str, w: u16) -> (App, String) {
    let mut app = App::new(root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from(page),
    }));
    let terminal = draw_app(&mut app, w, 24);
    let buf = terminal.backend().buffer();
    let y = app
        .hit_map
        .entries()
        .iter()
        .find(|(_, hit)| matches!(hit, Hit::Prev | Hit::Next))
        .map(|(rect, _)| rect.y)
        .expect("footer middle row");
    let row = (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol())
        .collect::<String>();
    (app, row)
}

#[test]
fn footer_missing_side_leaves_the_border_unbroken() {
    let root = fixture();
    // First page: no prev, so no dangling arrow or dash on the left.
    let (app, row) = footer_border(&root, "README.md", 100);
    assert!(!row.contains("‹"), "{row}");
    assert!(!row.contains('—'), "{row}");
    assert!(row.contains(" › "), "next control present: {row}");
    assert!(
        !app.hit_map
            .entries()
            .iter()
            .any(|(_, h)| matches!(h, Hit::Prev)),
        "no prev hit"
    );
    // Padded outline buttons.
    let (_, row) = footer_border(&root, "architecture/design-system/tokens.md", 100);
    assert!(row.contains(" ‹ Design System "), "{row}");
}

#[test]
fn footer_button_colours_follow_pane_focus_and_selection() {
    let root = fixture();
    let (mut app, _) = footer_border(&root, "architecture/design-system/tokens.md", 100);
    let theme = app.theme;
    // Foreground and background of the next label.
    let find_style = |app: &mut App| {
        let terminal = draw_app(app, 100, 24);
        let buf = terminal.backend().buffer();
        let (rect, _) = app
            .hit_map
            .entries()
            .iter()
            .find(|(_, hit)| *hit == Hit::Prev)
            .expect("prev button");
        let (x, y) = (rect.x, rect.y);
        // The header breadcrumb also uses `›`, but on another row.
        let nx = (0..buf.area.width)
            .find(|&nx| buf[(nx, y)].symbol() == "›")
            .expect("next button");
        assert_eq!(buf[(x, y)].bg, theme.surface);
        (buf[(nx, y)].fg, buf[(nx, y)].bg)
    };
    app.update(Action::FocusViewer);
    let (fg, bg) = find_style(&mut app);
    assert_eq!(fg, theme.text_muted);
    assert_eq!(bg, theme.surface);
    app.update(Action::FocusNav);
    assert_eq!(find_style(&mut app).0, theme.text_muted);
    app.update(Action::FocusViewer);
    app.update(Action::FocusFooter);
    let (fg, bg) = find_style(&mut app);
    assert_eq!(fg, theme.border_focus);
    assert_eq!(bg, theme.surface);
}

#[test]
fn status_bar_pill_and_page_status() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("decisions/0001-stack.md"),
    }));
    let terminal = draw_app(&mut app, 120, 24);
    let buf = terminal.backend().buffer();
    let y = buf.area.height - 1;
    let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
    assert!(row.contains(" VIEW "), "{row}");
    assert!(row.contains("accepted"), "status after date: {row}");
    let col = u16::try_from(row[..row.find("accepted").unwrap()].chars().count()).unwrap();
    assert_eq!(buf[(col, y)].fg, app.theme.status_ok, "accepted is green");
    let px = u16::try_from(row[..row.find(" VIEW ").unwrap()].chars().count()).unwrap() + 1;
    assert_eq!(buf[(px, y)].bg, app.theme.peach, "pill background");
}

#[test]
fn nav_right_on_current_page_keeps_view_position() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::FocusNav);
    for _ in 0..3 {
        app.update(Action::ViewerDown);
    }
    let cursor = app.cursor_line;
    // Nav cursor is on the current page: Right just hands focus to the View.
    app.update(Action::NavExpand);
    assert_eq!(app.focus, FocusPane::Viewer);
    assert_eq!(app.cursor_line, cursor);
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

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

/// Text of the footer bar's label row (where prev/next are drawn).
fn footer_row(root: &Path) -> String {
    let mut app = App::new(root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    let terminal = draw_app(&mut app, 100, 24);
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .find(|row| row.contains("‹ "))
        .expect("footer border row")
}

#[test]
fn footer_prev_next_use_folder_names_by_default() {
    let row = footer_row(&fixture());
    assert!(row.contains("‹ Design System"), "{row}");
    assert!(row.contains("Workflow OS ›"), "{row}");
    assert!(
        !row.contains("Readme") && !row.contains("Overview"),
        "{row}"
    );
}

#[test]
fn footer_landing_labels_use_titles_in_filename_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("wiki");
    copy_dir(&fixture(), &root);
    std::fs::write(
        root.join(".wiki-reader.toml"),
        "[nav]\nlabels = \"filename\"\n",
    )
    .unwrap();
    let row = footer_row(&root);
    assert!(row.contains("‹ Design System"), "{row}");
    assert!(row.contains("Workflow OS ›"), "{row}");
}

fn screen_rows(app: &mut App, w: u16, h: u16) -> Vec<String> {
    let terminal = draw_app(app, w, h);
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

#[test]
fn panes_have_no_title_tags() {
    let mut app = App::new(&fixture()).unwrap();
    let rows = screen_rows(&mut app, 120, 24);
    // Row 1 is the top border of both panes; the status pill names the focus.
    assert!(
        !rows[1].contains("Nav") && !rows[1].contains("View"),
        "{}",
        rows[1]
    );
}

#[test]
fn nested_nav_rows_sit_one_column_right_of_their_parent_label() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    app.update(Action::NavJumpDown);
    app.update(Action::NavExpand); // Architecture group
    let rows = screen_rows(&mut app, 120, 30);
    let col = |needle: &str| {
        let row = rows
            .iter()
            .find(|r| r.contains(needle))
            .unwrap_or_else(|| panic!("{needle} not drawn"));
        row[..row.find(needle).unwrap()].chars().count()
    };
    assert_eq!(
        col("Architecture Overview"),
        col("architecture") + 1,
        "leaf label one column right of the group label"
    );
}

#[test]
fn shift_click_nav_item_opens_a_new_tab() {
    let mut app = App::new(&fixture()).unwrap();
    // Expand the first group so a page other than the current one is listed.
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    app.update(Action::NavJumpDown);
    app.update(Action::NavExpand);
    let _ = draw_app(&mut app, 120, 30);
    let (rect, id) = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::NavItem(id @ NodeId::Page(_)) => Some((*r, id.clone())),
            _ => None,
        })
        .expect("a page row");
    let before = app.navigator.tab_count();
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 2,
        row: rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::SHIFT,
    };
    assert!(apply_mouse(&mut app, ev).is_none());
    assert_eq!(app.navigator.tab_count(), before + 1, "opened in a new tab");
    let NodeId::Page(key) = id else {
        unreachable!()
    };
    assert_eq!(app.navigator.tab().current().page, key);
}

#[test]
fn shift_enter_in_nav_opens_a_new_tab_and_enter_does_not() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    app.update(Action::NavJumpDown);
    app.update(Action::NavExpand);
    app.update(Action::NavStepDown); // first child page of the group
    let (shifted, _) = crate::tui::keymap::map_with_overrides(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        app.focus,
        app.input_mode,
        app.chord,
        None,
    );
    assert_eq!(shifted, Some(Action::NewTab));
    let (plain, _) = crate::tui::keymap::map_with_overrides(
        KeyEvent::from(KeyCode::Enter),
        app.focus,
        app.input_mode,
        app.chord,
        None,
    );
    assert_eq!(plain, Some(Action::NavActivate));
    let before = app.navigator.tab_count();
    app.update(Action::NewTab);
    assert_eq!(app.navigator.tab_count(), before + 1);
}

#[test]
fn ctrl_click_nav_and_link_open_a_new_tab_plain_click_does_not() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    app.update(Action::NavJumpDown);
    app.update(Action::NavExpand);
    let _ = draw_app(&mut app, 120, 30);
    let nav_rect = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::NavItem(NodeId::Page(_)) => Some(*r),
            _ => None,
        })
        .expect("a page row");
    let before = app.navigator.tab_count();
    let ctrl = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: nav_rect.x + 2,
        row: nav_rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::CONTROL,
    };
    assert!(apply_mouse(&mut app, ctrl).is_none());
    assert_eq!(app.navigator.tab_count(), before + 1);

    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 30);
    let link_rect = app
        .hit_map
        .entries()
        .iter()
        .find_map(|(r, h)| match h {
            Hit::Link(_) => Some(*r),
            _ => None,
        })
        .expect("a link");
    let before = app.navigator.tab_count();
    let ctrl_link = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: link_rect.x,
        row: link_rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::CONTROL,
    };
    assert!(apply_mouse(&mut app, ctrl_link).is_none());
    assert_eq!(app.navigator.tab_count(), before + 1);

    let plain = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: link_rect.x,
        row: link_rect.y,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    };
    let after_plain = app.navigator.tab_count();
    let _ = apply_mouse(&mut app, plain);
    assert_eq!(
        app.navigator.tab_count(),
        after_plain,
        "plain click must not open a tab"
    );
}

#[test]
fn ctrl_enter_and_ctrl_right_in_nav_open_a_new_tab() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::FocusNav);
    app.navigator.set_nav_stop(NavStop::Search);
    app.update(Action::NavStepDown);
    app.update(Action::NavJumpDown);
    app.update(Action::NavExpand);
    app.update(Action::NavStepDown);
    for code in [KeyCode::Enter, KeyCode::Right] {
        app.update(Action::FocusNav);
        let (mapped, _) = crate::tui::keymap::map_with_overrides(
            KeyEvent::new(code, KeyModifiers::CONTROL),
            app.focus,
            app.input_mode,
            app.chord,
            None,
        );
        let (want, want_focus) = if code == KeyCode::Right {
            (Action::NewTabFocusView, FocusPane::Viewer)
        } else {
            (Action::NewTab, FocusPane::Nav)
        };
        assert_eq!(mapped, Some(want), "Ctrl+{code:?}");
        let before = app.navigator.tab_count();
        app.update(mapped.expect("mapped"));
        assert_eq!(app.navigator.tab_count(), before + 1, "Ctrl+{code:?}");
        assert_eq!(app.focus, want_focus, "Ctrl+{code:?} focus");
    }
    app.update(Action::FocusNav);
    // Cmd is not a new-tab modifier (ADR-0016): Cmd+Enter acts like Enter, no new tab.
    let (cmd_enter, _) = crate::tui::keymap::map_with_overrides(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SUPER),
        app.focus,
        app.input_mode,
        app.chord,
        None,
    );
    assert_ne!(cmd_enter, Some(Action::NewTab));
    // Plain → still expands / opens in place.
    let (plain_right, _) = crate::tui::keymap::map_with_overrides(
        KeyEvent::from(KeyCode::Right),
        app.focus,
        app.input_mode,
        app.chord,
        None,
    );
    assert_eq!(plain_right, Some(Action::NavExpand));
}

#[test]
fn ctrl_enter_on_focused_viewer_link_opens_a_new_tab() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let idx = app
        .focus_list()
        .iter()
        .position(|it| it.kind == FocusTarget::Link)
        .expect("a link");
    app.update(Action::FocusViewer);
    app.focused_item = Some(idx);
    let (mapped, _) = crate::tui::keymap::map_with_overrides(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        app.focus,
        app.input_mode,
        app.chord,
        None,
    );
    assert_eq!(mapped, Some(Action::NewTab));
    let before = app.navigator.tab_count();
    app.update(mapped.expect("mapped"));
    assert_eq!(app.navigator.tab_count(), before + 1);
}

#[test]
fn frontmatter_label_colour_does_not_depend_on_the_cursor_line() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::FocusViewer);
    let fg_of_label = |app: &mut App| {
        let terminal = draw_app(app, 120, 24);
        let buf = terminal.backend().buffer();
        let (y, x) = (0..buf.area.height)
            .find_map(|y| {
                let row: Vec<&str> = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
                row.concat()
                    .contains("frontmatter")
                    .then(|| (y, row.iter().position(|s| *s == "f").unwrap()))
            })
            .expect("frontmatter label");
        buf[(u16::try_from(x).unwrap(), y)].fg
    };
    assert_eq!(app.cursor_line, 0, "cursor starts on the frontmatter row");
    let on_cursor = fg_of_label(&mut app);
    app.update(Action::ViewerDown);
    app.update(Action::ViewerDown);
    let off_cursor = fg_of_label(&mut app);
    assert_eq!(on_cursor, off_cursor);
}

#[test]
fn focused_backlink_hides_column_cursor() {
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/README.md"),
    }));
    let _ = draw_app(&mut app, 120, 24);
    let bl = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.ends_with("tokens.md") && s.raw_target.starts_with('/'))
        .expect("tokens backlink")
        .clone();
    let items = app.focus_list();
    let bl_idx = items
        .iter()
        .position(|it| it.link_id == Some(bl.id))
        .expect("backlink in focus list");
    app.update(Action::FocusViewer);
    app.focused_item = Some(bl_idx);
    app.cursor_line = bl.segments[0].0;
    app.cursor_col = bl.segments[0].1.0;
    let terminal = draw_app(&mut app, 120, 40);
    let buf = terminal.backend().buffer();
    let title_line = bl.segments[0].0;
    let (bx, title_y) = cell_xy(&app, title_line, 0);
    let (tx0, ty0) = cell_xy(&app, title_line, bl.segments[0].1.0);
    assert!(
        !buf[(bx, title_y)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "▌ cell must not also be reverse-video"
    );
    assert!(
        !buf[(tx0, ty0)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED),
        "title glyphs must not show the column cursor"
    );
}

#[test]
fn strong_inside_quote_keeps_quote_bar_background() {
    let md = "> [!NOTE]\n> Before **widget sidebar** after.\n";
    let (_d, mut app, _) = app_with_page(md);
    let terminal = draw_app(&mut app, 80, 20);
    let buf = terminal.backend().buffer();
    let line = row_of(&app, "widget sidebar");
    let col = u16::try_from(
        app.doc.lines()[line as usize]
            .find("widget")
            .expect("bold phrase"),
    )
    .unwrap_or(0);
    let (x, y) = cell_xy(&app, line, col);
    assert_eq!(
        buf[(x, y)].bg,
        app.theme.quote_bar,
        "strong inside quote must keep quote_bar bg, not a black hole"
    );
}

#[test]
fn tab_cycles_table_links_onto_their_cells() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# T\n\n| Phase | Tasks |\n| --- | --- |\n| 1 | [a.md](a.md) |\n| 2 | [b.md](b.md) |\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# A\n").unwrap();
    std::fs::write(dir.path().join("b.md"), "# B\n").unwrap();
    let mut app = App::new(dir.path()).unwrap();
    let _ = draw_app(&mut app, 80, 24);
    let a = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.contains("a.md"))
        .expect("a.md")
        .clone();
    let b = app
        .doc
        .link_spans()
        .iter()
        .find(|s| s.raw_target.contains("b.md"))
        .expect("b.md")
        .clone();
    let items = app.focus_list();
    let a_idx = items
        .iter()
        .position(|it| it.link_id == Some(a.id))
        .expect("a in focus list");
    let b_idx = items
        .iter()
        .position(|it| it.link_id == Some(b.id))
        .expect("b in focus list");
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = 0;
    for _ in 0..items.len().saturating_add(2) {
        app.update(Action::ViewerTab);
        if app.focused_item == Some(a_idx) {
            break;
        }
    }
    assert_eq!(app.focused_item, Some(a_idx));
    assert_eq!(app.cursor_line, a.segments[0].0, "cursor on a.md cell row");
    app.update(Action::ViewerTab);
    assert_eq!(app.focused_item, Some(b_idx));
    assert_eq!(app.cursor_line, b.segments[0].0, "cursor on b.md cell row");
    assert!(
        b.segments[0].0 > a.segments[0].0,
        "table links on distinct rows"
    );
}

#[test]
fn rendered_semantic_colours_survive_cursor_highlighting() {
    let (_d, mut app, _log) = app_with_page("# First\n\n## Second\n");
    app.update(Action::FocusViewer);
    let terminal = draw_app(&mut app, 80, 20);
    let buf = terminal.backend().buffer();
    let first = row_of(&app, "First");
    let second = row_of(&app, "Second");
    let (x1, y1) = cell_xy(&app, first, 0);
    let (x2, y2) = cell_xy(&app, second, 0);
    assert_eq!(buf[(x1, y1)].fg, app.theme.heading[0]);
    assert_eq!(buf[(x2, y2)].fg, app.theme.heading[1]);
}

#[test]
fn popups_gray_out_the_panes_behind_them() {
    let mut app = App::new(&fixture()).unwrap();
    let theme = app.theme;
    app.update(Action::FocusViewer);
    let corner_fg = |app: &mut App| {
        let terminal = draw_app(app, 120, 30);
        let buf = terminal.backend().buffer();
        // Top-left corner of the View pane (right of the nav column).
        let nav_w = app
            .hit_map
            .entries()
            .iter()
            .find_map(|(r, h)| matches!(h, Hit::FocusNav).then_some(r.width));
        let x = nav_w.expect("nav drawn");
        buf[(x, 1)].fg
    };
    assert_eq!(corner_fg(&mut app), theme.border_focus, "View focused");
    app.update(Action::OpenHelp);
    assert_eq!(corner_fg(&mut app), theme.border, "help open: gray");
    app.update(Action::CloseHelp);
    app.update(Action::OpenSearch);
    assert_eq!(corner_fg(&mut app), theme.border, "search open: gray");
}

#[test]
fn search_popup_follows_the_markdown_reader_layout() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::OpenSearch);
    let rows = screen_rows(&mut app, 100, 30);
    let all = rows.join("\n");
    assert!(
        all.contains("/ █ type to search"),
        "prompt + placeholder inline:\n{all}"
    );
    assert!(all.contains("type to search  ↑/↓: navigate  Enter: open  Tab: toggle mode"));
    assert!(!all.contains("(Tab)"), "no Files/Content tab row");
    for c in "ar".chars() {
        app.update(Action::SearchChar(c));
    }
    app.update(Action::SearchToggleMode);
    let rows = screen_rows(&mut app, 100, 30);
    let row = rows
        .iter()
        .find(|r| r.contains("] ") && r.contains(" – "))
        .unwrap_or_else(|| panic!("no content result row:\n{}", rows.join("\n")));
    let after_border = row.trim_start_matches(|c: char| c != '[');
    assert!(
        after_border.starts_with('['),
        "line number comes first: {row}"
    );
    assert!(
        rows.iter().any(|r| r.contains("matches")),
        "metadata in the footer"
    );
}

#[test]
fn help_popup_is_tall_and_thin_with_icons_beside_keys() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::OpenHelp);
    let rows = screen_rows(&mut app, 120, 40);
    let all = rows.join("\n");
    assert!(all.contains("b / ◫"), "icon joined to its key with ` / `");
    assert!(all.contains("q / ✕"));
    let top = rows.iter().position(|r| r.contains("Help (")).unwrap();
    let bottom = rows
        .iter()
        .rposition(|r| r.contains('└') && r.contains('┘'))
        .unwrap();
    assert!(bottom - top >= 30, "tall: {} rows", bottom - top);
    let chars: Vec<char> = rows[top].chars().collect();
    let title_at = rows[top][..rows[top].find("Help (").unwrap()]
        .chars()
        .count();
    let left = chars[..title_at].iter().rposition(|&c| c == '┌').unwrap();
    let right = title_at + chars[title_at..].iter().position(|&c| c == '┐').unwrap();
    assert!(right - left < 70, "thin: {} cols", right - left);
}

#[test]
fn raw_view_wraps_long_lines_and_numbers_only_the_first_row() {
    use crate::tui::page_doc::PageDoc;
    let para = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon";
    let (_d, mut app, log) = app_with_page(&format!("# T\n\n{para}\n\nafter\n"));
    app.update(Action::ToggleViewMode);
    let rows = screen_rows(&mut app, 60, 24);
    let PageDoc::Raw(doc) = &app.doc else {
        panic!("raw doc expected")
    };
    let width = usize::from(app.layout_width) - 6;
    assert!(
        doc.lines().iter().all(|l| l.chars().count() <= width),
        "{:?}",
        doc.lines()
    );
    assert!(doc.lines().len() > 5, "the long line took several rows");
    assert_eq!(
        doc.lines().concat(),
        format!("# T{para}after"),
        "rows rejoin exactly"
    );
    let nums: Vec<Option<u32>> = doc.numbers().to_vec();
    assert_eq!(nums[0], Some(1));
    assert!(nums.contains(&None), "continuation rows have no number");
    assert_eq!(
        nums.iter().flatten().copied().collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert!(
        rows.iter().any(|r| r.contains("    │ ")),
        "blank gutter on wrapped rows"
    );
    // Copying across the wrapped rows gives the source line back.
    let first = u32::try_from(
        doc.lines()
            .iter()
            .position(|l| l.starts_with("alpha"))
            .unwrap(),
    )
    .unwrap();
    let last = u32::try_from(
        doc.lines()
            .iter()
            .position(|l| l.contains("upsilon"))
            .unwrap(),
    )
    .unwrap();
    let end = last_col(&app, last);
    drag_select(&mut app, (first, 0), (last, end));
    assert_eq!(log.lock().unwrap().clone(), [para]);
    // Cursor mapping: a display row maps back to its source line and forward again.
    assert_eq!(app.doc.source_cursor(last), 2);
    assert_eq!(app.doc.display_cursor(2), first);
}

#[test]
fn reopened_nav_starts_right_under_the_search_bar() {
    let mut app = App::new(&fixture()).unwrap();
    let _ = draw_app(&mut app, 120, 30);
    app.update(Action::ToggleNav);
    let _ = draw_app(&mut app, 120, 30);
    // Footer links while the nav is hidden used to scroll the current page to
    // the top of a 1-row nav viewport.
    for _ in 0..4 {
        app.update(Action::NextPage);
        let _ = draw_app(&mut app, 120, 30);
    }
    app.update(Action::ToggleNav);
    let rows = screen_rows(&mut app, 120, 30);
    assert_eq!(app.nav_scroll, 0, "a list that fits is not scrolled");
    assert!(
        rows[4].contains("Worked Example Wiki"),
        "first tree row under the search bar: {:?}",
        &rows[1..6]
    );
}

#[test]
fn reopened_nav_still_keeps_the_current_row_visible_when_the_list_is_long() {
    let mut app = App::new(&fixture()).unwrap();
    let _ = draw_app(&mut app, 120, 30);
    app.update(Action::ToggleNav);
    let _ = draw_app(&mut app, 120, 30);
    for _ in 0..6 {
        app.update(Action::NextPage);
    }
    app.update(Action::ToggleNav);
    // A short terminal: the nav viewport is smaller than the list.
    let _ = draw_app(&mut app, 120, 9);
    let rows = app.nav_rows();
    let current = NodeId::Page(app.navigator.tab().current().page.clone());
    let idx = rows
        .iter()
        .position(|r| r.id == current)
        .expect("current row");
    let (start, vh) = (usize::from(app.nav_scroll), usize::from(app.nav_viewport));
    assert!(
        vh < rows.len(),
        "viewport {vh} should be shorter than {} rows",
        rows.len()
    );
    assert!(
        (start..start + vh).contains(&idx),
        "row {idx} outside [{start}, {})",
        start + vh
    );
}

fn content_search_activate(app: &mut App, query: &str) {
    app.update(Action::OpenSearch);
    for c in query.chars() {
        app.update(Action::SearchChar(c));
    }
    app.update(Action::SearchToggleMode);
    app.update(Action::SearchActivate);
}

#[test]
fn content_search_result_keeps_the_page_in_place_and_marks_the_phrase() {
    use ratatui::style::Modifier;
    let mut app = App::new(&fixture()).unwrap();
    let _ = draw_app(&mut app, 100, 30);
    content_search_activate(&mut app, "arc");
    assert_eq!(
        app.scroll, 0,
        "loads like a normal page, not pinned to the match"
    );
    let (line, c0, c1) = app.match_span().expect("phrase found on the match row");
    assert_eq!(app.cursor_line, line);
    assert_eq!(app.cursor_col, c0, "cursor on the phrase start");
    assert_eq!(c1 - c0, 3);
    let terminal = draw_app(&mut app, 100, 30);
    let buf = terminal.backend().buffer();
    let theme = app.theme;
    let (x, y) = cell_xy(&app, line, c0);
    // Cursor: the phrase colours remain present and are reversed.
    let cursor = &buf[(x, y)];
    assert!(cursor.modifier.contains(Modifier::REVERSED));
    assert_eq!((cursor.bg, cursor.fg), (theme.peach, theme.on_peach));
    // Rest of the phrase: the tab / footer-link colours, readable.
    for dx in 1..(c1 - c0) {
        let cell = &buf[(x + dx, y)];
        assert_eq!(
            (cell.bg, cell.fg),
            (theme.peach, theme.on_peach),
            "phrase cell +{dx}"
        );
    }
    // The rest of the row is just the cursor line, not a yellow slab.
    assert_eq!(buf[(x + (c1 - c0) + 1, y)].bg, theme.cursor_line);
}

#[test]
fn content_search_phrase_paints_across_wrapped_rows() {
    let (_dir, mut app, _log) = app_with_page("# T\n\nabcdefghijklmnop alpha\n");
    app.ensure_layout_width(20);
    app.search_phrase = "abcdefghijklmnop alpha".into();
    app.search_matches = vec![2];
    app.search_match_idx = 0;
    app.focus_current_search_match();

    let spans = app.match_spans();
    assert_eq!(spans.len(), 2, "phrase should continue onto the next row");
    assert_ne!(spans[0].0, spans[1].0);
    assert_eq!(app.cursor_line, spans[0].0);
    assert_eq!(app.cursor_col, spans[0].1);
}

#[test]
fn content_search_result_past_the_first_screen_scrolls_to_centre_it() {
    let dir = tempfile::tempdir().unwrap();
    let md = format!(
        "# Long\n\n{}the zebra crossing\n\n{}",
        "filler line\n\n".repeat(120),
        "trailing line\n\n".repeat(60)
    );
    std::fs::write(dir.path().join("README.md"), md).unwrap();
    let mut app = App::new(dir.path()).unwrap();
    let _ = draw_app(&mut app, 100, 24);
    content_search_activate(&mut app, "zebra");
    let rows = u32::from(app.viewer_rows);
    assert!(
        app.cursor_line > rows,
        "match is well past the first screen"
    );
    assert_eq!(app.scroll, app.cursor_line - rows / 2, "centred");
    assert!(app.match_span().is_some());
}

#[test]
fn search_content_rows_end_in_an_ellipsis_and_footer_hugs_border() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::OpenSearch);
    for c in "ar".chars() {
        app.update(Action::SearchChar(c));
    }
    app.update(Action::SearchToggleMode);
    let rows = screen_rows(&mut app, 90, 24);
    let cut = rows
        .iter()
        .find(|r| r.contains('[') && r.contains(" – ") && r.contains('…'))
        .unwrap_or_else(|| panic!("no truncated content row:\n{}", rows.join("\n")));
    let tail: Vec<char> = cut.chars().collect();
    let dots = tail.iter().rposition(|&c| c == '…').unwrap();
    assert!(
        tail[dots + 1..]
            .iter()
            .take_while(|&&c| c != '│')
            .all(|&c| c == ' '),
        "ellipsis is the last text on the row: {cut}"
    );
    let footer = rows
        .iter()
        .position(|r| r.contains("Tab: toggle mode") && r.contains("matches"))
        .unwrap();
    let below = &rows[footer + 1];
    assert!(
        below.contains('└') && below.contains('┘'),
        "popup border must sit directly under the footer: {below}"
    );
}

#[test]
fn search_query_row_sits_directly_under_top_border() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::OpenSearch);
    let rows = screen_rows(&mut app, 90, 24);
    let top = rows
        .iter()
        .position(|r| r.contains('┌') && r.contains("Search"))
        .expect("search popup top border");
    let under = &rows[top + 1];
    assert!(
        under.contains('/') && (under.contains('█') || under.contains("type to search")),
        "query row must sit under the top border: {under}"
    );
}

#[test]
fn narrow_nav_ellipsises_long_titles() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("wiki");
    copy_dir(&fixture(), &root);
    std::fs::write(
        root.join("decisions/long-title.md"),
        "---\ntitle: \"An extraordinarily long ADR title that must be clipped in a narrow nav\"\n---\n\n# Long\n\nBody.\n",
    )
    .unwrap();
    let mut app = App::new(&root).unwrap();
    app.nav_width = Some(22);
    app.navigator
        .set_group_expanded(NodeId::Group(PathBuf::from("decisions")), true);
    let rows = screen_rows(&mut app, 80, 24);
    let hit = rows
        .iter()
        .find(|r| r.contains('…') && (r.contains("extraordin") || r.contains("An extra")))
        .unwrap_or_else(|| panic!("expected ellipsised nav title:\n{}", rows.join("\n")));
    assert!(
        !hit.contains("clipped in a narrow"),
        "tail of title must be cut: {hit}"
    );
}

#[test]
fn help_dividers_have_one_row_above_and_none_below() {
    let app = App::new(&fixture()).unwrap();
    let help = crate::tui::help_ui::HelpOverlay::new(&app.key_overrides);
    let heads: Vec<usize> = help
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.heading)
        .map(|(i, _)| i)
        .collect();
    assert!(heads.len() >= 3);
    assert_eq!(
        heads[0], 0,
        "no spacer above the first divider (window padding covers it)"
    );
    for &h in &heads[1..] {
        assert!(
            help.rows[h - 1].spacer && !help.rows[h - 2].spacer,
            "one blank row above row {h}"
        );
    }
    for &h in &heads {
        assert!(
            help.rows[h + 1].selectable(),
            "binding rows start right under the divider"
        );
    }
}

#[test]
fn help_overlay_opens_on_a_tiny_terminal_without_panicking() {
    let mut app = App::new(&fixture()).unwrap();
    app.update(Action::OpenHelp);
    for (w, h) in [(20u16, 10u16), (8, 6), (3, 3)] {
        let _ = draw_app(&mut app, w, h);
    }
}

#[test]
fn content_search_phrase_ignores_trailing_space_in_the_query() {
    let mut app = App::new(&fixture()).unwrap();
    let _ = draw_app(&mut app, 100, 30);
    content_search_activate(&mut app, "arc ");
    assert!(
        app.match_span().is_some(),
        "the phrase is trimmed like the search itself"
    );
}

#[test]
fn clicking_in_the_view_clears_the_content_search_highlight() {
    let mut app = App::new(&fixture()).unwrap();
    let _ = draw_app(&mut app, 100, 30);
    content_search_activate(&mut app, "arc");
    assert!(!app.search_matches.is_empty());
    app.update(Action::SelectStart(0, 0));
    assert!(app.search_matches.is_empty());
}

#[test]
fn rewrapping_drops_the_selection_and_raw_keeps_its_highlights() {
    use crate::tui::page_doc::PageDoc;
    let para = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon";
    let (_d, mut app, _log) = app_with_page(&format!("# T\n\n{para}\n"));
    app.update(Action::ToggleViewMode);
    let _ = screen_rows(&mut app, 80, 24);
    if let PageDoc::Raw(d) = &mut app.doc {
        d.set_highlights(vec![
            vec![crate::tui::highlight::HlSpan {
                style: ratatui::style::Style::default(),
                text: "# T".into(),
            }],
            vec![],
            vec![],
        ]);
    }
    app.update(Action::SelectStart(2, 3));
    assert!(app.selection.is_some());
    let _ = screen_rows(&mut app, 60, 24);
    assert!(app.selection.is_none(), "display coordinates went stale");
    let PageDoc::Raw(d) = &app.doc else {
        panic!("raw doc expected")
    };
    assert!(!d.highlights.is_empty(), "syntax colours survive a resize");
}

#[test]
fn raw_status_column_counts_from_the_start_of_the_source_line() {
    use crate::tui::viewer_doc::RawDoc;
    let doc = RawDoc::from_source(&"x".repeat(30), None).wrapped(10);
    assert_eq!(doc.row_col_offset(0), 0);
    assert_eq!(doc.row_col_offset(1), 10);
    assert_eq!(doc.row_col_offset(2), 20);
}

fn app_with_config(toml: &str) -> (App, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    std::fs::write(&path, toml).expect("config");
    let app = App::build(&fixture(), Some(&path), false, true).expect("app");
    (app, tmp)
}

#[test]
fn theme_config_selects_the_preset_for_drawing_and_diagrams() {
    use wiki_reader_core::config::ThemeName;
    let (mut app, _tmp) = app_with_config("theme = \"light\"\n");
    let light = crate::tui::theme::Theme::from_name(ThemeName::Light);
    assert_eq!(app.theme.link, light.link);
    assert_eq!(
        app.render_opts().diagram_palette,
        light.diagram,
        "Mermaid is drawn with the preset's colours"
    );
    assert_eq!(app.theme.syntax, "InspiredGitHub");

    let terminal = draw_app(&mut app, 100, 24);
    let buf = terminal.backend().buffer();
    let x = buf.area.width - 2;
    let rows = (0..buf.area.height)
        .filter(|&y| buf[(x, y)].bg == light.cursor_line)
        .count();
    assert_eq!(rows, 1, "the light cursor row is painted");
    let dark = crate::tui::theme::Theme::dark();
    assert!(
        (0..buf.area.height).all(|y| buf[(x, y)].bg != dark.cursor_line),
        "no dark-preset colours leak into the light preset"
    );
}

#[test]
fn unknown_theme_keeps_the_dark_preset() {
    use wiki_reader_core::config::ThemeName;
    let (app, _tmp) = app_with_config("theme = \"neon\"\n");
    let dark = crate::tui::theme::Theme::from_name(ThemeName::Dark);
    assert_eq!(app.theme.link, dark.link);
    assert_eq!(app.render_opts().diagram_palette, dark.diagram);
}

#[test]
fn copy_page_path_from_nav_page_row() {
    use crate::tui::clipboard::RecordingClipboard;
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    app.update(Action::FocusNav);
    app.update(Action::CopyPagePath);
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied, ["architecture/design-system/tokens.md"]);
    assert!(app.message.contains("architecture/design-system/tokens.md"));
}

#[test]
fn copy_page_path_from_nav_folder_row() {
    use crate::tui::clipboard::RecordingClipboard;
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    app.update(Action::FocusNav);
    app.update(Action::NavCollapse);
    assert_eq!(
        app.navigator.nav().cursor,
        NavStop::Node(NodeId::Group(PathBuf::from("architecture/design-system")))
    );
    app.update(Action::CopyPagePath);
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied, ["architecture/design-system"]);
    assert!(app.message.contains("architecture/design-system"));
}

#[test]
fn copy_page_path_from_view_keeps_viewed_page() {
    use crate::tui::clipboard::RecordingClipboard;
    let root = fixture();
    let mut app = App::new(&root).unwrap();
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    // Nav cursor on a different row; View focus still copies the viewed page.
    app.update(Action::FocusNav);
    app.update(Action::NavCollapse);
    app.update(Action::FocusViewer);
    app.update(Action::CopyPagePath);
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied, ["architecture/design-system/tokens.md"]);
}

#[test]
fn copy_page_path_absolute_uses_collection_root() {
    use crate::tui::clipboard::RecordingClipboard;
    let (mut app, _tmp) = app_with_config("[copy]\npath = \"absolute\"\n");
    let rec = RecordingClipboard::default();
    let log = Arc::clone(&rec.copied);
    app.clipboard = Box::new(rec);
    app.update(Action::GoToPage(PageKey {
        collection_id: "worked-example".into(),
        relative_path: PathBuf::from("README.md"),
    }));
    app.update(Action::CopyPagePath);
    let copied = log.lock().unwrap().clone();
    assert_eq!(copied.len(), 1);
    let want = app.provider.root().join("README.md");
    assert_eq!(PathBuf::from(&copied[0]), want);
}

const GRID_MD: &str = "# T\n\n| Name | N |\n|---|---|\n| Bob | 10 |\n| alice | 9 |\n";

fn modal_key(app: &mut App, keys: &str) {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for c in keys.chars() {
        let code = if c == '\n' {
            KeyCode::Esc
        } else {
            KeyCode::Char(c)
        };
        app.update(Action::ModalKey(KeyEvent::new(code, KeyModifiers::NONE)));
    }
}

fn screen(term: &Terminal<TestBackend>) -> String {
    let buf = term.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn expand_table_action_opens_the_modal_and_esc_closes_it() {
    let (_d, mut app, _log) = app_with_page(GRID_MD);
    let _ = draw_app(&mut app, 80, 24);
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = 0;
    app.update(Action::ViewerTab);
    let it = app.focus_list()[app.focused_item.expect("focused")].clone();
    assert_eq!(it.kind, FocusTarget::BlockAction);
    assert_eq!(app.cursor_line, it.line.unwrap());
    app.update(Action::ViewerActivate);
    assert!(app.modal.is_some());
    assert_eq!(app.input_mode, InputMode::Modal);
    let term = draw_app(&mut app, 80, 24);
    let s = screen(&term);
    assert!(s.contains("Table (line 3)") && s.contains("alice"), "{s}");
    modal_key(&mut app, "\n");
    assert!(app.modal.is_none());
    assert_eq!(app.input_mode, InputMode::Normal);
}

#[test]
fn enter_inside_a_table_opens_it_and_modal_copies_a_row() {
    let (_d, mut app, log) = app_with_page(GRID_MD);
    let _ = draw_app(&mut app, 80, 24);
    app.update(Action::FocusViewer);
    app.focused_item = None;
    app.cursor_line = row_of(&app, "alice");
    app.update(Action::ViewerActivate);
    assert!(app.modal.is_some(), "Enter on a table row opens the modal");
    // Sort by name: alice first, then copy that row.
    modal_key(&mut app, "sY");
    assert_eq!(log.lock().unwrap().as_slice(), ["alice\t9"]);
}

#[test]
fn page_changes_reach_the_herdr_publisher_once_with_title_and_relative_path() {
    use crate::herdr::{Herdr, Publisher, Timing};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<Vec<String>>>);
    impl Herdr for Recorder {
        fn run(&self, args: &[String]) -> Result<String, String> {
            self.0.lock().unwrap().push(args.to_vec());
            Ok(String::new())
        }
    }

    let recorder = Arc::new(Recorder::default());
    let herdr: Arc<dyn Herdr> = Arc::clone(&recorder) as Arc<dyn Herdr>;
    let mut app = App::new(&fixture()).unwrap();
    app.publisher = Some(Publisher::spawn(
        herdr,
        "w1:p2".into(),
        Timing {
            debounce: Duration::from_millis(20),
            ..Timing::default()
        },
    ));
    let wait_for = |calls: usize| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.0.lock().unwrap().len() < calls {
            assert!(
                Instant::now() < deadline,
                "{:?}",
                recorder.0.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    };

    app.sync_herdr();
    app.sync_herdr();
    wait_for(1);
    app.update(Action::GoToPage(PageKey {
        collection_id: app.navigator.index().collection_id.clone(),
        relative_path: PathBuf::from("architecture/design-system/tokens.md"),
    }));
    app.sync_herdr();
    app.sync_herdr();
    wait_for(2);
    std::thread::sleep(Duration::from_millis(100));
    let calls = recorder.0.lock().unwrap().clone();
    assert_eq!(
        calls.len(),
        2,
        "one report per page, not per frame: {calls:?}"
    );
    assert!(
        calls[1].contains(&"--token=page=architecture/design-system/tokens.md".to_owned()),
        "{:?}",
        calls[1]
    );
    let title = calls[1]
        .iter()
        .find_map(|a| a.strip_prefix("--title="))
        .expect("a title");
    assert!(
        !title.is_empty() && std::path::Path::new(title).extension().is_none(),
        "a page title, got {title}"
    );

    // Without a publisher (tests, outside herdr, opted out) nothing happens.
    let mut quiet = App::new(&fixture()).unwrap();
    assert!(quiet.publisher.is_none());
    quiet.sync_herdr();
}
