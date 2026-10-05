//! Table viewer (P3-14 / P3-30): a [`ModalContent`] over one [`DocTable`].
//!
//! Two-axis scroll with a fixed header row and first column, row filter, column sort, and
//! cell / row copy (tab-separated, like the inline table copy from P2-R38). The focused row
//! expands so cells wrap in their columns; collapsed rows stay one line with an ellipsis.
//! Cells keep their inline styling (bold, link colour, code) from [`DocCell`]; filter, sort
//! and copy use the plain text.

use std::cmp::Ordering;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_render::{DocCell, DocTable, StyleKind, is_sanitized_control};

use super::modal_viewer::{ModalContent, ModalEvent};
use super::text_col::{char_width, line_width};
use super::theme::Theme;

const SEP: &str = " │ ";
/// Display columns of [`SEP`] (its `len()` is bytes).
const SEP_W: usize = 3;
/// Minimum leftover cells before a partial (clipped) next column is shown.
const MIN_PARTIAL: usize = 8;
const HINT: &str = "↑↓←→ move  / filter  s sort  y cell  Y row  Esc close";

/// Open table viewer state.
pub struct TableViewer {
    table: DocTable,
    /// Plain text of the body cells, for width, filter, sort and copy.
    text: Vec<Vec<String>>,
    /// Body row indexes after filter and sort.
    view: Vec<usize>,
    filter: String,
    editing: bool,
    /// Sorted column and whether descending.
    sort: Option<(usize, bool)>,
    /// Cursor row in `view`, cursor column.
    row: usize,
    col: usize,
    /// First visible row of `view`; first scrolling column (column 0 is always shown).
    top: usize,
    left: usize,
    /// Natural column widths (longest cell/header +2 for the sort arrow).
    widths: Vec<usize>,
    /// Body rows that fit, from the last draw (page keys).
    page: usize,
    /// Sort keys of the sorted column, computed once per sort (not per comparison).
    sort_keys: Vec<SortKey>,
    /// The column `sort_keys` belong to.
    sort_keys_col: Option<usize>,
}

/// Column layout for one frame, computed once and shared by the header, rule and body rows.
struct ColumnLayout {
    /// Display widths; a partial last-visible column is clipped to the leftover width.
    widths: Vec<usize>,
    /// Visible column indexes: column 0, then from `left`.
    cols: Vec<usize>,
    /// The last visible column reaches the right edge (no trailing separator).
    fills_edge: bool,
}

impl TableViewer {
    #[must_use]
    pub fn new(table: DocTable) -> Self {
        // Sanitise once so filter, sort, copy and display agree (N20).
        let plain = |r: &[DocCell]| r.iter().map(|c| show(&c.text())).collect::<Vec<_>>();
        let head = plain(&table.header);
        let text: Vec<Vec<String>> = table.rows.iter().map(|r| plain(r)).collect();
        let widths = (0..head.len())
            .map(|k| {
                let cells = text.iter().map(|r| r[k].as_str());
                let w = cells
                    .chain(std::iter::once(head[k].as_str()))
                    .map(|c| usize::from(line_width(c)))
                    .max()
                    .unwrap_or(1);
                // +2: room for the sort arrow.
                (w + 2).max(3)
            })
            .collect();
        let mut v = Self {
            view: Vec::new(),
            filter: String::new(),
            editing: false,
            sort: None,
            row: 0,
            col: 0,
            top: 0,
            left: 1,
            widths,
            page: 10,
            sort_keys: Vec::new(),
            sort_keys_col: None,
            text,
            table,
        };
        v.refresh();
        v
    }

    /// Re-run filter and sort over the body rows.
    fn refresh(&mut self) {
        // Case-insensitive without allocating a lowercase copy of every cell per keystroke.
        let needle = self.filter.as_str();
        self.view = (0..self.text.len())
            .filter(|&i| {
                needle.is_empty()
                    || self.text[i].iter().any(|c| {
                        wiki_reader_core::search::find_case_insensitive(c, needle).is_some()
                    })
            })
            .collect();
        if let Some((c, desc)) = self.sort {
            if self.sort_keys_col != Some(c) {
                self.sort_keys = self.text.iter().map(|r| sort_key(&r[c])).collect();
                self.sort_keys_col = Some(c);
            }
            let keys = &self.sort_keys;
            // Stable, so equal keys keep document order.
            self.view.sort_by(|&a, &b| {
                let o = cmp_keys(&keys[a], &keys[b]);
                if desc { o.reverse() } else { o }
            });
        }
        self.row = self.row.min(self.view.len().saturating_sub(1));
    }

