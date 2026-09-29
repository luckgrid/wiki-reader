//! Status bar (F2).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

/// Which pane is focused (for the status label).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPane {
    /// Side nav.
    Nav,
    /// Viewer.
    #[default]
    Viewer,
}

impl FocusPane {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Nav => "NAV",
            Self::Viewer => "VIEWER",
        }
    }
}

/// Status fields drawn into one line.
pub struct StatusModel<'a> {
    /// Focused pane.
    pub focus: FocusPane,
    /// Relative path.
    pub path: &'a str,
    /// Cursor line (1-based display).
    pub line: u32,
    /// Scroll percentage 0–100.
    pub pct: u32,
    /// Word count.
    pub words: u32,
    /// Reading time minutes (ceil words/230).
    pub minutes: u32,
    /// Updated frontmatter or "—".
    pub updated: &'a str,
    /// Message area.
    pub message: &'a str,
}

/// Draw the status bar.
pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &StatusModel<'_>, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let reading = if model.minutes == 0 && model.words > 0 {
        "<1m".to_owned()
    } else {
        format!("{}m", model.minutes)
    };
    let mut text = format!(
        "{} · {} · L{} {}% · {}w · {} · {}",
        model.focus.label(),
        model.path,
        model.line,
        model.pct,
        model.words,
        reading,
        model.updated
    );
    if !model.message.is_empty() {
        text.push_str(" · ");
        text.push_str(model.message);
    }
    // Truncate to width.
    let chars: String = text.chars().take(usize::from(area.width)).collect();
    frame.render_widget(Paragraph::new(chars).style(theme.muted()), area);
}

/// Reading time in minutes, rounded up (words / 230).
#[must_use]
pub fn reading_minutes(words: u32) -> u32 {
    if words == 0 {
        return 0;
    }
    words.div_ceil(230)
}
