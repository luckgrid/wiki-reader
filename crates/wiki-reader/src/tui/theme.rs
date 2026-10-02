//! Semantic color tokens and the built-in presets (ui-spec Theming).

use ratatui::style::{Color, Modifier, Style};
use wiki_reader_core::config::ThemeName;
use wiki_reader_render::DiagramPalette;

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
    /// Code block / inline code text.
    pub code_fg: Color,
    /// Quote body text on [`quote_bar`](Self::quote_bar).
    pub quote_text: Color,
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
    /// Syntect theme for the raw view (a name in `ThemeSet::load_defaults`).
    pub syntax: &'static str,
    /// Colours Mermaid diagrams are drawn with.
    pub diagram: DiagramPalette,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Preset for a configured name.
    #[must_use]
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::Herdr => Self::herdr(),
        }
    }

    /// Tuned for dark terminals; the default.
    #[must_use]
    pub fn dark() -> Self {
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
            code_fg: Color::Green,
            quote_text: Color::Rgb(210, 210, 220),
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
            syntax: "base16-ocean.dark",
            diagram: DiagramPalette::default(),
        }
    }

    /// Tuned for light terminals: surface and body text stay the terminal's own colours; every
    /// accent is dark enough to read on white.
    #[must_use]
    pub fn light() -> Self {
        Self {
            surface: Color::Reset,
            surface_muted: Color::Rgb(235, 237, 241),
            border: Color::Rgb(160, 165, 175),
            border_focus: Color::Rgb(0, 110, 150),
            text: Color::Reset,
            text_muted: Color::Rgb(100, 106, 118),
            text_secondary: Color::Rgb(80, 86, 98),
            selection: Color::Rgb(176, 206, 238),
            nav_folder: Color::Rgb(150, 84, 16),
            // A fill (dark text on top), so it can stay the herdr tab peach.
            peach: Color::Rgb(246, 201, 159),
            on_peach: Color::Rgb(26, 26, 26),
            accent: Color::Rgb(0, 110, 140),
            cursor_line: Color::Rgb(222, 228, 244),
            search_box: Color::Rgb(230, 233, 240),
            tab_active: Color::Rgb(196, 214, 238),
            tab_inactive: Color::Rgb(230, 233, 240),
            link: Color::Rgb(0, 100, 130),
            link_broken: Color::Rgb(180, 28, 28),
            link_external: Color::Rgb(140, 40, 150),
            link_unsupported: Color::Rgb(110, 116, 128),
            code_bg: Color::Rgb(238, 240, 245),
            code_fg: Color::Rgb(24, 104, 48),
            quote_text: Color::Rgb(44, 50, 66),
            quote_bar: Color::Rgb(226, 230, 240),
            heading: [
                Color::Rgb(0, 100, 130),
                Color::Rgb(150, 84, 16),
                Color::Rgb(150, 84, 16),
                Color::Rgb(150, 84, 16),
                Color::Rgb(80, 86, 98),
                Color::Rgb(100, 106, 118),
            ],
            alert: [
                Color::Rgb(100, 106, 118), // 0 unused
                Color::Rgb(0, 92, 170),    // NOTE
                Color::Rgb(16, 120, 70),   // TIP
                Color::Rgb(150, 100, 0),   // IMPORTANT
                Color::Rgb(176, 80, 0),    // WARNING
                Color::Rgb(190, 30, 30),   // CAUTION
                Color::Rgb(0, 115, 100),   // GOAL
                Color::Rgb(70, 80, 170),   // DECISION
                Color::Rgb(190, 30, 70),   // RISK
            ],
            status_ok: Color::Rgb(24, 120, 52),
            status_warn: Color::Rgb(160, 88, 0),
            status_plan: Color::Rgb(0, 100, 150),
            syntax: "InspiredGitHub",
            diagram: DiagramPalette {
                bg: (246, 247, 250),
                text: (24, 28, 38),
                line: (90, 98, 112),
                node_fill: (228, 233, 243),
                node_border: (120, 130, 150),
                cluster_fill: (238, 241, 248),
                cluster_border: (190, 196, 210),
                note_fill: (255, 247, 237),
                note_border: (240, 170, 100),
            },
        }
    }

    /// Dark, after herdr's vesper theme: peach and mint accents on near-black.
    #[must_use]
    pub fn herdr() -> Self {
        const PEACH: Color = Color::Rgb(255, 199, 153);
        const MINT: Color = Color::Rgb(153, 255, 228);
        Self {
            surface: Color::Reset,
            surface_muted: Color::Rgb(28, 28, 28),
            border: Color::Rgb(80, 80, 80),
            border_focus: PEACH,
            text: Color::Reset,
            text_muted: Color::Rgb(140, 140, 140),
            text_secondary: Color::Rgb(160, 160, 160),
            selection: Color::Rgb(64, 56, 46),
            nav_folder: PEACH,
            peach: PEACH,
            on_peach: Color::Rgb(16, 16, 16),
            accent: MINT,
            cursor_line: Color::Rgb(40, 40, 40),
            search_box: Color::Rgb(28, 28, 28),
            tab_active: Color::Rgb(52, 46, 40),
            tab_inactive: Color::Rgb(28, 28, 28),
            link: MINT,
            link_broken: Color::Rgb(255, 128, 128),
            link_external: Color::Rgb(180, 150, 255),
            link_unsupported: Color::Rgb(130, 130, 130),
            code_bg: Color::Rgb(24, 24, 24),
            code_fg: Color::Rgb(130, 220, 180),
            quote_text: Color::Rgb(200, 200, 200),
            quote_bar: Color::Rgb(36, 36, 36),
            heading: [
                MINT,
                PEACH,
                PEACH,
                PEACH,
                Color::Rgb(160, 160, 160),
                Color::Rgb(130, 130, 130),
            ],
            alert: [
                Color::Rgb(120, 120, 120), // 0 unused
                Color::Rgb(130, 190, 255), // NOTE
                MINT,                      // TIP
                PEACH,                     // IMPORTANT
                Color::Rgb(255, 170, 100), // WARNING
                Color::Rgb(255, 128, 128), // CAUTION
                Color::Rgb(120, 220, 200), // GOAL
                Color::Rgb(180, 160, 255), // DECISION
                Color::Rgb(255, 128, 150), // RISK
            ],
            status_ok: Color::Rgb(130, 230, 160),
            status_warn: PEACH,
            status_plan: Color::Rgb(130, 190, 255),
            syntax: "base16-mocha.dark",
            diagram: DiagramPalette {
                bg: (24, 24, 24),
                text: (240, 240, 240),
                line: (160, 160, 160),
                node_fill: (38, 38, 38),
                node_border: (255, 199, 153),
                cluster_fill: (30, 30, 30),
                cluster_border: (80, 80, 80),
                note_fill: (52, 46, 40),
                note_border: (255, 199, 153),
            },
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
            StyleKind::Plain
            | StyleKind::Table
            | StyleKind::BacklinkSummary
            | StyleKind::ImageSlot => self.text(),
            StyleKind::ImagePlaceholder => Style::default()
                .fg(self.text_muted)
                .add_modifier(Modifier::ITALIC),
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
                Style::default().fg(self.code_fg).bg(self.code_bg)
            }
            StyleKind::CodeLang => Style::default()
                .fg(self.text_muted)
                .bg(self.code_bg)
                .add_modifier(Modifier::BOLD),
            StyleKind::Link => Style::default().fg(self.link),
            StyleKind::Quote => Style::default().fg(self.quote_text).bg(self.quote_bar),
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

    /// WCAG relative luminance of an RGB colour.
    fn luminance(r: u8, g: u8, b: u8) -> f64 {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
        let (la, lb) = (luminance(a.0, a.1, a.2), luminance(b.0, b.1, b.2));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    fn rgb(c: Color) -> Option<(u8, u8, u8)> {
        match c {
            Color::Rgb(r, g, b) => Some((r, g, b)),
            _ => None,
        }
    }

    /// Foreground tokens drawn straight on the terminal background.
    fn text_tokens(t: &Theme) -> Vec<(&'static str, Color)> {
        let mut v = vec![
            ("text_muted", t.text_muted),
            ("text_secondary", t.text_secondary),
            ("accent", t.accent),
            ("nav_folder", t.nav_folder),
            ("border_focus", t.border_focus),
            ("link", t.link),
            ("link_broken", t.link_broken),
            ("link_external", t.link_external),
            ("link_unsupported", t.link_unsupported),
            ("status_ok", t.status_ok),
            ("status_warn", t.status_warn),
            ("status_plan", t.status_plan),
        ];
        v.extend(t.heading.iter().map(|c| ("heading", *c)));
        v.extend(t.alert.iter().skip(1).map(|c| ("alert", *c)));
        v
    }

    fn assert_readable(name: &str, t: &Theme, terminal_bg: (u8, u8, u8)) {
        for (token, color) in text_tokens(t) {
            if let Some(fg) = rgb(color) {
                let ratio = contrast(fg, terminal_bg);
                assert!(
                    ratio >= 4.5,
                    "{name}.{token} {fg:?} on {terminal_bg:?} = {ratio:.2}"
                );
            }
        }
        let pairs = [
            ("code_fg on code_bg", t.code_fg, t.code_bg),
            ("quote_text on quote_bar", t.quote_text, t.quote_bar),
            ("on_peach on peach", t.on_peach, t.peach),
        ];
        for (label, fg, bg) in pairs {
            if let (Some(fg), Some(bg)) = (rgb(fg), rgb(bg)) {
                let ratio = contrast(fg, bg);
                assert!(ratio >= 4.5, "{name}: {label} = {ratio:.2}");
            }
        }
        // The diagram card is a readable picture on its own.
        let d = t.diagram;
        for (label, fg) in [("text", d.text), ("line", d.line)] {
            assert!(contrast(fg, d.bg) >= 3.0, "{name}: diagram {label}");
        }
        for (label, fill) in [
            ("node", d.node_fill),
            ("cluster", d.cluster_fill),
            ("note", d.note_fill),
        ] {
            assert!(
                contrast(d.text, fill) >= 4.5,
                "{name}: diagram {label} text"
            );
        }
    }

    #[test]
    fn dark_presets_are_readable_on_a_dark_terminal() {
        assert_readable("dark", &Theme::dark(), (16, 16, 16));
        assert_readable("herdr", &Theme::herdr(), (16, 16, 16));
    }

    #[test]
    fn light_preset_is_readable_on_a_light_terminal() {
        assert_readable("light", &Theme::light(), (255, 255, 255));
    }

    #[test]
    fn default_is_the_dark_preset() {
        let (d, dark) = (Theme::default(), Theme::dark());
        assert_eq!(d.heading, dark.heading);
        assert_eq!(d.code_bg, dark.code_bg);
        assert_eq!(d.syntax, "base16-ocean.dark");
        assert_eq!(d.diagram, DiagramPalette::default());
        assert_eq!(
            rgb(d.code_bg),
            Some(d.diagram.bg),
            "card matches the code background"
        );
    }

    #[test]
    fn presets_are_selected_by_name_and_differ() {
        let dark = Theme::from_name(ThemeName::Dark);
        let light = Theme::from_name(ThemeName::Light);
        let herdr = Theme::from_name(ThemeName::Herdr);
        assert_ne!(dark.diagram, light.diagram);
        assert_ne!(dark.diagram, herdr.diagram);
        assert_ne!(dark.link, light.link);
        assert_eq!(light.syntax, "InspiredGitHub");
    }

    #[test]
    fn syntax_themes_exist_in_the_bundled_set() {
        let set = syntect::highlighting::ThemeSet::load_defaults();
        for t in [Theme::dark(), Theme::light(), Theme::herdr()] {
            assert!(set.themes.contains_key(t.syntax), "{}", t.syntax);
        }
    }
}
