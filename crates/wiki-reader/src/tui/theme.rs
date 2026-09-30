//! Minimal semantic color tokens (ui-spec Theming).

use ratatui::style::{Color, Modifier, Style};

/// Theme tokens used by the shell.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // tokens reserved for later regions
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
    #[allow(dead_code)] // used when viewer paints StyledLine (R13)
    pub fn style_kind(&self, kind: wiki_reader_render::StyleKind) -> Style {
        use wiki_reader_render::StyleKind;
        match kind {
            StyleKind::Plain | StyleKind::Table => self.text(),
            StyleKind::Heading(_) => Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD),
            StyleKind::Emphasis => Style::default()
                .fg(self.text)
                .add_modifier(Modifier::ITALIC),
            StyleKind::Strong => Style::default().fg(self.text).add_modifier(Modifier::BOLD),
            StyleKind::Strikethrough => Style::default()
                .fg(self.text_muted)
                .add_modifier(Modifier::CROSSED_OUT),
            StyleKind::InlineCode | StyleKind::CodeBlock => Style::default().fg(Color::Green),
            StyleKind::Link => Style::default().fg(self.link),
            StyleKind::Quote | StyleKind::Frontmatter => Style::default().fg(self.text_muted),
            StyleKind::Rule => Style::default().fg(self.border),
            StyleKind::TaskMarker => Style::default().fg(self.accent),
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
