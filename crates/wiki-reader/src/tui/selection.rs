//! Mouse text selection in the View, and turning it back into copyable text.
//!
//! A selection is two display cells (`line`, `col`); both end cells are
//! included. Copying maps cells back to text: gutters and table borders are
//! dropped, table cells become tab-separated, and rows that were soft-wrapped
//! from one source line are re-joined with a space.

use wiki_reader_render::{StyleKind, StyledLine};

use super::page_doc::PageDoc;
use super::text_col::{line_width, slice_cols};
use super::viewer_doc::ViewerDoc;

/// One display cell: 0-based display row and display column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: u32,
    pub col: u16,
}

/// Anchor (where the drag began) and head (where it is now).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

impl Selection {
    /// A bare click selects nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// `(start, end)` in document order.
    #[must_use]
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Selected display columns `[c0, c1)` on `line` (whose text is `width`
    /// columns wide), or `None` when the line is outside the selection.
    #[must_use]
    pub fn cols_on(&self, line: u32, width: u16) -> Option<(u16, u16)> {
        if self.is_empty() {
            return None;
        }
        let (s, e) = self.ordered();
        if line < s.line || line > e.line {
            return None;
        }
        let c0 = if line == s.line { s.col } else { 0 };
        let c1 = if line == e.line {
            e.col.saturating_add(1)
        } else {
            width.max(1)
        };
        Some((c0, c1))
    }
}

const BOX_GLYPHS: &str = "│─┌┬┐├┼┤└┴┘";

fn is_box_row(text: &str) -> bool {
    text.chars().all(|c| c == ' ' || BOX_GLYPHS.contains(c))
}

fn has_kind(sl: &StyledLine, pred: impl Fn(StyleKind) -> bool) -> bool {
    sl.spans.iter().any(|s| pred(s.kind))
}

/// Text of display row `text` within columns `[c0, c1)`, or `None` for rows
/// that carry no copyable content (rules, table borders).
fn row_piece(text: &str, sl: Option<&StyledLine>, c0: u16, c1: u16) -> Option<String> {
    let Some(sl) = sl else {
        return Some(slice_cols(text, c0, c1));
    };
    if has_kind(sl, |k| k == StyleKind::Rule) {
        return None;
    }
    if has_kind(sl, |k| {
        matches!(k, StyleKind::Table | StyleKind::TableHeader)
    }) {
        let piece = slice_cols(text, c0, c1);
        if is_box_row(&piece) {
            return None;
        }
        let cells: Vec<&str> = piece
            .split('│')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .collect();
        return Some(cells.join("\t"));
    }
    // Quote / code rows start with a two-column "│ " gutter that isn't content.
    let gutter = sl.spans.first().is_some_and(|s| {
        s.text == "│ " && matches!(s.kind, StyleKind::Quote | StyleKind::CodeBlock)
    });
    let c0 = if gutter { c0.max(2) } else { c0 };
    Some(slice_cols(text, c0, c1.max(c0)))
}

/// Plain flowing text (not a table, code, rule or frontmatter box).
fn is_prose(sl: &StyledLine) -> bool {
    !has_kind(sl, |k| {
        matches!(
            k,
            StyleKind::Table
                | StyleKind::TableHeader
                | StyleKind::CodeBlock
                | StyleKind::CodeLang
                | StyleKind::Rule
                | StyleKind::Frontmatter
                | StyleKind::FrontmatterKey
                | StyleKind::FrontmatterValue
                | StyleKind::FrontmatterPunct
        )
    })
}

/// Was `next` wrapped off `prev`? True when `prev` plus the first word of `next`
/// would not have fit in `wrap_width` columns.
fn wrapped_from(prev: &str, next: &str, wrap_width: u16) -> bool {
    let first = next.split_whitespace().next().unwrap_or("");
    !first.is_empty()
        && line_width(prev.trim_end())
            .saturating_add(1)
            .saturating_add(line_width(first))
            > wrap_width
}

/// Copyable text for `sel` over `doc`; `wrap_width` is the rendered layout width.
#[must_use]
pub fn extract(doc: &PageDoc, sel: &Selection, wrap_width: u16) -> String {
    let lines = doc.lines();
    if lines.is_empty() || sel.is_empty() {
        return String::new();
    }
    let styled = doc.styled_lines();
    let (start, end) = sel.ordered();
    let last = usize::try_from(end.line).unwrap_or(0).min(lines.len() - 1);
    let first = usize::try_from(start.line).unwrap_or(0).min(last);

    let mut out = String::new();
    // (full text, source line, prose?) of the previous emitted row.
    let mut prev: Option<(&str, u32, bool)> = None;
    for (i, text) in lines.iter().enumerate().take(last + 1).skip(first) {
        let text = text.as_str();
        let line = u32::try_from(i).unwrap_or(u32::MAX);
        let sl = styled.and_then(|s| s.get(i));
        let Some((c0, c1)) = sel.cols_on(line, line_width(text)) else {
            continue;
        };
        let Some(piece) = row_piece(text, sl, c0, c1) else {
            continue;
        };
        let source = doc.source_cursor(line);
        let prose = sl.is_some_and(is_prose);
        let mut joined = false;
        match prev {
            None => {}
            Some((above, above_src, above_prose))
                if prose
                    && above_prose
                    && above_src == source
                    && wrapped_from(above, text, wrap_width) =>
            {
                out.truncate(out.trim_end_matches(' ').len());
                out.push(' ');
                joined = true;
            }
            Some(_) => out.push('\n'),
        }
        // A soft-wrapped continuation row drops its hanging indent.
        if joined {
            out.push_str(piece.trim_start());
        } else {
            out.push_str(&piece);
        }
        prev = Some((text, source, prose));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, col: u16) -> Pos {
        Pos { line, col }
    }

    #[test]
    fn columns_per_line_include_both_end_cells() {
        let sel = Selection {
            anchor: pos(2, 5),
            head: pos(4, 3),
        };
        assert_eq!(sel.cols_on(1, 20), None);
        assert_eq!(
            sel.cols_on(2, 20),
            Some((5, 20)),
            "first line: anchor to end"
        );
        assert_eq!(sel.cols_on(3, 20), Some((0, 20)), "middle line: whole row");
        assert_eq!(
            sel.cols_on(4, 20),
            Some((0, 4)),
            "last line: start to head, inclusive"
        );
        // Dragging backwards selects the same cells.
        let back = Selection {
            anchor: sel.head,
            head: sel.anchor,
        };
        assert_eq!(back.cols_on(2, 20), sel.cols_on(2, 20));
        assert_eq!(back.cols_on(4, 20), sel.cols_on(4, 20));
        // Same row: both end cells included.
        let one = Selection {
            anchor: pos(1, 2),
            head: pos(1, 6),
        };
        assert_eq!(one.cols_on(1, 20), Some((2, 7)));
        // A bare click selects nothing.
        let click = Selection {
            anchor: pos(1, 2),
            head: pos(1, 2),
        };
        assert!(click.is_empty());
        assert_eq!(click.cols_on(1, 20), None);
    }

    #[test]
    fn wrap_detection_needs_the_next_word_not_to_fit() {
        // 10-col layout: "aaaa bbbb" (9) + " cc" would exceed.
        assert!(wrapped_from("aaaa bbbb", "cc dd", 10));
        // Plenty of room: a new item, not a wrap.
        assert!(!wrapped_from("• One", "• Two", 40));
    }

    #[test]
    fn box_rows_and_table_cells() {
        assert!(is_box_row("├──────┼────┤"));
        assert!(!is_box_row("│ 0001 │ a │"));
    }
}