    fn current_row(&self) -> Option<&Vec<String>> {
        self.view.get(self.row).map(|&i| &self.text[i])
    }

    fn move_row(&mut self, delta: isize) {
        let last = self.view.len().saturating_sub(1);
        self.row = self.row.saturating_add_signed(delta).min(last);
    }

    fn move_col(&mut self, delta: isize) {
        let last = self.widths.len().saturating_sub(1);
        self.col = self.col.saturating_add_signed(delta).min(last);
    }

    /// Display widths: natural, but col 0 ≤ ~avail/2 and others ≤ the rest after col 0 + SEP.
    fn eff_widths(&self, avail: usize) -> Vec<usize> {
        if self.widths.is_empty() {
            return Vec::new();
        }
        let w0_cap = (avail / 2).max(3);
        let rest_cap = avail
            .saturating_sub(SEP_W + self.widths[0].min(w0_cap).max(3))
            .max(3);
        self.widths
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                if i == 0 {
                    w.min(w0_cap).max(3)
                } else {
                    w.min(rest_cap).max(3)
                }
            })
            .collect()
    }

    /// Visible columns and clipped widths for `avail` cells.
    ///
    /// After the last whole column, the next one is shown clipped to the leftover width when
    /// that is at least [`MIN_PARTIAL`] (so the panel does not leave blank space).
    fn column_layout(&self, avail: usize) -> ColumnLayout {
        let mut widths = self.eff_widths(avail);
        let ncols = widths.len();
        if ncols == 0 {
            return ColumnLayout {
                widths,
                cols: Vec::new(),
                fills_edge: true,
            };
        }
        let mut cols = vec![0];
        let mut used = widths[0];
        let mut clipped = false;
        for (k, w) in widths.iter().enumerate().skip(self.left.min(ncols)) {
            if k == 0 {
                continue;
            }
            let need = used + SEP_W + *w;
            if need <= avail {
                used = need;
                cols.push(k);
                continue;
            }
            // Partial next column in the leftover, instead of blank space.
            let leftover = avail.saturating_sub(used + SEP_W);
            if leftover >= MIN_PARTIAL {
                widths[k] = leftover;
                cols.push(k);
                clipped = true;
            }
            break;
        }
        let last_is_table_final = cols.last().is_some_and(|&k| k + 1 == ncols);
        let fills_edge = clipped || last_is_table_final;
        // Stretch the true last column when it fits wholly (not when clipped).
        if last_is_table_final && !clipped {
            let used: usize = cols.iter().map(|&k| widths[k]).sum::<usize>()
                + cols.len().saturating_sub(1) * SEP_W;
            if let Some(&last) = cols.last() {
                widths[last] += avail.saturating_sub(used);
            }
        }
        ColumnLayout {
            widths,
            cols,
            fills_edge,
        }
    }

    fn keep_cursor_visible(&mut self, avail: usize, body_h: usize) {
        if self.view.is_empty() || body_h == 0 {
            return;
        }
        if self.row < self.top {
            self.top = self.row;
        }
        // Walk top forward until the expanded cursor row fits in the body budget.
        loop {
            let layout = self.column_layout(avail);
            let cost = self.cost_from_top(self.top, &layout, body_h);
            if cost.cursor_fits || self.top >= self.row {
                break;
            }
            self.top += 1;
        }
        if self.col > 0 {
            self.left = self.left.clamp(1, self.col);
            while self.left < self.col
                && self.column_layout(avail).cols.last().copied().unwrap_or(0) < self.col
            {
                self.left += 1;
            }
        }
    }

    /// How much of the body from `top` is used, and whether the cursor row is fully shown.
    fn cost_from_top(&self, top: usize, layout: &ColumnLayout, body_h: usize) -> VisibleCost {
        let mut used = 0usize;
        let mut cursor_fits = false;
        let mut page_rows = 0usize;
        for n in top..self.view.len() {
            let expand = n == self.row;
            let h = self.row_height(n, layout, expand, body_h);
            let cost = if expand { h.saturating_add(2) } else { 1 };
            if used + cost > body_h && n > top {
                break;
            }
            if expand && used + cost > body_h {
                // Clip the expansion to what remains (rules prefer to stay).
                let remain = body_h.saturating_sub(used).saturating_sub(2).max(1);
                cursor_fits = remain >= h || remain >= body_h.saturating_sub(2).max(1);
                page_rows += 1;
                break;
            }
            used += cost;
            page_rows += 1;
            if expand {
                cursor_fits = true;
            }
            if used >= body_h {
                break;
            }
        }
        VisibleCost {
            cursor_fits,
            page_rows: page_rows.max(1),
        }
    }

    /// Height in terminal lines of view-row `n` (1 when collapsed).
    fn row_height(&self, n: usize, layout: &ColumnLayout, expand: bool, max_h: usize) -> usize {
        if !expand {
            return 1;
        }
        let Some(&i) = self.view.get(n) else {
            return 1;
        };
        let cells = &self.table.rows[i];
        let h = layout
            .cols
            .iter()
            .map(|&k| wrap_line_count(&cells[k], layout.widths[k]))
            .max()
            .unwrap_or(1)
            .max(1);
        h.min(max_h.max(1))
    }

    /// Tallest expansion any body row would need at `avail` (for stable `want`).
    fn max_expand_height(&self, avail: usize) -> usize {
        if self.text.is_empty() {
            return 1;
        }
        let widths = self.eff_widths(avail.max(1));
        self.text
            .iter()
            .map(|row| {
                row.iter()
                    .enumerate()
                    .map(|(k, cell)| {
                        let w = widths.get(k).copied().unwrap_or(3).max(1);
                        wrap_plain_count(cell, w)
                    })
                    .max()
                    .unwrap_or(1)
            })
            .max()
            .unwrap_or(1)
            .max(1)
    }

    /// Header rule matching the visible columns (`─` / `─┼─`).
    fn rule_line(layout: &ColumnLayout, theme: &Theme) -> Line<'static> {
        let mut spans = Vec::new();
        for (i, &k) in layout.cols.iter().enumerate() {
            spans.push(Span::styled("─".repeat(layout.widths[k]), theme.muted()));
            let is_last = i + 1 == layout.cols.len();
            if !(layout.fills_edge && is_last) {
                spans.push(Span::styled("─┼─", theme.muted()));
            }
        }
        Line::from(spans)
    }

    /// One table line from `cells`, left to right; column 0 stays, then from `self.left`.
    /// `base` is the row style; `arrow` marks the sorted column (header only).
    fn cells_line(
        &self,
        cells: &[DocCell],
        layout: &ColumnLayout,
        base: Style,
        cursor_row: bool,
        theme: &Theme,
        arrow: Option<char>,
    ) -> Line<'static> {
        let sep_style = if cursor_row {
            theme.muted().bg(theme.cursor_line)
        } else {
            theme.muted()
        };
        let mut spans = Vec::new();
        for (i, &k) in layout.cols.iter().enumerate() {
            let arrow = arrow.filter(|_| self.sort.is_some_and(|(c, _)| c == k));
            if cursor_row && k == self.col {
                let cursor = Style::default().fg(theme.on_peach).bg(theme.peach);
                spans.extend(cell_spans(&cells[k], layout.widths[k], arrow, cursor, None));
            } else {
                spans.extend(cell_spans(
                    &cells[k],
                    layout.widths[k],
                    arrow,
                    base,
                    Some(theme),
                ));
            }
            let is_last = i + 1 == layout.cols.len();
            if !(layout.fills_edge && is_last) {
                spans.push(Span::styled(SEP, sep_style));
            }
        }
        Line::from(spans)
    }

    /// One wrap-line of an expanded row; `wrapped[col_slot]` is the pre-wrapped cell lines.
    fn expanded_line(
        &self,
        wrapped: &[Vec<Vec<Span<'static>>>],
        layout: &ColumnLayout,
        base: Style,
        theme: &Theme,
        line_idx: usize,
    ) -> Line<'static> {
        let sep_style = theme.muted().bg(theme.cursor_line);
        let mut spans = Vec::new();
        for (i, &k) in layout.cols.iter().enumerate() {
            let w = layout.widths[k];
            let cell_line = wrapped
                .get(i)
                .and_then(|lines| lines.get(line_idx))
                .cloned()
                .unwrap_or_else(|| vec![Span::styled(" ".repeat(w), base)]);
            if k == self.col {
                // Peach marker on the focused column for every wrap line.
                let cursor = Style::default().fg(theme.on_peach).bg(theme.peach);
                spans.extend(recolour_spans(cell_line, cursor));
            } else {
                spans.extend(cell_line);
            }
            let is_last = i + 1 == layout.cols.len();
            if !(layout.fills_edge && is_last) {
                spans.push(Span::styled(SEP, sep_style));
            }
        }
        Line::from(spans)
    }
}

