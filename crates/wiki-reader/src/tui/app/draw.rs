//! Frame draw: regions + status.

use ratatui::Frame;

use super::App;
use crate::tui::focus::FocusPane;
use crate::tui::layout;
use crate::tui::page_doc::PageDoc;
use crate::tui::regions::status::StatusModel;
use crate::tui::regions::{footer, header, side_nav, status, tabs, viewer};
use crate::tui::viewer_doc::{FocusTarget, ViewerDoc, format_target_with_provider};

/// Draw all regions and rebuild the hit map.
#[allow(clippy::too_many_lines)] // layout + tab bar
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    app.hit_map.clear();
    let area = frame.area();
    app.sync_nav_for_width(area.width);
    let regions = layout::split(area, app.nav_visible);
    let text_width = regions.viewer.width.saturating_sub(2).min(100);
    app.ensure_layout_width(text_width);
    // Borders (2) + search row (1); remaining rows show the tree.
    app.nav_viewport = regions.side_nav.height.saturating_sub(3).max(1);
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();

    header::draw(frame, regions.header, &crumbs, &theme, &mut app.hit_map);

    // Tab bar sits above the viewer only when ≥2 tabs.
    let viewer_area = if app.navigator.tab_count() >= 2 {
        let split = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(regions.viewer);
        tabs::draw(
            frame,
            split[0],
            app.navigator.tabs(),
            app.navigator.active(),
            &theme,
            &mut app.hit_map,
        );
        split[1]
    } else {
        regions.viewer
    };
    app.viewer_rows = viewer_area.height.max(1);

    let focus_item = app
        .focused_item
        .and_then(|i| app.focus_list().get(i).cloned());
    let (highlights, gutter) = match &app.doc {
        crate::tui::page_doc::PageDoc::Raw(d) => (Some(d.highlights.as_slice()), true),
        crate::tui::page_doc::PageDoc::Rendered(_) => (None, false),
    };
    viewer::draw(
        frame,
        viewer_area,
        app.doc.lines(),
        app.doc.styled_lines(),
        app.doc.link_spans(),
        highlights,
        gutter,
        app.scroll,
        app.cursor_line,
        app.match_highlight,
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
        FocusTarget::Link | FocusTarget::BlockAction => None,
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
            app.navigator.label_mode(),
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

    if let Some(overlay) = &app.search {
        draw_search_overlay(frame, area, overlay, &theme, &mut app.hit_map);
    }
}

fn draw_search_overlay(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    overlay: &crate::tui::search_ui::SearchOverlay,
    theme: &crate::tui::theme::Theme,
    hits: &mut crate::tui::hit::HitMap,
) {
    use crate::tui::hit::Hit;
    use crate::tui::search_ui::SearchMode;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

    // Full-frame dismiss hit under the panel.
    hits.push(area, Hit::SearchDismiss);

    let width = area.width.clamp(20, 60);
    let height = area.height.clamp(8, 16);
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect {
        x,
        y,
        width,
        height,
    };
    frame.render_widget(Clear, rect);
    let mode = match overlay.mode {
        SearchMode::Pages => "Pages",
        SearchMode::Text => "Text",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(format!(" Search [{mode}] (Tab) "));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    // Panel itself absorbs clicks (no dismiss).
    hits.push(rect, Hit::FocusViewer);
    let query_line = Paragraph::new(Line::from(vec![
        Span::raw("> "),
        Span::styled(overlay.query.clone(), theme.text()),
        Span::raw("█"),
    ]));
    let q_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(query_line, q_rect);

    let list_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    let items: Vec<ListItem> = match overlay.mode {
        SearchMode::Pages => overlay
            .page_hits
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mark = if i == overlay.selected { "› " } else { "  " };
                ListItem::new(format!("{mark}{}", h.page.relative_path.display()))
            })
            .collect(),
        SearchMode::Text => overlay
            .text_hits
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mark = if i == overlay.selected { "› " } else { "  " };
                ListItem::new(format!(
                    "{mark}{}:{} {}",
                    h.page.relative_path.display(),
                    h.line,
                    h.snippet
                ))
            })
            .collect(),
    };
    let visible = usize::from(list_rect.height);
    for i in 0..items.len().min(visible) {
        let row = ratatui::layout::Rect {
            x: list_rect.x,
            y: list_rect.y.saturating_add(u16::try_from(i).unwrap_or(0)),
            width: list_rect.width,
            height: 1,
        };
        hits.push(row, Hit::SearchResult(i));
    }
    frame.render_widget(List::new(items), list_rect);
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
        FocusTarget::BlockAction => block_action_status(app, &it.target),
        FocusTarget::FooterPrev => "‹ prev".into(),
        FocusTarget::FooterNext => "next ›".into(),
    }
}

fn block_action_status(app: &App, target: &str) -> String {
    let Some(id_str) = target.strip_prefix("block:") else {
        return String::new();
    };
    let Ok(id) = id_str.parse::<u32>() else {
        return String::new();
    };
    let PageDoc::Rendered(doc) = &app.doc else {
        return String::new();
    };
    let Some(action) = doc.block_actions().iter().find(|a| a.id == id) else {
        return String::new();
    };
    match action.kind {
        wiki_reader_render::BlockActionKind::ToggleFrontmatter => {
            if app.expanded_blocks.contains(&id) {
                "collapse frontmatter".into()
            } else {
                "expand frontmatter".into()
            }
        }
        wiki_reader_render::BlockActionKind::CopyCode => "copy code".into(),
    }
}
