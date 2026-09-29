//! Status bar (F2).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::tui::focus::FocusPane;
use crate::tui::theme::Theme;

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
    /// Message area (focused-item target or transient notice).
    pub message: &'a str,
}

/// Draw the status bar. When narrow, drop lower-priority fields before the message.
pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &StatusModel<'_>, theme: &Theme) {
    if area.width == 0 {
        return;
    }
    let reading = if model.minutes == 0 && model.words > 0 {
        "<1m".to_owned()
    } else {
        format!("{}m", model.minutes)
    };
    let w = usize::from(area.width);
    let msg = model.message;
    let msg_suffix = if msg.is_empty() {
        String::new()
    } else {
        format!(" · {msg}")
    };
    let msg_len = msg_suffix.chars().count();
    let budget = w.saturating_sub(msg_len);

    // High → low priority fields (drop from the end when narrowing).
    let mut fields: Vec<String> = vec![
        model.focus.label().to_owned(),
        model.path.to_owned(),
        format!("L{} {}%", model.line, model.pct),
        format!("{}w", model.words),
        reading,
        model.updated.to_owned(),
    ];
    let mut head = join_fields(&fields);
    while head.chars().count() > budget && fields.len() > 2 {
        fields.pop();
        head = join_fields(&fields);
    }
    if head.chars().count() > budget {
        head = head.chars().take(budget).collect();
    }
    let text = format!("{head}{msg_suffix}");
    let chars: String = text.chars().take(w).collect();
    frame.render_widget(Paragraph::new(chars).style(theme.muted()), area);
}

fn join_fields(fields: &[String]) -> String {
    fields.join(" · ")
}

/// Reading time in minutes, rounded up (words / 230).
#[must_use]
pub fn reading_minutes(words: u32) -> u32 {
    if words == 0 {
        return 0;
    }
    words.div_ceil(230)
}
