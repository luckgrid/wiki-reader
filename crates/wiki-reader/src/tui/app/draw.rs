//! Frame draw: regions + status.

use ratatui::Frame;

use super::App;
use crate::tui::focus::FocusPane;
use crate::tui::layout;
use crate::tui::page_doc::PageDoc;
use crate::tui::regions::status::StatusModel;
use crate::tui::regions::{header, side_nav, status, tabs, viewer};
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
    app.nav_viewport = regions
        .side_nav
        .height
        .saturating_sub(layout::NAV_CHROME_ROWS)
        .max(1);
    let theme = app.theme;
    let page = app.navigator.tab().current().page.clone();
    let crumbs = app.navigator.nav().tree.breadcrumb(&page);
    let nav = app.navigator.nav();

    header::draw(
        frame,
        regions.header,
        &crumbs,
        app.formatted_view,
        &theme,
        &mut app.hit_map,
    );

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
    app.viewer_rows = layout::viewer_visible_rows(viewer_area.height).max(1);

    let focus_items = app.focus_list();
    let focus_item = app.focused_item.and_then(|i| focus_items.get(i).cloned());
    let (highlights, gutter) = match &app.doc {
        crate::tui::page_doc::PageDoc::Raw(d) => (Some(d.highlights.as_slice()), true),
        crate::tui::page_doc::PageDoc::Rendered(_) => (None, false),
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
        &focus_items,
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
            focus: app.focus,
            path: &path,
            line: app.cursor_line.saturating_add(1),
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
    use crate::tui::regions::overlay::{centered_panel, clamp_scroll, ensure_visible};
    use crate::tui::search_ui::SearchMode;
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    hits.push(area, Hit::SearchDismiss);

    // ~80% width up to ~100 cols, ~70% height.
    let max_w = (u32::from(area.width) * 80 / 100).clamp(40, 100) as u16;
    let max_h = (u32::from(area.height) * 70 / 100).clamp(10, 40) as u16;
    let rect = centered_panel(area, max_w, max_h, 40, 10);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(" Search ");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    hits.push(rect, Hit::FocusViewer);

    if inner.height < 3 || inner.width == 0 {
        return;
    }

    // Files | Content header.
    let files_active = matches!(overlay.mode, SearchMode::Files);
    let mode_spans = vec![
        Span::styled(
            " Files ",
            if files_active {
                theme
                    .text()
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                theme.muted()
            },
        ),
        Span::raw(" "),
        Span::styled(
            " Content ",
            if files_active {
                theme.muted()
            } else {
                theme
                    .text()
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            },
        ),
        Span::styled("  (Tab)", theme.muted()),
    ];
    let header_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(Line::from(mode_spans)), header_rect);

    let query_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("> "),
            Span::styled(overlay.query.clone(), theme.text()),
            Span::raw("█"),
        ])),
        query_rect,
    );

    let n = overlay.result_len();
    let count_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y.saturating_add(2),
        width: inner.width,
        height: 1,
    };
    let count_label = if overlay.query.is_empty() {
        "type to search".to_string()
    } else if n == 0 {
        "no results".to_string()
    } else {
        format!("{n} result{}", if n == 1 { "" } else { "s" })
    };
    frame.render_widget(
        Paragraph::new(Span::styled(count_label, theme.muted())),
        count_rect,
    );

    let list_rect = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y.saturating_add(3),
        width: inner.width,
        height: inner.height.saturating_sub(3),
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
                Paragraph::new(Span::styled("No matches.", theme.muted())),
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
        let row_rect = ratatui::layout::Rect {
            x: list_rect.x,
            y,
            width: list_rect.width,
            height: 1,
        };
        let selected = abs == overlay.selected;
        let bg = if selected {
            theme.cursor_line
        } else {
            theme.surface
        };
        let line = match overlay.mode {
            SearchMode::Files => {
                let hit = &overlay.page_hits[abs];
                let title = index.pages.get(&hit.page).map_or("", |p| p.title.as_str());
                let path = hit.page.relative_path.display().to_string();
                search_result_line(title, &path, None, &q, selected, bg, theme)
            }
            SearchMode::Content => {
                let hit = &overlay.text_hits[abs];
                let title = index.pages.get(&hit.page).map_or("", |p| p.title.as_str());
                let path = hit.page.relative_path.display().to_string();
                let snip = format!("{}:{}", hit.line, hit.snippet);
                search_result_line(title, &path, Some(&snip), &q, selected, bg, theme)
            }
        };
        frame.render_widget(Paragraph::new(line), row_rect);
        hits.push(row_rect, Hit::SearchResult(abs));
    }
}

fn search_result_line(
    title: &str,
    path: &str,
    snippet: Option<&str>,
    query_lower: &str,
    selected: bool,
    bg: ratatui::style::Color,
    theme: &crate::tui::theme::Theme,
) -> ratatui::text::Line<'static> {
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};

    let mark = if selected { "› " } else { "  " };
    let mut spans = vec![Span::styled(mark, theme.text().bg(bg))];
    spans.extend(highlight_spans(
        title,
        query_lower,
        theme.text().bg(bg).add_modifier(Modifier::BOLD),
        theme
            .text()
            .bg(bg)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ));
    spans.push(Span::styled("  ", theme.muted().bg(bg)));
    spans.extend(highlight_spans(
        path,
        query_lower,
        theme.muted().bg(bg),
        theme.muted().bg(bg).add_modifier(Modifier::UNDERLINED),
    ));
    if let Some(snip) = snippet {
        spans.push(Span::styled("  ", theme.muted().bg(bg)));
        spans.extend(highlight_spans(
            snip,
            query_lower,
            theme.text().bg(bg),
            theme.text().bg(bg).add_modifier(Modifier::UNDERLINED),
        ));
    }
    Line::from(spans)
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
    use crate::tui::regions::overlay::{centered_panel, clamp_scroll, ensure_visible};
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    hits.push(area, Hit::HelpDismiss);

    let rect = centered_panel(area, 72, 22, 40, 10);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(true))
        .title(" Help (? or Esc to close) ");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    hits.push(rect, Hit::FocusViewer);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible = usize::from(inner.height);
    help.list_height = visible;
    help.scroll = ensure_visible(help.selected, help.scroll, visible);
    help.scroll = clamp_scroll(help.scroll, visible, help.rows.len());

    for row_i in 0..visible {
        let abs = help.scroll + row_i;
        let Some(row) = help.rows.get(abs) else {
            break;
        };
        let y = inner.y.saturating_add(u16::try_from(row_i).unwrap_or(0));
        let row_rect = ratatui::layout::Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        let selected = abs == help.selected;
        let bg = if selected {
            theme.cursor_line
        } else {
            theme.surface
        };
        if row.heading {
            // Full-width divider: `── Title ─────────`.
            let used = 4 + Span::raw(row.keys.as_str()).width();
            let fill = usize::from(inner.width).saturating_sub(used);
            let line = Line::from(vec![
                Span::styled("── ", theme.muted()),
                Span::styled(
                    row.keys.clone(),
                    theme.accent().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!(" {}", "─".repeat(fill)), theme.muted()),
            ]);
            frame.render_widget(Paragraph::new(line), row_rect);
            continue;
        }
        let style = if selected {
            theme.text().bg(bg).add_modifier(Modifier::BOLD)
        } else {
            theme.text().bg(bg)
        };
        let spans = vec![
            Span::styled(format!("{:<14}", row.keys), style),
            Span::styled(
                format!("{:<4}", row.icon.unwrap_or("")),
                theme.accent().bg(bg),
            ),
            Span::styled(row.help, theme.muted().bg(bg)),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)), row_rect);
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
