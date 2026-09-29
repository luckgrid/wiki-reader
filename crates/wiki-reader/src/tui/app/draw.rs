//! Frame draw: regions + status.

use ratatui::Frame;

use super::App;
use crate::tui::focus::FocusPane;
use crate::tui::layout;
use crate::tui::regions::status::StatusModel;
use crate::tui::regions::{footer, header, side_nav, status, viewer};
use crate::tui::viewer_doc::{FocusTarget, ViewerDoc, format_target_with_provider};

/// Draw all regions and rebuild the hit map.
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    app.hit_map.clear();
    let area = frame.area();
    app.sync_nav_for_width(area.width);
    let regions = layout::split(area, app.nav_visible);
    app.viewer_rows = regions.viewer.height.max(1);
    // Borders (2) + search row (1); remaining rows show the tree.
    app.nav_viewport = regions.side_nav.height.saturating_sub(3).max(1);
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();

    header::draw(frame, regions.header, &crumbs, &theme, &mut app.hit_map);

    let focus_item = app
        .focused_item
        .and_then(|i| app.focus_list().get(i).cloned());
    viewer::draw(
        frame,
        regions.viewer,
        app.doc.lines(),
        app.doc.link_spans(),
        app.scroll,
        app.cursor_line,
        app.focus == FocusPane::Viewer,
        focus_item.as_ref(),
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
    let footer_focus = focus_item.as_ref().and_then(|it| match it.kind {
        FocusTarget::FooterPrev | FocusTarget::FooterNext => Some(it.kind),
        FocusTarget::Link => None,
    });
    footer::draw(
        frame,
        regions.footer,
        prev_label.as_deref(),
        next_label.as_deref(),
        footer_focus,
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
            &nav.cursor,
            app.nav_scroll,
            app.focus == FocusPane::Nav,
            &theme,
            &mut app.hit_map,
        );
    }

    let total = u32::try_from(app.doc.lines().len().max(1)).unwrap_or(1);
    let pct = ((app.scroll.saturating_add(1)) * 100) / total;
    let path = page.relative_path.display().to_string();
    let focus_target = focused_status_message(app);
    let status_msg = if app.message.is_empty() {
        focus_target.as_str()
    } else {
        app.message.as_str()
    };
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
            message: status_msg,
        },
        &theme,
    );
}

fn focused_status_message(app: &App) -> String {
    let Some(i) = app.focused_item else {
        return String::new();
    };
    let items = app.focus_list();
    let Some(it) = items.get(i) else {
        return String::new();
    };
    let page = &app.navigator.tab().current().page;
    match it.kind {
        FocusTarget::Link => format_target_with_provider(
            &it.target,
            page,
            app.navigator.index(),
            Some(&app.provider),
        ),
        FocusTarget::FooterPrev => "‹ prev".into(),
        FocusTarget::FooterNext => "next ›".into(),
    }
}
