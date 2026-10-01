//! Viewer body: lines, link hits, optional raw gutter + syntect spans.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::highlight::HlSpan;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::layout::{VIEWER_LEFT_PAD, VIEWER_TOP_PAD};
use crate::tui::selection::Selection;
use crate::tui::text_col;
use crate::tui::theme::Theme;
use crate::tui::viewer_doc::{FocusItem, FocusTarget};
use wiki_reader_render::{LinkClass, LinkId, LinkSpan, StyleKind, StyledLine};

/// Where the View's text sits on screen (for mouse → document-cell mapping).
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewerGeom {
    /// Screen column of the first text cell (after marker and gutter).
    pub text_x: u16,
    /// Screen row of the first text row.
    pub top_y: u16,
    /// Visible text rows.
    pub rows: u16,
}

/// Restyle the cells in display columns `[from, to)` of a row's spans, splitting
/// spans at char boundaries. A wide glyph is patched whole if it starts in range.
fn patch_cols(
    spans: Vec<Span<'static>>,
    from: usize,
    to: usize,
    patch: Style,
) -> Vec<Span<'static>> {
    if from >= to {
        return spans;
    }
    let mut out: Vec<Span<'static>> = Vec::with_capacity(spans.len() + 2);
    let mut x = 0usize;
    for sp in spans {
        let w = Span::raw(sp.content.as_ref()).width();
        if x + w <= from || x >= to {
            x += w;
            out.push(sp);
            continue;
        }
        let mut run = String::new();
        let mut run_hit = false;
        let mut cx = x;
        for ch in sp.content.chars() {
            let hit = cx >= from && cx < to;
            if !run.is_empty() && hit != run_hit {
                let st = if run_hit {
                    sp.style.patch(patch)
                } else {
                    sp.style
                };
                out.push(Span::styled(std::mem::take(&mut run), st));
            }
            run_hit = hit;
            run.push(ch);
            cx += Span::raw(ch.to_string()).width().max(1);
        }
        if !run.is_empty() {
            let st = if run_hit {
                sp.style.patch(patch)
            } else {
                sp.style
            };
            out.push(Span::styled(run, st));
        }
        x += w;
    }
    out
}