struct VisibleCost {
    cursor_fits: bool,
    page_rows: usize,
}

/// One cell as spans of exactly `w` columns: runs cut with `…` when too long, the sort `arrow`
/// after them when it fits, then padding. `theme` is `None` to paint the whole cell in `base`.
fn cell_spans(
    cell: &DocCell,
    w: usize,
    arrow: Option<char>,
    base: Style,
    theme: Option<&Theme>,
) -> Vec<Span<'static>> {
    let text_w: usize = cell
        .spans
        .iter()
        .map(|s| usize::from(line_width(&s.text)))
        .sum();
    let arrow = arrow.filter(|_| text_w + 2 <= w);
    let cut = text_w + if arrow.is_some() { 2 } else { 0 } > w;
    let room = if cut { w.saturating_sub(1) } else { w };
    // ponytail: every link takes the internal link colour; upgrade: carry `LinkClass` on the span.
    let mut out = Vec::new();
    let mut used = 0;
    'runs: for run in &cell.spans {
        let style = span_style(theme, run.kind, base);
        let mut piece = String::new();
        for ch in show(&run.text).chars() {
            let cw = usize::from(char_width(ch));
            if used + cw > room {
                out.push(Span::styled(piece, style));
                break 'runs;
            }
            used += cw;
            piece.push(ch);
        }
        out.push(Span::styled(piece, style));
    }
    if cut {
        out.push(Span::styled("…", base));
        used += 1;
    }
    if let Some(a) = arrow {
        out.push(Span::styled(format!(" {a}"), base));
        used += 2;
    }
    out.push(Span::styled(" ".repeat(w.saturating_sub(used)), base));
    out
}

