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
