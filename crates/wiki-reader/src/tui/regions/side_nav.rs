//! Side nav: flat visible rows into our `HitMap` ([ADR-0010](../../../../wiki/decisions/0010-flat-side-nav-rows.md)).
//!
//! ADR-0009 chose `tui-tree-widget`; P1-07c shipped flat rows instead because
//! syncing `TreeState` from core `NavState` each frame duplicated the source of truth.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use wiki_reader_core::nav::{NavItem, NavStop, NavTree, NodeId};
use wiki_reader_core::provider::PageKey;

use crate::tui::hit::{Hit, HitMap};
use crate::tui::layout::NAV_CHROME_ROWS;
use crate::tui::theme::Theme;

/// One visible side-nav row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavRow {
    /// Node id (page or group).
    pub id: NodeId,
    /// Display label (with indent / triangle).
    pub label: String,
    /// True when this row is a collapsible group header.
    pub is_group: bool,
}

/// Flatten the tree to currently visible rows (respecting `expanded`).
#[must_use]
pub fn visible_rows(tree: &NavTree, expanded: &std::collections::HashSet<NodeId>) -> Vec<NavRow> {
    let mut rows = Vec::new();
    collect_visible(&tree.items, expanded, 0, &mut rows);
    rows
}

/// Draw search row + flat visible tree rows; register hits.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    tree: &NavTree,
    expanded: &std::collections::HashSet<NodeId>,
    current: &PageKey,
    cursor: &NavStop,
    scroll: u16,
    focused: bool,
    labels: wiki_reader_core::config::LabelMode,
    theme: &Theme,
    hits: &mut HitMap,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    hits.push(area, Hit::FocusNav);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(focused))
        .title("Nav");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Inner chrome: blank, search box, blank — then the tree list.
    // NAV_CHROME_ROWS = borders(2) + these three rows.
    let list_offset: u16 = 3;
    debug_assert_eq!(NAV_CHROME_ROWS, 2 + list_offset);

    let search_y = inner.y.saturating_add(1);
    let search_rect = Rect {
        x: inner.x,
        y: search_y,
        width: inner.width,
        height: 1,
    };
    hits.push(search_rect, Hit::NavSearchRow);
    let on_search = matches!(cursor, NavStop::Search);
    let search_bg = if on_search {
        theme.cursor_line
    } else {
        theme.search_box
    };
    let search_style = if on_search {
        theme.text().bg(search_bg)
    } else {
        theme.muted().bg(search_bg)
    };
    let search_mark = if on_search { "▌" } else { " " };
    let search_label = format!("{search_mark}⌕ Search…");
    let search_w = Span::raw(search_label.as_str()).width();
    let mut search_spans = vec![Span::styled(search_label, search_style)];
    if search_w < usize::from(inner.width) {
        search_spans.push(Span::styled(
            " ".repeat(usize::from(inner.width) - search_w),
            Style::default().bg(search_bg),
        ));
    }

    let rows = visible_rows(tree, expanded);
    let scroll = usize::from(scroll);
    let mut lines = vec![
        Line::from(""), // gap under title
        Line::from(search_spans),
        Line::from(""), // gap under search
    ];
    let max_rows = usize::from(inner.height.saturating_sub(list_offset));
    for (i, row) in rows.into_iter().skip(scroll).take(max_rows).enumerate() {
        let y = inner
            .y
            .saturating_add(list_offset)
            .saturating_add(u16::try_from(i).unwrap_or(0));
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        if row.is_group {
            hits.push(rect, Hit::NavGroupToggle(row.id.clone()));
        } else {
            hits.push(rect, Hit::NavItem(row.id.clone()));
        }
        let marker = match &row.id {
            NodeId::Page(k) if k == current => "● ",
            _ => "  ",
        };
        let on_row = matches!(cursor, NavStop::Node(id) if id == &row.id);
        let cursor_mark = if on_row { "▌" } else { " " };
        let style = if on_row {
            theme.text().bg(theme.cursor_line)
        } else {
            theme.text()
        };
        let mut spans = vec![Span::styled(
            format!("{cursor_mark}{marker}{}", row.label),
            style,
        )];
        if labels == wiki_reader_core::config::LabelMode::TitleFilename
            && let NodeId::Page(key) = &row.id
        {
            let file = wiki_reader_core::nav::humanize_filename(&key.relative_path);
            let title = row.label.trim_start();
            if title != file {
                spans.push(Span::styled(format!(" ({file})"), theme.muted()));
            }
        }
        let max = usize::from(inner.width);
        let mut used = 0usize;
        let mut clipped = Vec::new();
        for sp in spans {
            let w = Span::raw(sp.content.as_ref()).width();
            if used >= max {
                break;
            }
            if used + w <= max {
                used += w;
                clipped.push(sp);
            } else {
                let take = max.saturating_sub(used);
                // Truncate by display width (char-based is wrong for wide glyphs).
                let mut text = String::new();
                let mut tw = 0usize;
                for ch in sp.content.chars() {
                    let cw = Span::raw(ch.to_string()).width();
                    if tw + cw > take {
                        break;
                    }
                    text.push(ch);
                    tw += cw;
                }
                clipped.push(Span::styled(text, sp.style));
                used = max;
                break;
            }
        }
        if on_row {
            let pad = max.saturating_sub(used);
            if pad > 0 {
                clipped.push(Span::styled(
                    " ".repeat(pad),
                    Style::default().bg(theme.cursor_line),
                ));
            }
        }
        lines.push(Line::from(clipped));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn collect_visible(
    items: &[NavItem],
    expanded: &std::collections::HashSet<NodeId>,
    depth: usize,
    out: &mut Vec<NavRow>,
) {
    let indent = "  ".repeat(depth);
    for item in items {
        match item {
            NavItem::Page { id, label, .. } => {
                out.push(NavRow {
                    id: id.clone(),
                    label: format!("{indent}{label}"),
                    is_group: false,
                });
            }
            NavItem::Group {
                id,
                label,
                children,
            } => {
                let open = expanded.contains(id);
                let tri = if open { "▾ " } else { "▸ " };
                out.push(NavRow {
                    id: id.clone(),
                    label: format!("{indent}{tri}{label}"),
                    is_group: true,
                });
                if open {
                    collect_visible(children, expanded, depth + 1, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use wiki_reader_core::Index;
    use wiki_reader_core::provider::FsProvider;

    fn worked_tree() -> NavTree {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let index = Index::build(&FsProvider::open(&root).unwrap()).unwrap();
        NavTree::build(&index)
    }

    #[test]
    fn visible_rows_collapsed_and_expanded() {
        let tree = worked_tree();
        let collapsed = visible_rows(&tree, &HashSet::new());
        assert!(collapsed.iter().any(|r| r.label.contains("Architecture")));
        assert!(!collapsed.iter().any(|r| r.label.contains("Token")));

        let mut expanded = HashSet::new();
        expanded.insert(NodeId::Group(PathBuf::from("architecture")));
        expanded.insert(NodeId::Group(PathBuf::from("architecture/design-system")));
        let open = visible_rows(&tree, &expanded);
        assert!(open.iter().any(|r| r.label.contains("Token")));
        insta::assert_debug_snapshot!(
            "worked_nav_expanded",
            open.iter().map(|r| &r.label).collect::<Vec<_>>()
        );
    }
}