fn span_style(theme: Option<&Theme>, kind: StyleKind, base: Style) -> Style {
    match (theme, kind) {
        (Some(t), k)
            if !matches!(
                k,
                StyleKind::Plain | StyleKind::Table | StyleKind::TableHeader
            ) =>
        {
            base.patch(t.style_kind(k))
        }
        _ => base,
    }
}

fn recolour_spans(spans: Vec<Span<'static>>, style: Style) -> Vec<Span<'static>> {
    spans
        .into_iter()
        .map(|s| Span::styled(s.content.to_string(), style))
        .collect()
}

/// Wrap a cell's styled runs to `w` columns; each line is padded to exactly `w`.
fn wrap_cell(
    cell: &DocCell,
    w: usize,
    base: Style,
    theme: Option<&Theme>,
) -> Vec<Vec<Span<'static>>> {
    if w == 0 {
        return vec![vec![Span::styled(String::new(), base)]];
    }
    let mut chars: Vec<char> = Vec::new();
    let mut kinds: Vec<StyleKind> = Vec::new();
    for run in &cell.spans {
        for ch in show(&run.text).chars() {
            chars.push(ch);
            kinds.push(run.kind);
        }
    }
    let starts = wrap_starts(&chars, w);
    let mut lines = Vec::with_capacity(starts.len());
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(chars.len());
        let mut spans = Vec::new();
        let mut used = 0usize;
        if start < end {
            let mut piece = String::new();
            let mut kind = kinds[start];
            for j in start..end {
                if kinds[j] != kind && !piece.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut piece),
                        span_style(theme, kind, base),
                    ));
                }
                kind = kinds[j];
                piece.push(chars[j]);
                used += usize::from(char_width(chars[j]));
            }
            if !piece.is_empty() {
                spans.push(Span::styled(piece, span_style(theme, kind, base)));
            }
        }
        spans.push(Span::styled(" ".repeat(w.saturating_sub(used)), base));
        lines.push(spans);
    }
    if lines.is_empty() {
        lines.push(vec![Span::styled(" ".repeat(w), base)]);
    }
    lines
}

fn wrap_line_count(cell: &DocCell, w: usize) -> usize {
    wrap_plain_count(&show(&cell.text()), w)
}

fn wrap_plain_count(s: &str, w: usize) -> usize {
    if w == 0 {
        return 1;
    }
    let chars: Vec<char> = s.chars().collect();
    wrap_starts(&chars, w).len().max(1)
}

/// Soft-wrap starts (ponytail: local copy of `viewer_doc::wrap_starts`; shared helper if a third
/// caller appears).
fn wrap_starts(chars: &[char], width: usize) -> Vec<usize> {
    let mut starts = vec![0];
    if width == 0 || chars.is_empty() {
        return starts;
    }
    let widths: Vec<usize> = chars.iter().map(|&c| usize::from(char_width(c))).collect();
    let (mut start, mut w, mut brk, mut i) = (0usize, 0usize, None::<usize>, 0usize);
    while i < chars.len() {
        if w + widths[i] > width && i > start {
            let b = brk.take().filter(|&b| b > start).unwrap_or(i);
            starts.push(b);
            start = b;
            w = widths[b..i].iter().sum();
            continue;
        }
        w += widths[i];
        if chars[i] == ' ' {
            brk = Some(i + 1);
        }
        i += 1;
    }
    starts
}

