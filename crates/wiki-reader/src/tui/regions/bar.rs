//! Connected three-row bars for the View's tabs (top) and prev/next links (bottom).
//!
//! The pane's own `Block` draws the outer frame. A bar adds the seam rule, the
//! `│` dividers between cells and the `┬` / `┴` junctions where the dividers meet
//! the rules. Every glyph is thin and uses the single `border` style the caller
//! passes (the pane's border style), so the bar always matches its pane.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;

/// Rows a bar occupies, counting the pane border row it shares.
pub const BAR_ROWS: u16 = 3;

/// One bordered cell on the label row.
pub struct Cell {
    /// Absolute column of the first label column.
    pub x: u16,
    /// Label width in columns (padding included).
    pub width: u16,
    /// Label text; its width equals `width`.
    pub line: Line<'static>,
}

/// Draw a three-row bar into `area` (full pane width, `BAR_ROWS` high).
///
/// `top`: the pane border is row 0 and the seam is row 2. Otherwise the seam is
/// row 0 and the pane border is row 2. Dividers sit beside each cell, except where
/// that column is the pane's own left or right border.
pub fn draw(frame: &mut Frame<'_>, area: Rect, top: bool, cells: &[Cell], border: Style) {
    if area.height < BAR_ROWS || area.width < 3 {
        return;
    }
    let left = area.x;
    let right = area.right() - 1;
    let label_y = area.y + 1;
    let seam_y = if top { area.y + 2 } else { area.y };
    let buf = frame.buffer_mut();

    for x in left + 1..right {
        buf[(x, label_y)].set_symbol(" ").set_style(border);
        buf[(x, seam_y)].set_symbol("─").set_style(border);
    }
    buf[(left, seam_y)].set_symbol("├").set_style(border);
    buf[(right, seam_y)].set_symbol("┤").set_style(border);

    for cell in cells {
        let end = cell.x.saturating_add(cell.width);
        let dividers = [cell.x.checked_sub(1), Some(end)];
        for dx in dividers.into_iter().flatten() {
            if dx <= left || dx >= right {
                continue;
            }
            buf[(dx, area.y)].set_symbol("┬").set_style(border);
            buf[(dx, label_y)].set_symbol("│").set_style(border);
            buf[(dx, area.y + 2)].set_symbol("┴").set_style(border);
        }
        buf.set_line(cell.x, label_y, &cell.line, cell.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::widgets::{Block, Borders};

    fn render(top: bool, cells: &[Cell]) -> Terminal<TestBackend> {
        let border = Style::default().fg(Color::Red);
        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(
                    Block::default().borders(Borders::ALL).border_style(border),
                    area,
                );
                draw(frame, area, top, cells, border);
            })
            .unwrap();
        terminal
    }

    fn row(terminal: &Terminal<TestBackend>, y: u16) -> String {
        let buf = terminal.backend().buffer();
        (0..20).map(|x| buf[(x, y)].symbol()).collect()
    }

    fn cell(x: u16, text: &str) -> Cell {
        Cell {
            x,
            width: u16::try_from(text.chars().count()).unwrap(),
            line: Line::from(text.to_owned()),
        }
    }

    #[test]
    fn top_bar_joins_dividers_to_the_frame_and_seam() {
        let t = render(true, &[cell(1, " a.md × "), cell(10, " b ")]);
        assert_eq!(row(&t, 0), "┌────────┬───┬─────┐");
        assert_eq!(row(&t, 1), "│ a.md × │ b │     │");
        assert_eq!(row(&t, 2), "├────────┴───┴─────┤");
    }

    #[test]
    fn bottom_bar_has_the_seam_above_and_the_frame_below() {
        let t = render(false, &[cell(1, " ‹ p "), cell(14, " n › ")]);
        assert_eq!(row(&t, 0), "├─────┬──────┬─────┤");
        assert_eq!(row(&t, 1), "│ ‹ p │      │ n › │");
        assert_eq!(row(&t, 2), "└─────┴──────┴─────┘");
    }

    #[test]
    fn every_glyph_uses_the_pane_border_style_and_stays_thin() {
        let t = render(true, &[cell(1, " a.md × ")]);
        let buf = t.backend().buffer();
        for y in [0, 2] {
            for x in 0..20 {
                assert_eq!(buf[(x, y)].fg, Color::Red, "{x},{y}");
                assert!(!"━┃┏┓┣┫┳┻┗┛".contains(buf[(x, y)].symbol()));
            }
        }
        assert_eq!(buf[(9, 1)].fg, Color::Red);
    }
}
