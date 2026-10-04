//! Table viewer (P3-14): a [`ModalContent`] over one [`DocTable`].
//!
//! Two-axis scroll with a fixed header row and first column, row filter, column sort, and
//! cell / row copy (tab-separated, like the inline table copy from P2-R38). Cells keep their
//! inline styling (bold, link colour, code) from [`DocCell`]; filter, sort and copy use the
//! plain text.

use std::cmp::Ordering;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_render::{DocCell, DocTable, StyleKind};

use super::modal_viewer::{ModalContent, ModalEvent};
use super::text_col::line_width;
use super::theme::Theme;

const SEP: &str = " │ ";
/// Display columns of [`SEP`] (its `len()` is bytes).
const SEP_W: usize = 3;
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
}

impl TableViewer {
    #[must_use]
    pub fn new(table: DocTable) -> Self {
        let plain = |r: &[DocCell]| r.iter().map(DocCell::text).collect::<Vec<_>>();
        let head = plain(&table.header);
        let text: Vec<Vec<String>> = table.rows.iter().map(|r| plain(r)).collect();
        let widths = (0..head.len())
            .map(|k| {
                let cells = text.iter().map(|r| show(&r[k]));
                let w = cells
                    .chain(std::iter::once(show(&head[k])))
                    .map(|c| usize::from(line_width(&c)))
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
            text,
            table,
        };
        v.refresh();
        v
    }

    /// Re-run filter and sort over the body rows.
    fn refresh(&mut self) {
        let needle = self.filter.to_lowercase();
        self.view = (0..self.text.len())
            .filter(|&i| {
                needle.is_empty()
                    || self.text[i]
                        .iter()
                        .any(|c| c.to_lowercase().contains(&needle))
            })
            .collect();
        if let Some((c, desc)) = self.sort {
            let rows = &self.text;
            // Stable, so equal keys keep document order.
            self.view.sort_by(|&a, &b| {
                let o = cmp_cells(&rows[a][c], &rows[b][c]);
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
        let w0 = self.widths[0].min(w0_cap).max(3);
        let rest_cap = avail.saturating_sub(SEP_W + w0).max(3);
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

    /// Visible column indexes (col 0 sticky, then from `self.left`) for `avail` cells.
    fn visible_cols(&self, widths: &[usize], avail: usize) -> Vec<usize> {
        let ncols = widths.len();
        if ncols == 0 {
            return Vec::new();
        }
        let mut cols = vec![0];
        let mut used = widths[0];
        for (k, &w) in widths.iter().enumerate().skip(self.left.min(ncols)) {
            if k == 0 {
                continue;
            }
            let next = used + SEP_W + w;
            if next > avail && k > self.left {
                break;
            }
            used = next;
            cols.push(k);
        }
        cols
    }

    /// Last column fully visible when scrolled to `self.left` in `avail` cells.
    fn last_full(&self, avail: usize) -> usize {
        let widths = self.eff_widths(avail);
        *self.visible_cols(&widths, avail).last().unwrap_or(&0)
    }

    fn keep_cursor_visible(&mut self, avail: usize, rows: usize) {
        if self.row < self.top {
            self.top = self.row;
        } else if self.row >= self.top + rows {
            self.top = self.row + 1 - rows;
        }
        if self.col > 0 {
            self.left = self.left.clamp(1, self.col);
            while self.left < self.col && self.last_full(avail) < self.col {
                self.left += 1;
            }
        }
    }

    /// Header rule matching the visible columns (`─` / `─┼─`).
    fn rule_line(&self, avail: usize, theme: &Theme) -> Line<'static> {
        let mut widths = self.eff_widths(avail);
        let cols = self.visible_cols(&widths, avail);
        let last_is_final = cols.last().is_some_and(|&k| k + 1 == widths.len());
        if last_is_final {
            let used: usize = cols.iter().map(|&k| widths[k]).sum::<usize>()
                + cols.len().saturating_sub(1) * SEP_W;
            if let Some(&last) = cols.last() {
                widths[last] += avail.saturating_sub(used);
            }
        }
        let mut spans = Vec::new();
        for (i, &k) in cols.iter().enumerate() {
            spans.push(Span::styled("─".repeat(widths[k]), theme.muted()));
            let is_last = i + 1 == cols.len();
            if !(last_is_final && is_last) {
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
        avail: usize,
        base: Style,
        cursor_row: bool,
        theme: &Theme,
        arrow: Option<char>,
    ) -> Line<'static> {
        let mut widths = self.eff_widths(avail);
        let cols = self.visible_cols(&widths, avail);
        let last_is_final = cols.last().is_some_and(|&k| k + 1 == widths.len());
        if last_is_final {
            let used: usize = cols.iter().map(|&k| widths[k]).sum::<usize>()
                + cols.len().saturating_sub(1) * SEP_W;
            if let Some(&last) = cols.last() {
                widths[last] += avail.saturating_sub(used);
            }
        }
        let sep_style = if cursor_row {
            theme.muted().bg(theme.cursor_line)
        } else {
            theme.muted()
        };
        let mut spans = Vec::new();
        for (i, &k) in cols.iter().enumerate() {
            let arrow = arrow.filter(|_| self.sort.is_some_and(|(c, _)| c == k));
            if cursor_row && k == self.col {
                let cursor = Style::default().fg(theme.on_peach).bg(theme.peach);
                spans.extend(cell_spans(&cells[k], widths[k], arrow, cursor, None));
            } else {
                spans.extend(cell_spans(&cells[k], widths[k], arrow, base, Some(theme)));
            }
            let is_last = i + 1 == cols.len();
            if !(last_is_final && is_last) {
                spans.push(Span::styled(SEP, sep_style));
            }
        }
        Line::from(spans)
    }
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
        let style = match (theme, run.kind) {
            (Some(t), k)
                if !matches!(
                    k,
                    StyleKind::Plain | StyleKind::Table | StyleKind::TableHeader
                ) =>
            {
                base.patch(t.style_kind(k))
            }
            _ => base,
        };
        let mut piece = String::new();
        for ch in show(&run.text).chars() {
            let cw = usize::from(line_width(ch.encode_utf8(&mut [0; 4])));
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

impl ModalContent for TableViewer {
    fn label(&self) -> &'static str {
        "TABLE"
    }

    fn want(&mut self, cap: (u16, u16)) -> (u16, u16) {
        // All columns and all body rows (not the filtered count, so the panel holds still while
        // typing a filter); header + rule + body (or "no rows match"). `cap` sizes the panel
        // via fit; draw calls eff_widths with the real body width.
        let _ = cap;
        let w = self.widths.iter().map(|w| w + SEP_W).sum::<usize>();
        let h = 2 + self.text.len().max(1);
        let to_u16 = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        (to_u16(w), to_u16(h))
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
        let rows = usize::from(body.height.saturating_sub(2)).max(1);
        let avail = usize::from(body.width);
        self.page = rows;
        self.keep_cursor_visible(avail, rows);
        let head = self.cells_line(
            &self.table.header,
            avail,
            theme.text().add_modifier(Modifier::BOLD),
            false,
            theme,
            Some(if self.sort.is_some_and(|(_, d)| d) {
                '▼'
            } else {
                '▲'
            }),
        );
        let mut lines = vec![head, self.rule_line(avail, theme)];
        for (n, &i) in self.view.iter().enumerate().skip(self.top).take(rows) {
            let cursor = n == self.row;
            let style = if cursor {
                theme.text().bg(theme.cursor_line)
            } else {
                theme.text()
            };
            lines.push(self.cells_line(&self.table.rows[i], avail, style, cursor, theme, None));
        }
        if self.view.is_empty() {
            lines.push(Line::from(Span::styled("no rows match", theme.muted())));
        }
        frame.render_widget(Paragraph::new(lines), body);
    }
}

/// Cell text for one terminal line: control characters become spaces.
fn show(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Total order: numbers (by value) before text (case-insensitive).
fn cmp_cells(a: &str, b: &str) -> Ordering {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => x.total_cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.to_lowercase().cmp(&b.to_lowercase()),
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
    fn copy_cell_and_row() {
        let mut v = viewer();
        press(&mut v, "jl");
        assert_eq!(press(&mut v, "y"), ModalEvent::Copy("9".into()));
        assert_eq!(press(&mut v, "Y"), ModalEvent::Copy("alice\t9".into()));
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
        let mut term = Terminal::new(TestBackend::new(30, 4)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &theme)).unwrap();
        let buf = term.backend().buffer();
        assert!(buf[(0, 0)].modifier.contains(Modifier::BOLD), "bold header");
        let rule = (0..30).map(|x| buf[(x, 1)].symbol()).collect::<String>();
        assert!(
            rule.contains('─') && rule.contains('┼'),
            "header rule: {rule}"
        );
        let row = (0..30).map(|x| buf[(x, 2)].symbol()).collect::<String>();
        let x = u16::try_from(row.find("docs").unwrap()).unwrap();
        assert_eq!(buf[(x, 2)].fg, theme.link, "link colour in a body cell");
        // Content-sized: both columns plus separators; height is header + rule + body.
        // "docs" (4) + sort room beats header "ref" (3) → col1 width 6.
        assert_eq!(v.want((90, 30)), (6 + 3 + 6 + 3, 3));
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
        let mut term = Terminal::new(TestBackend::new(40, 6)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let row = |y: u16| (0..40).map(|x| buf[(x, y)].symbol()).collect::<String>();
        assert!(
            row(0).starts_with("h-col0") && row(0).contains("h-col6"),
            "{}",
            row(0)
        );
        assert!(
            row(3).starts_with("r2-col0") && row(3).contains("r2-col6"),
            "{}",
            row(3)
        );
        assert!(
            !row(0).contains("h-col1 "),
            "scrolled past col 1: {}",
            row(0)
        );
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
        let mut term = Terminal::new(TestBackend::new(40, 5)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let row = (0..40).map(|x| buf[(x, 2)].symbol()).collect::<String>();
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
        let mut term = Terminal::new(TestBackend::new(90, 5)).unwrap();
        term.draw(|f| v.draw(f, f.area(), &Theme::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let row = (0..90).map(|x| buf[(x, 2)].symbol()).collect::<String>();
        assert!(row.contains(&long), "full cell text: {row:?}");
        assert!(!row.contains('…'), "no ellipsis: {row:?}");
    }
}
