//! Minimal semantic color tokens (ui-spec Theming).

use ratatui::style::{Color, Modifier, Style};

/// Theme tokens used by the shell.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // tokens reserved for later regions / P3-07 presets
pub struct Theme {
    /// Default background.
    pub surface: Color,
    /// Muted surface (dim panes).
    pub surface_muted: Color,
    /// Default border.
    pub border: Color,
    /// Focused pane border.
    pub border_focus: Color,
    /// Primary text.
    pub text: Color,
    /// Dim / secondary text.
    pub text_muted: Color,
    /// Accent (current page, icons).
    pub accent: Color,
    /// Cursor line highlight.
    pub cursor_line: Color,
    /// Focused Tab-cycle item.
    pub focus_item: Color,
    /// Internal link text.
    pub link: Color,
    /// Broken / unresolved link.
    pub link_broken: Color,
    /// External link.
    pub link_external: Color,
    /// Unsupported scheme (muted).
    pub link_unsupported: Color,
    /// Code block background tint.
    pub code_bg: Color,
    /// Quote / alert left-bar tint.
    pub quote_bar: Color,
    /// Heading colours H1…H6 (semantic; P3-07 can swap presets).
    pub heading: [Color; 6],
    /// Alert colours by kind id (0 unused, 1=NOTE …).
    pub alert: [Color; 9],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            surface: Color::Reset,
            surface_muted: Color::DarkGray,
            border: Color::DarkGray,
            border_focus: Color::Cyan,
            text: Color::Reset,
            text_muted: Color::DarkGray,
            accent: Color::Cyan,
            cursor_line: Color::Rgb(40, 40, 50),
            focus_item: Color::Yellow,
            link: Color::Blue,
            link_broken: Color::Red,
            link_external: Color::Magenta,
            link_unsupported: Color::DarkGray,
            code_bg: Color::Rgb(30, 32, 36),
            quote_bar: Color::Rgb(80, 80, 100),
            heading: [
                Color::Rgb(255, 120, 80),  // H1
                Color::Rgb(255, 180, 60),  // H2
                Color::Rgb(100, 200, 120), // H3
                Color::Rgb(80, 180, 220),  // H4
                Color::Rgb(160, 140, 220), // H5
                Color::Rgb(180, 180, 180), // H6
            ],
            alert: [
                Color::DarkGray,           // 0 unused
                Color::Rgb(80, 160, 220),  // NOTE
                Color::Rgb(80, 200, 140),  // TIP
                Color::Rgb(200, 160, 60),  // IMPORTANT
                Color::Rgb(220, 140, 60),  // WARNING
                Color::Rgb(220, 80, 80),   // CAUTION
                Color::Rgb(100, 200, 180), // GOAL
                Color::Rgb(140, 160, 220), // DECISION
                Color::Rgb(220, 100, 120), // RISK
            ],
        }
    }
}

impl Theme {
    /// Normal text style.
    #[must_use]
    pub fn text(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Muted text.
    #[must_use]
    pub fn muted(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    /// Accent text.
    #[must_use]
    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }

    /// Link text style by resolved class.
    #[must_use]
    pub fn link_class(&self, class: wiki_reader_render::LinkClass) -> Style {
        let fg = match class {
            wiki_reader_render::LinkClass::Internal => self.link,
            wiki_reader_render::LinkClass::Broken => self.link_broken,
            wiki_reader_render::LinkClass::External => self.link_external,
            wiki_reader_render::LinkClass::Unsupported => self.link_unsupported,
        };
        Style::default().fg(fg)
    }

    /// Map renderer [`StyleKind`](wiki_reader_render::StyleKind) to a ratatui style.
    #[must_use]
    pub fn style_kind(&self, kind: wiki_reader_render::StyleKind) -> Style {
        use wiki_reader_render::StyleKind;
        match kind {
            StyleKind::Plain | StyleKind::Table => self.text(),
            StyleKind::TableHeader => Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD),
            StyleKind::Heading(level) => {
                let idx = usize::from(level.saturating_sub(1).min(5));
                Style::default()
                    .fg(self.heading[idx])
                    .add_modifier(Modifier::BOLD)
            }
            StyleKind::Emphasis => Style::default()
                .fg(self.text)
                .add_modifier(Modifier::ITALIC),
            StyleKind::Strong => Style::default().fg(self.text).add_modifier(Modifier::BOLD),
            StyleKind::Strikethrough => Style::default()
                .fg(self.text_muted)
                .add_modifier(Modifier::CROSSED_OUT),
            StyleKind::InlineCode | StyleKind::CodeBlock => {
                Style::default().fg(Color::Green).bg(self.code_bg)
            }
            StyleKind::CodeLang => Style::default()
                .fg(self.text_muted)
                .bg(self.code_bg)
                .add_modifier(Modifier::BOLD),
            StyleKind::Link => Style::default().fg(self.link),
            StyleKind::Quote => Style::default()
                .fg(Color::Rgb(210, 210, 220))
                .bg(self.quote_bar),
            StyleKind::Alert(id) => {
                let fg = self.alert[usize::from(id).min(self.alert.len() - 1)];
                Style::default().fg(fg).add_modifier(Modifier::BOLD)
            }
            StyleKind::Frontmatter => Style::default().fg(self.text_muted),
            StyleKind::Rule => Style::default().fg(self.border),
            StyleKind::ListMarker | StyleKind::TaskMarker => Style::default().fg(self.accent),
        }
    }

    /// Border for a pane; `focused` uses `border_focus`.
    #[must_use]
    pub fn border(&self, focused: bool) -> Style {
        if focused {
            Style::default()
                .fg(self.border_focus)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.border)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_reader_render::StyleKind;

    #[test]
    fn style_kind_heading_ramp_and_alerts() {
        let t = Theme::default();
        for level in 1..=6u8 {
            let st = t.style_kind(StyleKind::Heading(level));
            let idx = usize::from(level.saturating_sub(1));
            assert_eq!(st.fg, Some(t.heading[idx]));
            assert!(st.add_modifier.contains(Modifier::BOLD));
        }
        let note = t.style_kind(StyleKind::Alert(1));
        assert_eq!(note.fg, Some(t.alert[1]));
        let warn = t.style_kind(StyleKind::Alert(4));
        assert_eq!(warn.fg, Some(t.alert[4]));
    }

    #[test]
    fn style_kind_code_quote_table_header() {
        let t = Theme::default();
        let code = t.style_kind(StyleKind::CodeBlock);
        assert_eq!(code.bg, Some(t.code_bg));
        let quote = t.style_kind(StyleKind::Quote);
        assert_eq!(quote.bg, Some(t.quote_bar));
        let header = t.style_kind(StyleKind::TableHeader);
        assert_eq!(header.fg, Some(t.accent));
        assert!(header.add_modifier.contains(Modifier::BOLD));
    }
}