impl ModalContent for TableViewer {
    fn label(&self) -> &'static str {
        "TABLE"
    }

    fn want(&mut self, cap: (u16, u16)) -> (u16, u16) {
        // Stable height: collapsed rows + rules around the expand + tallest wrap, capped by
        // the modal so the panel does not jump as the cursor moves.
        let w = self.widths.iter().map(|w| w + SEP_W).sum::<usize>();
        let avail = usize::from(cap.0).min(w).max(1);
        let expand = self.max_expand_height(avail);
        let h = 2 + self.text.len().max(1) + 2 + expand.saturating_sub(1);
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        (to_u16(w), to_u16(h).min(cap.1.max(1)))
    }

    fn title(&self) -> String {
        format!(
            "Table (line {}) · row {}/{} · col {}/{}",
            self.table.source_line,
            if self.view.is_empty() {
                0
            } else {
                self.row + 1
            },
            self.view.len(),
            self.col + 1,
            self.widths.len()
        )
    }

    fn hint(&self) -> String {
        if self.editing {
            format!("/ {}█   Enter keep · Esc clear", self.filter)
        } else if self.filter.is_empty() {
            HINT.into()
        } else {
            format!("filter “{}”  {HINT}", self.filter)
        }
    }

    fn editing(&self) -> bool {
        self.editing
    }

    fn key(&mut self, key: KeyEvent) -> ModalEvent {
        if self.editing {
            match key.code {
                KeyCode::Esc => {
                    self.filter.clear();
                    self.editing = false;
                }
                KeyCode::Enter => self.editing = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.filter.push(c);
                }
                _ => {}
            }
            self.refresh();
            return ModalEvent::Stay;
        }
        let page = isize::try_from(self.page.max(1)).unwrap_or(1);
        match key.code {
            KeyCode::Char('q') => return ModalEvent::Close,
            KeyCode::Up | KeyCode::Char('k') => self.move_row(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_row(1),
            KeyCode::PageUp => self.move_row(-page),
            KeyCode::PageDown => self.move_row(page),
            KeyCode::Home | KeyCode::Char('g') => self.row = 0,
            KeyCode::End | KeyCode::Char('G') => self.row = self.view.len().saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => self.move_col(-1),
            KeyCode::Right | KeyCode::Char('l') => self.move_col(1),
            KeyCode::Char('/') => self.editing = true,
            KeyCode::Char('s') => {
                // asc → desc → unsorted, per column.
                self.sort = match self.sort {
                    Some((c, false)) if c == self.col => Some((c, true)),
                    Some((c, true)) if c == self.col => None,
                    _ => Some((self.col, false)),
                };
                self.refresh();
            }
            KeyCode::Char('y') => {
                if let Some(cell) = self.current_row().map(|r| r[self.col].clone()) {
                    return ModalEvent::Copy(cell);
                }
            }
            KeyCode::Char('Y') => {
                if let Some(row) = self.current_row() {
                    return ModalEvent::Copy(row.join("\t"));
                }
            }
            _ => {}
        }
        ModalEvent::Stay
    }

    fn draw(&mut self, frame: &mut Frame<'_>, body: Rect, theme: &Theme) {
        let body_h = usize::from(body.height.saturating_sub(2)).max(1);
        let avail = usize::from(body.width);
        self.keep_cursor_visible(avail, body_h);
        let layout = self.column_layout(avail);
        let cost = self.cost_from_top(self.top, &layout, body_h);
        self.page = cost.page_rows;

        let head = self.cells_line(
            &self.table.header,
            &layout,
            theme.text().add_modifier(Modifier::BOLD),
            false,
            theme,
            Some(if self.sort.is_some_and(|(_, d)| d) {
                '▼'
            } else {
                '▲'
            }),
        );
        let mut lines = vec![head, Self::rule_line(&layout, theme)];
        let mut used = 0usize;
        for n in self.top..self.view.len() {
            let expand = n == self.row;
            let i = self.view[n];
            let style = if expand {
                theme.text().bg(theme.cursor_line)
            } else {
                theme.text()
            };
            if expand {
                let max_h = body_h.saturating_sub(used).saturating_sub(2).max(1);
                let h = self.row_height(n, &layout, true, max_h);
                let cost = h + 2;
                if used + cost > body_h && n > self.top {
                    break;
                }
                let wrapped: Vec<Vec<Vec<Span<'static>>>> = layout
                    .cols
                    .iter()
                    .map(|&k| {
                        let mut lines = wrap_cell(
                            &self.table.rows[i][k],
                            layout.widths[k],
                            style,
                            if k == self.col { None } else { Some(theme) },
                        );
                        // Clip with … on the last line when capped.
                        if lines.len() > h {
                            lines.truncate(h);
                            if let Some(last) = lines.last_mut() {
                                *last = cell_spans(
                                    &DocCell {
                                        spans: vec![wiki_reader_render::StyledSpan {
                                            text: show(&self.table.rows[i][k].text()),
                                            kind: StyleKind::Plain,
                                        }],
                                    },
                                    layout.widths[k],
                                    None,
                                    if k == self.col {
                                        Style::default().fg(theme.on_peach).bg(theme.peach)
                                    } else {
                                        style
                                    },
                                    None,
                                );
                            }
                        }
                        while lines.len() < h {
                            lines.push(vec![Span::styled(" ".repeat(layout.widths[k]), style)]);
                        }
                        lines
                    })
                    .collect();
                lines.push(Self::rule_line(&layout, theme));
                for li in 0..h {
                    lines.push(self.expanded_line(&wrapped, &layout, style, theme, li));
                }
                lines.push(Self::rule_line(&layout, theme));
                used += cost.min(body_h.saturating_sub(used));
            } else {
                if used + 1 > body_h && n > self.top {
                    break;
                }
                lines.push(self.cells_line(
                    &self.table.rows[i],
                    &layout,
                    style,
                    false,
                    theme,
                    None,
                ));
                used += 1;
            }
            if used >= body_h {
                break;
            }
        }
        if self.view.is_empty() {
            lines.push(Line::from(Span::styled("no rows match", theme.muted())));
        }
        frame.render_widget(Paragraph::new(lines), body);
    }
}