/// Draw lines into the viewer and register link + line hits.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
    styled: Option<&[StyledLine]>,
    link_spans: &[LinkSpan],
    highlights: Option<&[Vec<HlSpan>]>,
    gutter: Option<&[Option<u32>]>,
    scroll: u32,
    cursor_line: u32,
    cursor_col: u16,
    selection: Option<Selection>,
    match_span: Option<(u32, u16, u16)>,
    focused: bool,
    focused_item: Option<&FocusItem>,
    focus_items: &[FocusItem],
    prev_label: Option<&str>,
    next_label: Option<&str>,
    footer_focus: Option<FocusTarget>,
    tabs: &[wiki_reader_core::nav::Tab],
    active_tab: usize,
    theme: &Theme,
    hits: &mut HitMap,
) -> ViewerGeom {
    if area.width == 0 || area.height == 0 {
        return ViewerGeom::default();
    }

    hits.push(area, Hit::FocusViewer);

    let footer = crate::tui::regions::footer::titles(
        area,
        prev_label,
        next_label,
        footer_focus,
        focused,
        theme,
        hits,
    );

    let tab_bar = crate::tui::regions::tabs::titles(area, tabs, active_tab, focused, theme, hits);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(focused))
        .title(tab_bar)
        .title_bottom(footer.left)
        .title_bottom(footer.right);
    let bordered = block.inner(area);
    frame.render_widget(block, area);
    // Clamp so the pad can be flipped back to a non-zero value without a rewrite.
    #[allow(clippy::unnecessary_min_or_max)]
    let top_pad = VIEWER_TOP_PAD.min(bordered.height);
    let inner_area = Rect {
        y: bordered.y.saturating_add(top_pad),
        height: bordered.height.saturating_sub(top_pad),
        ..bordered
    };

    // Left pad column holds the ▌ cursor marker (P2-19); text starts one col in.
    let content = Rect {
        x: inner_area.x.saturating_add(VIEWER_LEFT_PAD),
        y: inner_area.y,
        width: inner_area.width.saturating_sub(VIEWER_LEFT_PAD),
        height: inner_area.height,
    };

    let show_gutter = gutter.is_some();
    let gutter_w: u16 = if show_gutter { 6 } else { 0 };
    let geom = ViewerGeom {
        text_x: content.x.saturating_add(gutter_w),
        top_y: inner_area.y,
        rows: inner_area.height,
    };
    // Columns before the text in each painted row: marker + gutter.
    let origin = usize::from(VIEWER_LEFT_PAD + gutter_w);
    let text_width = usize::from(content.width.saturating_sub(gutter_w)).min(100);
    let visible_h = usize::from(content.height);
    let scroll = usize::try_from(scroll).unwrap_or(0);
    let mut out_lines = Vec::with_capacity(visible_h);
    let focus_style = Style::default().bg(theme.focus_item).fg(theme.text);
    let focused_link = focused_item.and_then(|it| it.link_id);
    let gutter_style = theme.muted();
    let cursor_bg = Style::default().bg(theme.cursor_line).fg(theme.text);

    for row in 0..visible_h {
        let src_idx = scroll + row;
        let line_no = u32::try_from(src_idx).unwrap_or(u32::MAX);
        let y = content
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));

        let on_cursor = line_no == cursor_line;
        let base = if on_cursor { cursor_bg } else { theme.text() };

        hits.push(
            Rect {
                x: inner_area.x,
                y,
                width: inner_area.width,
                height: 1,
            },
            Hit::ViewerLine(line_no),
        );

        for span in link_spans {
            for &(seg_line, (c0, c1)) in &span.segments {
                if seg_line == line_no && c0 < c1 {
                    let x0 = gutter_w.saturating_add(c0);
                    let x1 = gutter_w.saturating_add(c1).min(content.width);
                    if x0 >= content.width || x1 <= x0 {
                        continue;
                    }
                    hits.push(
                        Rect {
                            x: content.x.saturating_add(x0),
                            y,
                            width: x1.saturating_sub(x0),
                            height: 1,
                        },
                        Hit::Link(span.id.0),
                    );
                }
            }
        }

        for it in focus_items {
            if it.kind != FocusTarget::BlockAction || it.line != Some(line_no) {
                continue;
            }
            let Some(id) = it
                .target
                .strip_prefix("block:")
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            let x0 = gutter_w.saturating_add(it.cols.0);
            let x1 = gutter_w.saturating_add(it.cols.1).min(content.width);
            if x0 < x1 {
                hits.push(
                    Rect {
                        x: content.x.saturating_add(x0),
                        y,
                        width: x1 - x0,
                        height: 1,
                    },
                    Hit::Block(id),
                );
            }
        }

        let focused_block_cols = focused_item.and_then(|it| {
            if it.kind == FocusTarget::BlockAction && it.line == Some(line_no) {
                Some(it.cols)
            } else {
                None
            }
        });

        // Marker column: ▌ on the cursor row.
        let mut spans: Vec<Span<'static>> = Vec::new();
        if on_cursor {
            spans.push(Span::styled("▌", cursor_bg));
        } else {
            spans.push(Span::raw(" "));
        }

        if let Some(numbers) = gutter {
            // Soft-wrapped continuation rows carry no number.
            let label = match numbers.get(src_idx).copied().flatten() {
                Some(n) => format!("{n:4}│ "),
                None => "    │ ".to_owned(),
            };
            spans.push(Span::styled(label, gutter_style));
        }

        if let Some(hl_lines) = highlights
            && let Some(hl) = hl_lines.get(src_idx)
        {
            let mut col = 0usize;
            for run in hl {
                if col >= text_width {
                    break;
                }
                let remain = text_width.saturating_sub(col);
                let clipped: String = run.text.chars().take(remain).collect();
                if clipped.is_empty() && run.text.is_empty() {
                    continue;
                }
                let mut st = run.style;
                if on_cursor {
                    st = st.bg(theme.cursor_line);
                }
                col += clipped.chars().count();
                spans.push(Span::styled(clipped, st));
            }
            if spans.len() == 1 + usize::from(show_gutter) {
                spans.push(Span::styled(String::new(), base));
            }
        } else if let Some(styled_lines) = styled
            && let Some(sl) = styled_lines.get(src_idx)
        {
            spans.extend(paint_styled_line(
                sl,
                line_no,
                text_width,
                link_spans,
                focused_link,
                focused_block_cols,
                base,
                focus_style,
                theme,
                on_cursor,
            ));
        } else {
            let raw = lines.get(src_idx).map_or("", String::as_str);
            let display: String = raw.chars().take(text_width).collect();
            spans.extend(styled_line(
                &display,
                line_no,
                link_spans,
                focused_link,
                focused_block_cols,
                base,
                focus_style,
                theme,
            ));
        }
        if on_cursor {
            // Pad highlight across the full inner width (marker + content).
            let used: usize = spans
                .iter()
                .map(|s| Span::raw(s.content.as_ref()).width())
                .sum();
            let pad = usize::from(inner_area.width).saturating_sub(used);
            if pad > 0 {
                spans.push(Span::styled(
                    " ".repeat(pad),
                    Style::default().bg(theme.cursor_line),
                ));
            }
        }
        let row_text = lines.get(src_idx).map_or("", String::as_str);
        // The searched phrase: the active tab / footer link colours.
        if let Some((l, c0, c1)) = match_span
            && l == line_no
        {
            spans = patch_cols(
                spans,
                origin + usize::from(c0),
                (origin + usize::from(c1)).min(origin + text_width),
                Style::default().bg(theme.peach).fg(theme.on_peach),
            );
        }
        if let Some(sel) = selection
            && let Some((c0, c1)) = sel.cols_on(line_no, text_col::line_width(row_text))
        {
            spans = patch_cols(
                spans,
                origin + usize::from(c0),
                (origin + usize::from(c1)).min(origin + text_width),
                Style::default().bg(theme.selection),
            );
        }
        if on_cursor && focused {
            let col = usize::from(text_col::clamp_col(row_text, cursor_col));
            let w = row_text
                .chars()
                .scan(0usize, |x, ch| {
                    let start = *x;
                    *x += usize::from(text_col::char_width(ch));
                    Some((start, usize::from(text_col::char_width(ch))))
                })
                .find(|&(start, _)| start == col)
                .map_or(1, |(_, w)| w);
            spans = patch_cols(
                spans,
                origin + col,
                origin + col + w,
                // Default text colour as the block, the glyph inverted: reads on a
                // peach phrase and on the cursor line alike.
                Style::default()
                    .fg(Color::Reset)
                    .bg(Color::Reset)
                    .add_modifier(Modifier::REVERSED),
            );
        }
        out_lines.push(Line::from(spans));
    }

    // Paint into full inner (marker column + content).
    frame.render_widget(Paragraph::new(out_lines), inner_area);
    geom
}

