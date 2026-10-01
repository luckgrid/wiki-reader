//! Frame draw: regions + status.

use ratatui::Frame;

use super::App;
use crate::tui::focus::FocusPane;
use crate::tui::layout;
use crate::tui::page_doc::PageDoc;
use crate::tui::regions::status::StatusModel;
use crate::tui::regions::{header, side_nav, status, viewer};
use crate::tui::viewer_doc::{FocusTarget, ViewerDoc, format_target_with_provider};

/// Draw all regions and rebuild the hit map.
#[allow(clippy::too_many_lines)] // layout + tab bar
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    app.hit_map.clear();
    let area = frame.area();
    app.sync_nav_for_width(area.width);
    let regions = layout::split(area, app.nav_visible, app.nav_width);
    let text_width = layout::viewer_text_width(regions.viewer);
    app.ensure_layout_width(text_width);
    // Only a drawn nav has a real viewport. While it is hidden the last value
    // stays, so footer navigation can't scroll the list against a 1-row pane.
    if regions.side_nav.width > 0 {
        let viewport = regions
            .side_nav
            .height
            .saturating_sub(layout::NAV_CHROME_ROWS)
            .max(1);
        let resized = app.nav_viewport != viewport;
        app.nav_viewport = viewport;
        if resized {
            // A taller or shorter pane (resize): keep the cursor row on screen.
            app.ensure_nav_cursor_visible();
        }
        // A list that fits always starts right under the search bar.
        app.clamp_nav_scroll();
    }
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();
    // A popup owns the keys: both panes behind it drop their active colours.
    let overlay_open = app.search.is_some() || app.help.is_some();

    header::draw(frame, regions.header, &crumbs, &theme, &mut app.hit_map);

    let viewer_area = regions.viewer;
    app.viewer_rows = layout::viewer_visible_rows(viewer_area.height).max(1);

    let focus_items = app.focus_list();
    let focus_item = app.focused_item.and_then(|i| focus_items.get(i).cloned());
    let (highlights, gutter) = match &app.doc {
        crate::tui::page_doc::PageDoc::Raw(d) => (Some(d.highlights.as_slice()), Some(d.numbers())),
        crate::tui::page_doc::PageDoc::Rendered(_) => (None, None),
    };
    let prev = nav.tree.prev(&page);
    let next = nav.tree.next(&page);
    let footer_label = |k: &wiki_reader_core::provider::PageKey| {
        nav.tree.page_display_label(k).unwrap_or_else(|| {
            wiki_reader_core::nav::page_label_with(
                app.navigator.index(),
                k,
                app.navigator.label_mode(),
            )
        })
    };
    let prev_label = prev.as_ref().map(footer_label);
    let next_label = next.as_ref().map(footer_label);
    let footer_focus = focus_item.as_ref().and_then(|it| match it.kind {
        FocusTarget::FooterPrev | FocusTarget::FooterNext => Some(it.kind),
        FocusTarget::Link | FocusTarget::BlockAction => None,
    });
    let match_spans = app.match_spans();
    app.viewer_geom = viewer::draw(
        frame,
        viewer_area,
        app.doc.lines(),
        app.doc.styled_lines(),
        app.doc.link_spans(),
        highlights,
        gutter,
        app.scroll,
        app.cursor_line,
        app.effective_col(),
        app.selection,
        &match_spans,
        app.focus == FocusPane::Viewer && !overlay_open,
        focus_item.as_ref(),
        &focus_items,
        prev_label.as_deref(),
        next_label.as_deref(),
        footer_focus,
        app.navigator.tabs(),
        app.navigator.active(),
        &theme,
        &mut app.hit_map,
    );

    if regions.side_nav.width > 0 {
        side_nav::draw(
            frame,
            regions.side_nav,
            &nav.tree,
            &nav.expanded,
            &nav.cursor,
            app.nav_scroll,
            app.focus == FocusPane::Nav && !overlay_open,
            app.navigator.label_mode(),
            &theme,
            &mut app.hit_map,
        );
    }

    let total = u32::try_from(app.doc.lines().len().max(1)).unwrap_or(1);
    let pct = ((app.scroll.saturating_add(1)) * 100) / total;
    let path = page.relative_path.display().to_string();
    let page_status = app
        .navigator
        .index()
        .pages
        .get(&page)
        .and_then(|p| p.parsed.frontmatter.status.clone());
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
            mode_label: if app.search.is_some() {
                "SEARCH"
            } else if app.help.is_some() {
                "HELP"
            } else {
                app.focus.label()
            },
            path: &path,
            // Raw rows soft-wrap, so report the source line there.
            line: match &app.doc {
                PageDoc::Raw(d) => d.source_line_of_row(app.cursor_line),
                PageDoc::Rendered(_) => app.cursor_line,
            }
            .saturating_add(1),
            col: app.effective_col().saturating_add(1),
            pct,
            words: app.doc.word_count(),
            minutes: status::reading_minutes(app.doc.word_count()),
            updated: app.doc.updated(),
            status: page_status.as_deref(),
            message: status_msg,
        },
        &theme,
    );

    if let Some(overlay) = app.search.as_mut() {
        draw_search_overlay(
            frame,
            area,
            overlay,
            app.navigator.index(),
            &theme,
            &mut app.hit_map,
        );
    }
    if let Some(help) = &mut app.help {
        draw_help_overlay(frame, area, help, &theme, &mut app.hit_map);
    }
}

