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
    /// Read-only text that should stay legible (current crumb, status bar).
    pub text_secondary: Color,
    /// Mouse-selection background.
    pub selection: Color,
    /// Collapsible folder rows in the nav (the herdr tab peach).
    pub nav_folder: Color,
    /// The one warm colour: selected button / tab fill, status pill, popup
    /// border and accents (sampled from herdr's tab).
    pub peach: Color,
    /// Dark text on [`peach`](Self::peach) fills.
    pub on_peach: Color,
    /// Accent (current page, icons).
    pub accent: Color,
    /// Cursor line highlight.
    pub cursor_line: Color,
    /// Nav search box background.
    pub search_box: Color,
    /// Active tab fill.
    pub tab_active: Color,
    /// Inactive tab fill.
    pub tab_inactive: Color,
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
    /// Frontmatter status: accepted / active / done (was heading[2]).
    pub status_ok: Color,
    /// Frontmatter status: draft / proposed / wip (was heading[1]).
    pub status_warn: Color,
    /// Frontmatter status: planned / todo / open (was heading[3]).
    pub status_plan: Color,
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
            text_secondary: Color::Rgb(160, 165, 175),
            selection: Color::Rgb(45, 95, 125),
            // Sampled from a herdr tab: #f6c99f.
            nav_folder: Color::Rgb(246, 201, 159),
            peach: Color::Rgb(246, 201, 159),
            on_peach: Color::Rgb(26, 26, 26),
            accent: Color::Cyan,
            // Stronger than near-black so the row reads on Reset surfaces (P3-07 owns presets).
            cursor_line: Color::Rgb(70, 75, 100),
            // ponytail: Reset surface means unknown terminal bg; DarkGray reads on both until P3-07.
            search_box: Color::DarkGray,
            tab_active: Color::Rgb(50, 60, 80),
            tab_inactive: Color::Rgb(35, 38, 48),
            link: Color::Cyan,
            link_broken: Color::Red,
            link_external: Color::Magenta,
            link_unsupported: Color::DarkGray,
            code_bg: Color::Rgb(30, 32, 36),
            quote_bar: Color::Rgb(80, 80, 100),
            heading: [
                Color::Cyan,               // H1 = accent
                Color::Rgb(246, 201, 159), // H2 = peach
                Color::Rgb(246, 201, 159), // H3 = peach, same as H2
                Color::Rgb(246, 201, 159), // H4 = peach, same as H2
                Color::Rgb(165, 170, 180), // H5
                Color::Rgb(140, 145, 155), // H6 muted
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
            // Kept off the heading ramp so recolouring H1–H6 does not shift status pills.
            status_ok: Color::Rgb(100, 200, 120),
            status_warn: Color::Rgb(255, 180, 60),
            status_plan: Color::Rgb(80, 180, 220),
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

    /// Secondary (read-only but legible) text.
    #[must_use]
    pub fn secondary(&self) -> Style {
        Style::default().fg(self.text_secondary)
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
            StyleKind::Plain | StyleKind::Table | StyleKind::BacklinkSummary => self.text(),
            // Bold text — TableHeader uses text (not accent/cyan) so it doesn't read as a link.
            StyleKind::TableHeader | StyleKind::Strong => {
                Style::default().fg(self.text).add_modifier(Modifier::BOLD)
            }
            StyleKind::Heading(level) => {
                let idx = usize::from(level.saturating_sub(1).min(5));
                Style::default()
                    .fg(self.heading[idx])
                    .add_modifier(Modifier::BOLD)
            }
            StyleKind::Emphasis => Style::default()
                .fg(self.text)
                .add_modifier(Modifier::ITALIC),
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
            StyleKind::Frontmatter | StyleKind::FrontmatterPunct => {
                Style::default().fg(self.text_muted)
            }
            StyleKind::FrontmatterValue | StyleKind::BacklinkTag => {
                Style::default().fg(self.text_secondary)
            }
            StyleKind::Rule | StyleKind::BacklinkBorder => Style::default().fg(self.border),
            StyleKind::FrontmatterKey | StyleKind::ListMarker | StyleKind::TaskMarker => {
                Style::default().fg(self.accent)
            }
        }
    }

    /// Colour a frontmatter `status:` value by meaning.
    #[must_use]
    pub fn status_style(&self, status: &str) -> Style {
        let fg = match status.to_ascii_lowercase().as_str() {
            "accepted" | "active" | "done" | "stable" | "approved" | "published" => self.status_ok,
            "draft" | "proposed" | "review" | "doing" | "wip" => self.status_warn,
            "planned" | "todo" | "open" => self.status_plan,
            "deferred" | "superseded" | "deprecated" | "rejected" | "archived" => self.text_muted,
            _ => self.text,
        };
        Style::default().fg(fg)
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
        assert_eq!(header.fg, Some(t.text), "header must not look like a link");
        assert!(header.add_modifier.contains(Modifier::BOLD));
    }
}
