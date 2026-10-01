//! Status bar (F2).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::layout;
use crate::tui::theme::Theme;

/// Status fields drawn into one line.
pub struct StatusModel<'a> {
    /// Pill label: the focused pane (`NAV` / `VIEW`) or the open popup.
    pub mode_label: &'a str,
    /// Relative path.
    pub path: &'a str,
    /// Cursor line (1-based display).
    pub line: u32,
    /// Cursor column (1-based display).
    pub col: u16,
    /// Scroll percentage 0–100.
    pub pct: u32,
    /// Word count.
    pub words: u32,
    /// Reading time minutes (ceil words/230).
    pub minutes: u32,
    /// Updated frontmatter or "—".
    pub updated: &'a str,
    /// Frontmatter `status:` value, shown after the date.
    pub status: Option<&'a str>,
    /// Message area (focused-item target or transient notice).
    pub message: &'a str,
}

/// Display columns for `s` (CJK/emoji-safe; matches ratatui cell width).
fn col_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Truncate `s` to at most `max` display columns.
fn truncate_cols(s: &str, max: usize) -> String {
    if col_width(s) <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = col_width(&ch.to_string());
        if used + w > max {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

/// One status field: text + style.
type Field = (String, Style);

/// Draw the status bar. When narrow, drop lower-priority fields before the message.
pub fn draw(frame: &mut Frame<'_>, area: Rect, model: &StatusModel<'_>, theme: &Theme) {
    let area = layout::chrome_pad(area);
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
    let msg_len = if msg.is_empty() {
        0
    } else {
        SEP.chars().count() + col_width(msg)
    };
    let budget = w.saturating_sub(msg_len);

    // One pill colour for both panes: the label says which pane has focus.
    let mode_bg = theme.gold;
    let text = theme.text();
    // High → low priority fields (drop from the end when narrowing). Date and
    // status outrank word count and reading time.
    let mut fields: Vec<Field> = vec![
        (
            format!(" {} ", model.mode_label),
            Style::default()
                .bg(mode_bg)
                .fg(theme.on_peach)
                .add_modifier(Modifier::BOLD),
        ),
        (model.path.to_owned(), text),
        (
            format!("L{}:C{} {}%", model.line, model.col, model.pct),
            text,
        ),
        (model.updated.to_owned(), theme.secondary()),
    ];
    if let Some(status) = model.status.filter(|s| !s.is_empty()) {
        fields.push((status.to_owned(), theme.status_style(status)));
    }
    fields.push((format!("{}w", model.words), text));
    fields.push((reading, text));
    while fields_width(&fields) > budget && fields.len() > 2 {
        fields.pop();
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, (text, style)) in fields.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(SEP, theme.muted()));
        }
        spans.push(Span::styled(text, style));
    }
    if !msg.is_empty() {
        spans.push(Span::styled(SEP, theme.muted()));
        spans.push(Span::styled(msg.to_owned(), theme.secondary()));
    }
    frame.render_widget(Paragraph::new(Line::from(clip_spans(spans, w))), area);
}

const SEP: &str = " · ";

fn fields_width(fields: &[Field]) -> usize {
    let text: usize = fields.iter().map(|(t, _)| col_width(t)).sum();
    text + fields.len().saturating_sub(1) * SEP.chars().count()
}

/// Clip a span list to `max` display columns.
pub(crate) fn clip_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let mut used = 0;
    let mut out = Vec::new();
    for sp in spans {
        let w = col_width(&sp.content);
        if used + w <= max {
            used += w;
            out.push(sp);
        } else {
            let cut = truncate_cols(&sp.content, max - used);
            if !cut.is_empty() {
                out.push(Span::styled(cut, sp.style));
            }
            break;
        }
    }
    out
}

/// Reading time in minutes, rounded up (words / 230).
#[must_use]
pub fn reading_minutes(words: u32) -> u32 {
    if words == 0 {
        return 0;
    }
    words.div_ceil(230)
}