#[allow(clippy::too_many_lines)]
fn draw_search_overlay(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    overlay: &mut crate::tui::search_ui::SearchOverlay,
    index: &wiki_reader_core::Index,
    theme: &crate::tui::theme::Theme,
    hits: &mut crate::tui::hit::HitMap,
) {
    use crate::tui::hit::Hit;
    use crate::tui::regions::overlay::{
        centered_panel, clamp_scroll, ensure_visible, popup_accent, popup_block, popup_row,
    };
    use crate::tui::search_ui::SearchMode;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Clear, Paragraph};

    hits.push(area, Hit::SearchDismiss);

    // ~80% width up to ~100 cols, ~80% height.
    let max_w = (u32::from(area.width) * 80 / 100).clamp(40, 100) as u16;
    let max_h = (u32::from(area.height) * 80 / 100).clamp(10, 50) as u16;
    let rect = centered_panel(area, max_w, max_h, 40, 10);
    frame.render_widget(Clear, rect);
    let mode_tag = match overlay.mode {
        SearchMode::Files => "[Files]",
        SearchMode::Content => "[Content]",
    };
    let title = Line::from(vec![
        Span::styled(" Search ", popup_accent(theme)),
        Span::styled(mode_tag, theme.text().add_modifier(Modifier::BOLD)),
        Span::styled(" (Tab: toggle mode  Esc: close) ", theme.muted()),
    ]);
    let block = popup_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    hits.push(rect, Hit::FocusViewer);

    // Blank row, query row, results, footer, blank row.
    if inner.height < 5 || inner.width == 0 {
        return;
    }
    let row_at = |dy: u16| Rect {
        x: inner.x,
        y: inner.y.saturating_add(dy),
        width: inner.width,
        height: 1,
    };

    // `/` is the key that opens search; the placeholder sits beside the cursor.
    let mut query = vec![
        Span::styled("/ ", popup_accent(theme)),
        Span::styled(overlay.query.clone(), theme.text()),
        Span::styled("█", theme.text()),
    ];
    if overlay.query.is_empty() {
        query.push(Span::styled(" type to search…", theme.muted()));
    }
    frame.render_widget(
        Paragraph::new(popup_row(query, inner.width, None)),
        row_at(1),
    );

    let n = overlay.result_len();
    // Blank row below the footer, mirroring the one above the input.
    let footer_y = inner.height - 2;
    let count = if overlay.query.is_empty() {
        "type to search".to_owned()
    } else {
        match overlay.mode {
            SearchMode::Files => format!("{n} file{}", if n == 1 { "" } else { "s" }),
            SearchMode::Content => {
                let files: std::collections::HashSet<_> =
                    overlay.text_hits.iter().map(|h| &h.page).collect();
                format!(
                    "{} file{}, {n} match{}",
                    files.len(),
                    if files.len() == 1 { "" } else { "s" },
                    if n == 1 { "" } else { "es" }
                )
            }
        }
    };
    let footer = vec![
        Span::styled(count, theme.text()),
        Span::styled(
            "  ↑/↓: navigate  Enter: open  Tab: toggle mode",
            theme.muted(),
        ),
    ];
    frame.render_widget(
        Paragraph::new(popup_row(footer, inner.width, None)),
        row_at(footer_y),
    );

    let list_rect = Rect {
        x: inner.x,
        y: inner.y.saturating_add(2),
        width: inner.width,
        height: inner.height.saturating_sub(4),
    };
    let visible = usize::from(list_rect.height);
    overlay.list_height = visible;
    if visible == 0 {
        return;
    }
    overlay.scroll = ensure_visible(overlay.selected, overlay.scroll, visible);
    overlay.scroll = clamp_scroll(overlay.scroll, visible, n);

    if n == 0 {
        if !overlay.query.is_empty() {
            frame.render_widget(
                Paragraph::new(popup_row(
                    vec![Span::styled("No matches.", theme.muted())],
                    list_rect.width,
                    None,
                )),
                list_rect,
            );
        }
        return;
    }

    let q = overlay.query.to_lowercase();
    for row_i in 0..visible {
        let abs = overlay.scroll + row_i;
        if abs >= n {
            break;
        }
        let y = list_rect
            .y
            .saturating_add(u16::try_from(row_i).unwrap_or(0));
        let row_rect = Rect {
            x: list_rect.x,
            y,
            width: list_rect.width,
            height: 1,
        };
        let selected = abs == overlay.selected;
        let bg = selected.then_some(theme.cursor_line);
        let spans = match overlay.mode {
            SearchMode::Files => {
                let hit = &overlay.page_hits[abs];
                let title = index.pages.get(&hit.page).map_or("", |p| p.title.as_str());
                let path = hit.page.relative_path.display().to_string();
                search_result_spans(None, title, &path, None, &q, selected, theme)
            }
            SearchMode::Content => {
                let hit = &overlay.text_hits[abs];
                let title = index.pages.get(&hit.page).map_or("", |p| p.title.as_str());
                let path = hit.page.relative_path.display().to_string();
                search_result_spans(
                    Some(hit.line),
                    title,
                    &path,
                    Some(&hit.snippet),
                    &q,
                    selected,
                    theme,
                )
            }
        };
        frame.render_widget(
            Paragraph::new(popup_row(spans, row_rect.width, bg)),
            row_rect,
        );
        hits.push(row_rect, Hit::SearchResult(abs));
    }
}