#[allow(clippy::too_many_arguments)]
fn paint_styled_line(
    sl: &StyledLine,
    line_no: u32,
    text_width: usize,
    link_spans: &[LinkSpan],
    focused_link: Option<LinkId>,
    focused_block_cols: Option<(u16, u16)>,
    base: Style,
    focus_style: Style,
    theme: &Theme,
    on_cursor: bool,
) -> Vec<Span<'static>> {
    let mut ranges: Vec<(u16, u16, Option<LinkClass>, bool)> = Vec::new();
    for span in link_spans {
        for &(seg_line, cols) in &span.segments {
            if seg_line == line_no && cols.0 < cols.1 {
                let is_focus = focused_link == Some(span.id);
                ranges.push((cols.0, cols.1, Some(span.class), is_focus));
            }
        }
    }
    if let Some((c0, c1)) = focused_block_cols
        && c0 < c1
    {
        ranges.push((c0, c1, None, true));
    }
    ranges.sort_by_key(|r| r.0);

    let style_at = |col: u16, kind: StyleKind| -> Style {
        let mut st = theme.style_kind(kind);
        let flat = !matches!(
            kind,
            StyleKind::CodeBlock | StyleKind::CodeLang | StyleKind::InlineCode | StyleKind::Quote
        );
        if on_cursor {
            st = st.bg(theme.cursor_line);
            // Same text colour on and off the cursor line (the frontmatter label
            // used to read gray until the cursor left it).
            if flat {
                st = st.fg(theme.text);
            }
        } else if flat {
            st = st.patch(base);
        }
        for &(c0, c1, class, is_focus) in &ranges {
            if col >= c0 && col < c1 {
                st = if is_focus {
                    focus_style
                } else if let Some(class) = class {
                    theme.link_class(class)
                } else {
                    st
                };
                if on_cursor {
                    st = st.bg(theme.cursor_line);
                }
                break;
            }
        }
        st
    };

    let mut spans = Vec::new();
    let mut col = 0u16;
    let mut buf = String::new();
    let mut style = base;
    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), style));
        }
    };

    for run in &sl.spans {
        if usize::from(col) >= text_width {
            break;
        }
        for ch in run.text.chars() {
            if usize::from(col) >= text_width {
                break;
            }
            let w = u16::try_from(Span::raw(ch.to_string()).width())
                .unwrap_or(1)
                .max(1);
            let st = style_at(col, run.kind);
            if buf.is_empty() {
                style = st;
            } else if st != style {
                flush(&mut buf, style, &mut spans);
                style = st;
            }
            buf.push(ch);
            col = col.saturating_add(w);
        }
    }
    flush(&mut buf, style, &mut spans);

    // Full-row code/quote shading (glyphs alone leave a ragged right edge).
    if !on_cursor {
        let shade = sl.spans.iter().find_map(|s| match s.kind {
            StyleKind::CodeBlock | StyleKind::CodeLang | StyleKind::InlineCode => {
                Some(theme.code_bg)
            }
            StyleKind::Quote => Some(theme.quote_bar),
            _ => None,
        });
        if let Some(bg) = shade {
            let pad = text_width.saturating_sub(usize::from(col));
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
            }
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

#[allow(clippy::too_many_arguments)] // link + block focus overlays on one pass
fn styled_line(
    display: &str,
    line_no: u32,
    link_spans: &[LinkSpan],
    focused_link: Option<LinkId>,
    focused_block_cols: Option<(u16, u16)>,
    base: Style,
    focus_style: Style,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut ranges: Vec<(u16, u16, Option<LinkClass>, bool)> = Vec::new();
    for span in link_spans {
        for &(seg_line, cols) in &span.segments {
            if seg_line == line_no && cols.0 < cols.1 {
                let is_focus = focused_link == Some(span.id);
                ranges.push((cols.0, cols.1, Some(span.class), is_focus));
            }
        }
    }
    if let Some((c0, c1)) = focused_block_cols
        && c0 < c1
    {
        ranges.push((c0, c1, None, true));
    }
    ranges.sort_by_key(|r| r.0);

    if ranges.is_empty() {
        return vec![Span::styled(display.to_owned(), base)];
    }

    let mut spans = Vec::new();
    let mut col = 0u16;
    let mut buf = String::new();
    let mut style = base;

    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(buf), style));
        }
    };

    for ch in display.chars() {
        let w = u16::try_from(Span::raw(ch.to_string()).width())
            .unwrap_or(1)
            .max(1);
        let mut next_style = base;
        for &(c0, c1, class, is_focus) in &ranges {
            if col >= c0 && col < c1 {
                next_style = if is_focus {
                    focus_style
                } else if let Some(class) = class {
                    theme.link_class(class)
                } else {
                    base
                };
                break;
            }
        }
        if next_style != style {
            flush(&mut buf, style, &mut spans);
            style = next_style;
        }
        buf.push(ch);
        col = col.saturating_add(w);
    }
    flush(&mut buf, style, &mut spans);
    spans
}
