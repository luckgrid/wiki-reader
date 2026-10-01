//! Display-column math for the View's column cursor and selection.
//!
//! Columns are terminal cells (wide glyphs take two), so every helper walks
//! chars with their display width instead of indexing bytes.

use ratatui::text::Span;

/// Display width of one char (at least 1, so zero-width marks never stall a walk).
#[must_use]
pub fn char_width(ch: char) -> u16 {
    u16::try_from(Span::raw(ch.to_string()).width())
        .unwrap_or(1)
        .max(1)
}

/// Display width of `s` in columns.
#[must_use]
pub fn line_width(s: &str) -> u16 {
    s.chars()
        .fold(0u16, |w, ch| w.saturating_add(char_width(ch)))
}

/// Start column of the char containing `col`, clamped to the last char of the
/// line (an empty line has only column 0).
#[must_use]
pub fn clamp_col(line: &str, col: u16) -> u16 {
    let mut start = 0u16;
    let mut last = 0u16;
    for ch in line.chars() {
        let w = char_width(ch);
        last = start;
        if col < start.saturating_add(w) {
            return start;
        }
        start = start.saturating_add(w);
    }
    last
}

/// Start column of the next char after the one at `col`; stays put on the last char.
#[must_use]
pub fn step_right(line: &str, col: u16) -> u16 {
    let col = clamp_col(line, col);
    let mut start = 0u16;
    for ch in line.chars() {
        let end = start.saturating_add(char_width(ch));
        if start == col {
            return if end < line_width(line) { end } else { col };
        }
        start = end;
    }
    col
}

/// Start column of the char before the one at `col`; `None` at column 0.
#[must_use]
pub fn step_left(line: &str, col: u16) -> Option<u16> {
    let col = clamp_col(line, col);
    if col == 0 {
        return None;
    }
    let mut start = 0u16;
    let mut prev = 0u16;
    for ch in line.chars() {
        if start >= col {
            break;
        }
        prev = start;
        start = start.saturating_add(char_width(ch));
    }
    Some(prev)
}

/// The chars of `line` that start within display columns `[c0, c1)`.
#[must_use]
pub fn slice_cols(line: &str, c0: u16, c1: u16) -> String {
    let mut out = String::new();
    let mut start = 0u16;
    for ch in line.chars() {
        if start >= c1 {
            break;
        }
        if start >= c0 {
            out.push(ch);
        }
        start = start.saturating_add(char_width(ch));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_clamp_handle_wide_glyphs() {
        assert_eq!(line_width("ab漢c"), 5);
        // cols: a=0 b=1 漢=2..4 c=4
        assert_eq!(
            clamp_col("ab漢c", 3),
            2,
            "second cell of a wide glyph snaps back"
        );
        assert_eq!(clamp_col("ab漢c", 99), 4, "clamped to the last char");
        assert_eq!(clamp_col("", 5), 0);
    }

    #[test]
    fn stepping_moves_by_whole_chars() {
        let s = "ab漢c";
        assert_eq!(step_right(s, 0), 1);
        assert_eq!(step_right(s, 1), 2);
        assert_eq!(step_right(s, 2), 4, "skips both cells of 漢");
        assert_eq!(step_right(s, 4), 4, "stays on the last char");
        assert_eq!(step_left(s, 4), Some(2));
        assert_eq!(step_left(s, 2), Some(1));
        assert_eq!(step_left(s, 0), None);
        assert_eq!(step_right("", 0), 0);
        assert_eq!(step_left("", 0), None);
    }

    #[test]
    fn slice_takes_chars_starting_in_range() {
        assert_eq!(slice_cols("hello world", 6, 11), "world");
        assert_eq!(slice_cols("ab漢c", 2, 4), "漢");
        assert_eq!(
            slice_cols("ab漢c", 3, 5),
            "c",
            "a wide glyph starting before c0 is excluded"
        );
    }
}
