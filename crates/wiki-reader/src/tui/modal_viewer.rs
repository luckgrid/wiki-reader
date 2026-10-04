//! Modal viewer shell (P3-14), shared with the image / diagram viewer (P3-15).
//!
//! The shell owns what every full-screen viewer needs: input mode (keys go to the modal only),
//! `Esc` to close, a bordered panel with a key-hint footer, mouse wheel → `Up` / `Down`, click
//! outside to dismiss, and draw order. The panel is drawn **last**: pictures are drawn before
//! popups and a popup's `Clear` covers them (image → `Clear` → popup, ADR-0004). Content only
//! fills the body rect and handles keys, see [`ModalContent`].

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Margin, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::hit::{Hit, HitMap};
use super::regions::footer::ellipsis;
use super::regions::overlay::{popup_accent, popup_block};
use super::theme::Theme;

/// What the app should do after a modal key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalEvent {
    /// Keep the modal open.
    Stay,
    /// Close the modal.
    Close,
    /// Copy this text to the clipboard (OSC 52), keep the modal open.
    Copy(String),
}

/// Body of a modal viewer: the table viewer (P3-14) and the image / diagram viewer (P3-15).
///
/// Pictures are drawn from [`Self::draw`]: the shell calls it after the panel's `Clear`, so a
/// content-owned image lands over the cleared cells (image → `Clear` → popup order holds).
pub trait ModalContent {
    /// Status-bar mode label.
    fn label(&self) -> &'static str;
    /// True while background work is pending, so the event loop polls fast and redraws.
    fn busy(&self) -> bool {
        false
    }
    /// Body size (columns, rows) the content wants, given the most the window can offer.
    /// The shell sizes the panel to it, never beyond `cap`; the default fills the cap.
    /// Called before every draw, so content may start background work from here.
    fn want(&mut self, cap: (u16, u16)) -> (u16, u16) {
        cap
    }
    /// Text on the top border.
    fn title(&self) -> String;
    /// Footer line: key hints, or a prompt while [`Self::editing`].
    fn hint(&self) -> String;
    /// True while the content swallows typed text; the shell then lets `Esc` through.
    fn editing(&self) -> bool {
        false
    }
    /// Handle one key (never `Esc` unless [`Self::editing`]).
    fn key(&mut self, key: KeyEvent) -> ModalEvent;
    /// Paint into `body` (inside the border, above the footer).
    fn draw(&mut self, frame: &mut Frame<'_>, body: Rect, theme: &Theme);
}

/// Route a key: `Esc` closes unless the content is editing, everything else is the content's.
pub fn handle_key(content: &mut dyn ModalContent, key: KeyEvent) -> ModalEvent {
    if key.code == KeyCode::Esc && !content.editing() {
        return ModalEvent::Close;
    }
    content.key(key)
}

/// Narrowest body: room for the key hint's first words and the title.
const MIN_BODY_W: u16 = 40;

/// The panel for `content` inside `cap`: border (2 cols, 2 rows) and footer (1 row) around the
/// body it asks for, centred, never larger than `cap` and never narrower than [`MIN_BODY_W`].
fn fit(cap: Rect, content: &mut dyn ModalContent) -> Rect {
    let (max_w, max_h) = (cap.width.saturating_sub(2), cap.height.saturating_sub(3));
    if max_w == 0 || max_h == 0 {
        return cap;
    }
    let (w, h) = content.want((max_w, max_h));
    let w = w.max(MIN_BODY_W).min(max_w) + 2;
    let h = h.max(1).min(max_h) + 3;
    Rect {
        x: cap.x + (cap.width - w) / 2,
        y: cap.y + (cap.height - h) / 2,
        width: w,
        height: h,
    }
}

/// Draw the panel over `area` and let `content` fill it. Call after every other layer.
pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    content: &mut dyn ModalContent,
    theme: &Theme,
    hits: &mut HitMap,
) {
    hits.push(area, Hit::ModalDismiss);
    // Leave the header and status rows visible around a roomy panel; tiny terminals get all.
    let cap = if area.width >= 30 && area.height >= 10 {
        area.inner(Margin::new(2, 1))
    } else {
        area
    };
    let rect = fit(cap, content);
    frame.render_widget(Clear, rect);
    hits.occlude(rect);
    let title = Line::from(Span::styled(
        format!(
            " {} ",
            ellipsis(&content.title(), usize::from(rect.width).saturating_sub(4))
        ),
        popup_accent(theme),
    ));
    let block = popup_block(title, theme);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    hits.push(rect, Hit::ModalBody);
    if inner.width == 0 || inner.height < 2 {
        return;
    }
    let body = Rect {
        height: inner.height - 1,
        ..inner
    };
    content.draw(frame, body, theme);
    let foot = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    let hint = ellipsis(&content.hint(), usize::from(foot.width));
    frame.render_widget(Paragraph::new(Span::styled(hint, theme.muted())), foot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    struct Typing(bool);
    impl ModalContent for Typing {
        fn label(&self) -> &'static str {
            "TEST"
        }
        fn title(&self) -> String {
            String::new()
        }
        fn hint(&self) -> String {
            String::new()
        }
        fn editing(&self) -> bool {
            self.0
        }
        fn key(&mut self, _: KeyEvent) -> ModalEvent {
            ModalEvent::Stay
        }
        fn draw(&mut self, _: &mut Frame<'_>, _: Rect, _: &Theme) {}
    }

    #[test]
    fn panel_is_content_sized_and_capped() {
        struct Want((u16, u16));
        impl ModalContent for Want {
            fn label(&self) -> &'static str {
                "TEST"
            }
            fn want(&mut self, _: (u16, u16)) -> (u16, u16) {
                self.0
            }
            fn title(&self) -> String {
                String::new()
            }
            fn hint(&self) -> String {
                String::new()
            }
            fn key(&mut self, _: KeyEvent) -> ModalEvent {
                ModalEvent::Stay
            }
            fn draw(&mut self, _: &mut Frame<'_>, _: Rect, _: &Theme) {}
        }
        let cap = Rect::new(2, 1, 96, 38);
        // Small content: centred, with border + footer around the body (and the width floor).
        let r = fit(cap, &mut Want((10, 5)));
        assert_eq!((r.width, r.height), (MIN_BODY_W + 2, 5 + 3));
        assert_eq!(
            (r.x, r.y),
            (2 + (96 - r.width) / 2, 1 + (38 - r.height) / 2)
        );
        // Huge content is capped at today's size.
        assert_eq!(fit(cap, &mut Want((999, 999))), cap);
    }

    #[test]
    fn esc_closes_unless_content_is_editing() {
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(handle_key(&mut Typing(false), esc), ModalEvent::Close);
        assert_eq!(handle_key(&mut Typing(true), esc), ModalEvent::Stay);
    }
}
