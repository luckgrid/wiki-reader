//! Code viewer: a [`ModalContent`] over one [`DocCodeBlock`].

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use wiki_reader_render::DocCodeBlock;

use super::modal_viewer::{ModalContent, ModalEvent};
use super::text_col::{char_width, line_width};
use super::theme::Theme;

const GUTTER: &str = "│ ";
const GUTTER_W: u16 = 2;
const HINT: &str = "↑↓←→ scroll  y copy  Esc close";

/// Open code viewer state.
pub struct CodeViewer {
    code: DocCodeBlock,
    /// First visible line.
    top: usize,
    /// Horizontal scroll in display columns (past the gutter).
    left: u16,
    /// Body rows that fit, from the last draw.
    page: usize,
}

impl CodeViewer {
    #[must_use]
    pub fn new(code: DocCodeBlock) -> Self {
        Self {
            code,
            top: 0,
            left: 0,
            page: 10,
        }
    }

    fn max_line_w(&self) -> u16 {
        self.code
            .lines
            .iter()
            .map(|l| line_width(l))
            .max()
            .unwrap_or(0)
    }

    fn scroll_y(&mut self, delta: isize) {
        let last = self.code.lines.len().saturating_sub(1);
        self.top = self.top.saturating_add_signed(delta).min(last);
    }

    fn scroll_x(&mut self, delta: i16) {
        let max = self.max_line_w().saturating_sub(1);
        self.left = self.left.saturating_add_signed(delta).min(max);
    }

    fn slice_line(&self, line: &str, avail: u16) -> String {
        let mut start = 0u16;
        let mut out = String::new();
        let mut used = 0u16;
        for ch in line.chars() {
            let w = char_width(ch);
            if start + w <= self.left {
                start = start.saturating_add(w);
                continue;
            }
            if used + w > avail {
                break;
            }
            out.push(ch);
            used = used.saturating_add(w);
            start = start.saturating_add(w);
        }
        out
    }
}

impl ModalContent for CodeViewer {
    fn label(&self) -> &'static str {
        "CODE"
    }

    fn want(&mut self, _: (u16, u16)) -> (u16, u16) {
        let w = self.max_line_w().saturating_add(GUTTER_W);
        let h = u16::try_from(self.code.lines.len().max(1)).unwrap_or(u16::MAX);
        (w, h)
    }

    fn title(&self) -> String {
        let lang = if self.code.lang.is_empty() {
            "code"
        } else {
            self.code.lang.as_str()
        };
        format!("Code (line {}) · {lang}", self.code.source_line)
    }

    fn hint(&self) -> String {
        HINT.into()
    }

    fn key(&mut self, key: KeyEvent) -> ModalEvent {
        let page = isize::try_from(self.page.max(1)).unwrap_or(1);
        match key.code {
            KeyCode::Char('q') => return ModalEvent::Close,
            KeyCode::Up | KeyCode::Char('k') => self.scroll_y(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_y(1),
            KeyCode::PageUp => self.scroll_y(-page),
            KeyCode::PageDown => self.scroll_y(page),
            KeyCode::Char('g') => self.top = 0,
            KeyCode::Char('G') => self.top = self.code.lines.len().saturating_sub(1),
            KeyCode::Home | KeyCode::Char('0') => self.left = 0,
            KeyCode::End | KeyCode::Char('$') => self.left = self.max_line_w().saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => self.scroll_x(-1),
            KeyCode::Right | KeyCode::Char('l') => self.scroll_x(1),
            KeyCode::Char('y') => {
                return ModalEvent::Copy(self.code.lines.join("\n"));
            }
            _ => {}
        }
        let _ = key.modifiers;
        ModalEvent::Stay
    }

    fn draw(&mut self, frame: &mut Frame<'_>, body: Rect, theme: &Theme) {
        let rows = usize::from(body.height).max(1);
        self.page = rows;
        let avail = body.width.saturating_sub(GUTTER_W);
        let mut lines = Vec::new();
        for line in self.code.lines.iter().skip(self.top).take(rows) {
            let text = self.slice_line(line, avail);
            lines.push(Line::from(vec![
                Span::styled(GUTTER, theme.muted()),
                Span::styled(
                    text,
                    theme.style_kind(wiki_reader_render::StyleKind::CodeBlock),
                ),
            ]));
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled("(empty)", theme.muted())));
        }
        frame.render_widget(Paragraph::new(lines), body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn viewer(lines: &[&str]) -> CodeViewer {
        CodeViewer::new(DocCodeBlock {
            source_line: 3,
            line: 0,
            height: u32::try_from(lines.len() + 1).unwrap(),
            lang: "rust".into(),
            lines: lines.iter().map(|s| (*s).to_owned()).collect(),
        })
    }

    fn press(v: &mut CodeViewer, c: char) -> ModalEvent {
        v.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn want_is_longest_line_plus_gutter() {
        let mut v = viewer(&["hi", "abcdefghij"]);
        assert_eq!(v.want((80, 20)), (12, 2));
    }

    #[test]
    fn y_copies_the_full_source() {
        let mut v = viewer(&["one", "two"]);
        assert_eq!(press(&mut v, 'y'), ModalEvent::Copy("one\ntwo".into()));
    }

    #[test]
    fn horizontal_scroll_moves_left() {
        let mut v = viewer(&["abcdefghijklmnopqrstuvwxyz"]);
        press(&mut v, 'l');
        press(&mut v, 'l');
        assert_eq!(v.left, 2);
        press(&mut v, '0');
        assert_eq!(v.left, 0);
        press(&mut v, '$');
        assert_eq!(v.left, 25);
    }
}