/// One result row: `[line]` (Content), bold title, muted path, snippet — each
/// part in its own colour, query matches underlined.
fn search_result_spans(
    line: Option<u32>,
    title: &str,
    path: &str,
    snippet: Option<&str>,
    query_lower: &str,
    selected: bool,
    theme: &crate::tui::theme::Theme,
) -> Vec<ratatui::text::Span<'static>> {
    use crate::tui::regions::overlay::popup_accent;
    use ratatui::style::Modifier;
    use ratatui::text::Span;

    let title_style = if selected {
        theme.text().add_modifier(Modifier::BOLD)
    } else {
        theme.text()
    };
    let mut spans = Vec::new();
    if let Some(line) = line {
        spans.push(Span::styled(format!("[{line}] "), popup_accent(theme)));
    }
    spans.extend(highlight_spans(
        title,
        query_lower,
        title_style.add_modifier(Modifier::BOLD),
        title_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ));
    spans.push(Span::raw("  "));
    spans.extend(highlight_spans(
        path,
        query_lower,
        theme.muted(),
        theme.muted().add_modifier(Modifier::UNDERLINED),
    ));
    if let Some(snip) = snippet {
        spans.push(Span::styled(" – ", theme.muted()));
        spans.extend(highlight_spans(
            snip,
            query_lower,
            theme.secondary(),
            theme.text().add_modifier(Modifier::UNDERLINED),
        ));
    }
    spans
}