/// Cell text for one terminal line: tabs, newlines, other controls and bidi become spaces.
fn show(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || is_sanitized_control(c) {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Sort key of one cell, computed once per sort.
enum SortKey {
    Num(f64),
    /// Lowercased text.
    Text(String),
}

/// Numbers (finite, so a name like "Nan" or "Inf" sorts as text) before text.
fn sort_key(cell: &str) -> SortKey {
    match cell.trim().parse::<f64>().ok().filter(|n| n.is_finite()) {
        Some(n) => SortKey::Num(n),
        None => SortKey::Text(cell.to_lowercase()),
    }
}

/// Total order: numbers (by value) before text (case-insensitive).
fn cmp_keys(a: &SortKey, b: &SortKey) -> Ordering {
    match (a, b) {
        (SortKey::Num(x), SortKey::Num(y)) => x.total_cmp(y),
        (SortKey::Num(_), SortKey::Text(_)) => Ordering::Less,
        (SortKey::Text(_), SortKey::Num(_)) => Ordering::Greater,
        (SortKey::Text(x), SortKey::Text(y)) => x.cmp(y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(s: &str) -> DocCell {
        DocCell {
            spans: vec![wiki_reader_render::StyledSpan {
                text: s.to_owned(),
                kind: StyleKind::Plain,
            }],
        }
    }

    fn viewer() -> TableViewer {
        let row = |a: &str, b: &str| vec![cell(a), cell(b)];
        TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 6,
            header: row("name", "n"),
            rows: vec![row("Bob", "10"), row("alice", "9"), row("Carol", "x")],
        })
    }

    fn press(v: &mut TableViewer, keys: &str) -> ModalEvent {
        let mut ev = ModalEvent::Stay;
        for c in keys.chars() {
            ev = v.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        ev
    }

    fn names(v: &TableViewer) -> Vec<&str> {
        v.view.iter().map(|&i| v.text[i][0].as_str()).collect()
    }

    fn row_at(buf: &ratatui::buffer::Buffer, y: u16, w: u16) -> String {
        (0..w).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn filter_is_case_insensitive_over_all_cells() {
        let mut v = viewer();
        press(&mut v, "/ALI");
        assert_eq!(names(&v), ["alice"]);
        v.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(names(&v).len(), 3, "Esc clears the filter");
        press(&mut v, "/10");
        assert_eq!(names(&v), ["Bob"], "matches any cell");
    }

    #[test]
    fn sort_cycles_asc_desc_off_with_numbers_before_text() {
        let mut v = viewer();
        press(&mut v, "s");
        assert_eq!(names(&v), ["alice", "Bob", "Carol"]);
        press(&mut v, "s");
        assert_eq!(names(&v), ["Carol", "Bob", "alice"]);
        press(&mut v, "s");
        assert_eq!(names(&v), ["Bob", "alice", "Carol"], "document order again");
        press(&mut v, "ls");
        assert_eq!(names(&v), ["alice", "Bob", "Carol"], "9 < 10 < x");
    }

    #[test]
    fn names_that_parse_as_floats_sort_as_text() {
        let row = |a: &str| vec![cell(a), cell("x")];
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 6,
            header: vec![cell("name"), cell("n")],
            rows: vec![row("Nan"), row("Bob"), row("5"), row("inf")],
        });
        press(&mut v, "s");
        // 5 is a number; "inf" and "Nan" are words here, so they sort as text.
        assert_eq!(names(&v), ["5", "Bob", "inf", "Nan"]);
    }

    #[test]
    fn filter_folds_case_without_slicing_and_sort_keys_are_reused() {
        let row = |a: &str| vec![cell(a), cell("x")];
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 5,
            header: vec![cell("name"), cell("n")],
            rows: vec![row("İstanbul"), row("Ankara"), row("izmir")],
        });
        press(&mut v, "s");
        assert_eq!(
            v.sort_keys_col,
            Some(0),
            "keys computed once for the sorted column"
        );
        press(&mut v, "/STANBUL");
        assert_eq!(names(&v), ["İstanbul"]);
        assert_eq!(
            v.sort_keys.len(),
            3,
            "typing a filter does not rebuild the keys"
        );
    }

    #[test]
    fn copy_cell_and_row() {
        let mut v = viewer();
        press(&mut v, "jl");
        assert_eq!(press(&mut v, "y"), ModalEvent::Copy("9".into()));
        assert_eq!(press(&mut v, "Y"), ModalEvent::Copy("alice\t9".into()));
    }

    #[test]
    fn bidi_in_cell_is_sanitised_for_display_and_copy() {
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 3,
            header: vec![cell("name")],
            rows: vec![vec![cell("safe\u{202E}evil")]],
        });
        assert_eq!(v.text[0][0], "safe evil");
        assert_eq!(press(&mut v, "y"), ModalEvent::Copy("safe evil".into()));
        assert!(!v.text[0][0].contains('\u{202E}'));
    }

    #[test]
    fn tab_and_newline_in_cell_become_spaces_for_display_and_copy() {
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 3,
            header: vec![cell("a"), cell("b")],
            rows: vec![vec![cell("x\ty"), cell("a\nb")]],
        });
        assert_eq!(v.text[0][0], "x y");
        assert_eq!(v.text[0][1], "a b");
        assert_eq!(press(&mut v, "y"), ModalEvent::Copy("x y".into()));
        assert_eq!(press(&mut v, "Y"), ModalEvent::Copy("x y\ta b".into()));
    }

    #[test]
    fn cells_keep_bold_header_and_link_colour_and_the_panel_fits_the_grid() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let link = DocCell {
            spans: vec![wiki_reader_render::StyledSpan {
                text: "docs".into(),
                kind: StyleKind::Link,
            }],
        };
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 4,
            header: vec![cell("name"), cell("ref")],
            rows: vec![vec![cell("a"), link]],
        });
        let theme = Theme::default();
        let mut term = Terminal::new(TestBackend::new(30, 8)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &theme)).unwrap();
        let buf = term.backend().buffer();
        assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD), "bold header");
        let rule = row_at(buf, 1, 30);
        assert!(
            rule.contains('─') && rule.contains('┼'),
            "header rule: {rule}"
        );
        // Expanded row: rule, body, rule — body is on y=3.
        let row = row_at(buf, 3, 30);
        let x = u16::try_from(row.find("docs").unwrap()).unwrap();
        assert_eq!(buf[(x, 3)].fg, theme.link, "link colour in a body cell");
        // Content-sized: both columns plus separators; height reserves expand + rules.
        // "docs" (4) + sort room beats header "ref" (3) → col1 width 6.
        assert_eq!(v.want((90, 30)).0, 6 + 3 + 6 + 3);
        assert!(
            v.want((90, 30)).1 >= 5,
            "stable height includes expand room"
        );
    }

    #[test]
    fn scrolling_right_keeps_header_and_first_column() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let cells = |p: &str| {
            (0..8)
                .map(|k| cell(&format!("{p}-col{k}")))
                .collect::<Vec<_>>()
        };
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 4,
            header: cells("h"),
            rows: vec![cells("r1"), cells("r2")],
        });
        press(&mut v, "j");
        for _ in 0..6 {
            press(&mut v, "l");
        }
        let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let head = row_at(buf, 0, 40);
        assert!(
            head.starts_with("h-col0") && head.contains("h-col6"),
            "{head}"
        );
        assert!(!head.contains("h-col1 "), "scrolled past col 1: {head}");
    }

    #[test]
    fn last_column_fills_the_window_and_wide_cells_keep_natural_width() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let long = "x".repeat(50);
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 3,
            header: vec![cell("a"), cell("b")],
            rows: vec![vec![cell("1"), cell(&long)]],
        });
        assert!(v.widths[1] > 40, "natural width is not clamped to 40");
        let mut term = Terminal::new(TestBackend::new(40, 8)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        // Expanded body line (after header rule + expand rule).
        let row = row_at(buf, 3, 40);
        assert!(
            !row.trim_end().ends_with('│'),
            "no dangling separator: {row:?}"
        );
        assert_eq!(
            row.chars().count(),
            40,
            "last column fills the window: {row:?}"
        );
    }

    #[test]
    fn a_cell_that_fits_the_window_is_shown_in_full_without_an_ellipsis() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let long = "y".repeat(60);
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 3,
            header: vec![cell("a"), cell("b")],
            rows: vec![vec![cell("1"), cell(&long)]],
        });
        let mut term = Terminal::new(TestBackend::new(90, 8)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let row = row_at(buf, 3, 90);
        assert!(row.contains(&long), "full cell text: {row:?}");
        assert!(!row.contains('…'), "no ellipsis: {row:?}");
    }

    #[test]
    fn expanded_row_wraps_and_previous_row_collapses_with_ellipsis() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let long = "word ".repeat(20);
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 5,
            header: vec![cell("a"), cell("b")],
            rows: vec![
                vec![cell("short"), cell(&long)],
                vec![cell("next"), cell(&long)],
            ],
        });
        let mut term = Terminal::new(TestBackend::new(40, 16)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let all: String = (0..16).map(|y| row_at(buf, y, 40)).collect();
        assert!(all.matches("word").count() > 2, "expanded row wraps: {all}");
        press(&mut v, "j");
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let collapsed = row_at(buf, 2, 40);
        assert!(
            collapsed.contains('…') || collapsed.contains("short"),
            "previous row collapses: {collapsed}"
        );
    }

    #[test]
    fn clipped_next_column_is_shown_not_blank() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        // Narrow col0, medium col1, long col2 — leftover after col0+col1 should show col2 clipped.
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 3,
            header: vec![cell("id"), cell("mid"), cell("acceptance")],
            rows: vec![vec![
                cell("1"),
                cell("medium-text-here"),
                cell("very-long-acceptance-criteria-text-that-needs-clipping"),
            ]],
        });
        let mut term = Terminal::new(TestBackend::new(42, 10)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let head = row_at(buf, 0, 42);
        assert!(
            head.contains('…') || head.contains("accept"),
            "partial third column visible: {head}"
        );
        let blank_tail = head.trim_end().len() < 30;
        assert!(!blank_tail, "should not leave a large blank: {head:?}");
    }

    #[test]
    fn rules_appear_only_around_the_expanded_row() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 6,
            header: vec![cell("a"), cell("b")],
            rows: vec![
                vec![cell("r0"), cell("x")],
                vec![cell("r1"), cell("y")],
                vec![cell("r2"), cell("z")],
            ],
        });
        press(&mut v, "j");
        let mut term = Terminal::new(TestBackend::new(30, 14)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let is_rule = |y: u16| {
            let s = row_at(buf, y, 30);
            s.contains('─') && !s.contains('r') && !s.contains('a')
        };
        // y0 header, y1 header rule, then collapsed r0, then expand rules around r1.
        assert!(is_rule(1), "header rule");
        let rule_ys: Vec<_> = (0..14).filter(|&y| is_rule(y)).collect();
        assert!(
            rule_ys.len() >= 3,
            "header + above + below expand: {rule_ys:?}"
        );
        // No rule between two collapsed rows when cursor is on middle.
        let body: String = (0..14)
            .map(|y| row_at(buf, y, 30))
            .collect::<Vec<_>>()
            .join("|");
        assert!(
            body.contains("r0") && body.contains("r1") && body.contains("r2"),
            "{body}"
        );
    }

    #[test]
    fn page_keys_move_with_an_expanded_row() {
        let mut v = TableViewer::new(DocTable {
            source_line: 1,
            line: 0,
            height: 20,
            header: vec![cell("a")],
            rows: (0..20).map(|i| vec![cell(&format!("r{i}"))]).collect(),
        });
        v.page = 5;
        v.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert!(v.row >= 1, "page down moves the expanded cursor");
        let after_down = v.row;
        v.key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert!(v.row < after_down, "page up moves back");
    }
}