fn highlight_spans(
    text: &str,
    query_lower: &str,
    normal: ratatui::style::Style,
    match_style: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::text::Span;
    if query_lower.is_empty() {
        return vec![Span::styled(text.to_owned(), normal)];
    }
    let lower = text.to_lowercase();
    let mut spans = Vec::new();
    let mut rest = text;
    let mut rest_lower = lower.as_str();
    while let Some(pos) = rest_lower.find(query_lower) {
        if pos > 0 {
            spans.push(Span::styled(rest[..pos].to_owned(), normal));
        }
        let end = pos + query_lower.len();
        spans.push(Span::styled(rest[pos..end].to_owned(), match_style));
        rest = &rest[end..];
        rest_lower = &rest_lower[end..];
    }
    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_owned(), normal));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_owned(), normal));
    }
    spans
}

fn draw_help_overlay(
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    help: &mut crate::tui::help_ui::HelpOverlay,
    theme: &crate::tui::theme::Theme,
    hits: &mut crate::tui::hit::HitMap,
) {
    use crate::tui::hit::Hit;
    use crate::tui::regions::footer::ellipsis;
    use crate::tui::regions::overlay::{
        POPUP_PAD, centered_panel, clamp_scroll, ensure_visible, popup_accent, popup_block,
        popup_row,
    };
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Clear, Paragraph};

    hits.push(area, Hit::HelpDismiss);

    // Tall and thin: nearly the full height, a narrow column.
    let max_h = area.height.saturating_sub(2).clamp(10, 60);
    let rect = centered_panel(area, 58, max_h, 40, 10);
    frame.render_widget(Clear, rect);
    let title = Line::from(Span::styled(
        " Help (? or Esc to close) ",
        popup_accent(theme),
    ));
    let block = popup_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    hits.push(rect, Hit::FocusViewer);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // One blank row top and bottom: about the same visual size as the side padding.
    let visible = usize::from(inner.height.saturating_sub(2));
    if visible == 0 {
        return;
    }
    help.list_height = visible;
    help.scroll = ensure_visible(help.selected, help.scroll, visible);
    help.scroll = clamp_scroll(help.scroll, visible, help.rows.len());

    // Key column: keys, and a header icon that triggers the same action joined
    // with ` / ` like any other alternate key.
    let key_label = |row: &crate::tui::help_ui::HelpRow| match row.icon {
        Some(icon) => format!("{} / {icon}", row.keys),
        None => row.keys.clone(),
    };
    let content_w = usize::from(inner.width.saturating_sub(POPUP_PAD * 2));
    let key_w = help
        .rows
        .iter()
        .filter(|r| r.selectable())
        .map(|r| Span::raw(key_label(r)).width())
        .max()
        .unwrap_or(8)
        .clamp(8, content_w / 2);

    for row_i in 0..visible {
        let abs = help.scroll + row_i;
        let Some(row) = help.rows.get(abs) else {
            break;
        };
        if row.spacer {
            continue;
        }
        let y = inner
            .y
            .saturating_add(1)
            .saturating_add(u16::try_from(row_i).unwrap_or(0));
        let row_rect = ratatui::layout::Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        let selected = abs == help.selected;
        if row.heading {
            // Full-width divider: `── Title ─────────`.
            let used = 4 + Span::raw(row.keys.as_str()).width();
            let fill = content_w.saturating_sub(used);
            let spans = vec![
                Span::styled("── ", theme.muted()),
                Span::styled(row.keys.clone(), popup_accent(theme)),
                Span::styled(format!(" {}", "─".repeat(fill)), theme.muted()),
            ];
            frame.render_widget(
                Paragraph::new(popup_row(spans, inner.width, None)),
                row_rect,
            );
            continue;
        }
        let bg = selected.then_some(theme.cursor_line);
        let key_style = if selected {
            theme.text().add_modifier(Modifier::BOLD)
        } else {
            theme.text()
        };
        let keys = ellipsis(&key_label(row), key_w);
        let pad = key_w.saturating_sub(Span::raw(keys.as_str()).width());
        let desc = ellipsis(row.help, content_w.saturating_sub(key_w + 2));
        let spans = vec![
            Span::styled(keys, key_style),
            Span::raw(" ".repeat(pad + 2)),
            Span::styled(desc, theme.muted()),
        ];
        frame.render_widget(Paragraph::new(popup_row(spans, inner.width, bg)), row_rect);
        if row.action.is_some() {
            hits.push(row_rect, Hit::HelpRow(abs));
        }
    }
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
