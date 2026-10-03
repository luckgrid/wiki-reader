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

type Rgb = (u8, u8, u8);

/// A preset's base colours. [`Theme::from_palette`] derives every token from these, so a preset is
/// one table entry; surfaces are tints of `bg` toward `fg` or an accent, which keeps a palette's
/// pairs (text on a tint, a fill under dark text) readable by construction. The contrast tests
/// check each preset.
#[derive(Clone, Copy)]
struct Palette {
    /// The palette's own background; only used to mix tints, never painted.
    bg: Rgb,
    /// Body-ish text; mixed into tints and the diagram card.
    fg: Rgb,
    muted: Rgb,
    secondary: Rgb,
    /// Page title (H1) and the accent token.
    accent: Rgb,
    /// Fill colour: selected tab or button, status pill, popup border.
    warm: Rgb,
    /// Text on a `warm` fill.
    on_warm: Rgb,
    /// Focused pane border and bold accents; must read as text on the terminal.
    focus: Rgb,
    link: Rgb,
    /// H2–H4.
    h2: Rgb,
    /// Collapsible folder rows in the nav.
    folder: Rgb,
    red: Rgb,
    green: Rgb,
    orange: Rgb,
    blue: Rgb,
    purple: Rgb,
    teal: Rgb,
    syntax: &'static str,
    site: Site,
}

/// Values a preset takes straight from a design system rather than deriving, and whether it
/// paints the screen. [`Site::NONE`] derives everything and leaves the terminal's own colours.
#[derive(Clone, Copy)]
struct Site {
    /// Paint `bg` and `fg` over the whole screen instead of using the terminal's colours.
    paint: bool,
    /// Code blocks and the diagram card.
    panel: Option<Rgb>,
    /// Raised fills: nav search box, inactive tabs, quote bar, diagram nodes.
    raised: Option<Rgb>,
    /// Unfocused borders.
    stroke: Option<Rgb>,
}

impl Site {
    const NONE: Self = Self {
        paint: false,
        panel: None,
        raised: None,
        stroke: None,
    };
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| {
        let v = f32::from(x) + (f32::from(y) - f32::from(x)) * t;
        // The clamp keeps the cast in range.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            v.round().clamp(0.0, 255.0) as u8
        }
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

const fn col(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// luckgrid.net dark (`src/styles/theme.css` of that site, OKLCH mapped to sRGB the way a browser
/// does it): black and white, the pale lime `--color-accent` as the fill, its vivid
/// `--color-accent-stroke` for text and borders, and the site's terminal syntax hues.
const LUCKGRID_DARK: Palette = Palette {
    bg: (0, 0, 0),
    fg: (255, 255, 255),
    muted: (122, 122, 122),
    secondary: (209, 209, 209),
    accent: (170, 247, 0),
    warm: (219, 255, 164),
    on_warm: (0, 0, 0),
    focus: (219, 255, 164),
    link: (58, 199, 255),
    h2: (252, 159, 48),
    folder: (252, 159, 48),
    red: (237, 75, 67),
    green: (128, 219, 162),
    orange: (252, 159, 48),
    blue: (58, 199, 255),
    purple: (184, 158, 255),
    teal: (67, 213, 220),
    syntax: "base16-ocean.dark",
    site: Site {
        paint: true,
        panel: Some((9, 9, 9)),
        raised: Some((36, 36, 36)),
        stroke: Some((122, 122, 122)),
    },
};

/// luckgrid.net light: white and black, the cyan `--color-accent` as the fill. Text colours are
/// the site's hues darkened until they read on white (>= 4.5:1); the site's own stroke
/// (0, 147, 188) is 3.7:1 and is too light for text.
const LUCKGRID_LIGHT: Palette = Palette {
    bg: (255, 255, 255),
    fg: (0, 0, 0),
    muted: (96, 96, 96),
    secondary: (70, 70, 70),
    accent: (0, 102, 153),
    warm: (0, 201, 254),
    on_warm: (0, 0, 0),
    focus: (0, 124, 154),
    link: (0, 120, 170),
    h2: (160, 94, 0),
    folder: (160, 94, 0),
    red: (179, 0, 13),
    green: (10, 118, 64),
    orange: (160, 94, 0),
    blue: (0, 120, 170),
    purple: (122, 90, 196),
    teal: (0, 124, 129),
    syntax: "InspiredGitHub",
    site: Site {
        paint: true,
        panel: Some((245, 245, 245)),
        raised: Some((235, 235, 235)),
        stroke: Some((152, 152, 152)),
    },
};

impl Theme {
    /// Preset for a configured name, with herdr's default theme (vesper) for `herdr`. The app
    /// passes the theme read from herdr's config through [`Theme::resolve`] instead.
    #[cfg(test)]
    #[must_use]
    pub fn from_name(name: ThemeName) -> Self {
        Self::resolve(name, "vesper")
    }

    /// Preset for a configured name; `herdr_theme` is the theme name from herdr's config, used
    /// when `name` is [`ThemeName::Herdr`].
    #[must_use]
    pub fn resolve(name: ThemeName, herdr_theme: &str) -> Self {
        match name {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::Herdr => Self::herdr(herdr_theme).0,
        }
    }

    /// The preset to start with: the configured one, except that inside herdr, with no config
    /// file naming a theme, the default is `herdr` so the reader matches its host.
    #[must_use]
    pub fn effective_name(
        configured: ThemeName,
        config_sets_theme: bool,
        in_herdr: bool,
    ) -> ThemeName {
        if in_herdr && !config_sets_theme {
            ThemeName::Herdr
        } else {
            configured
        }
    }

    /// Style for the whole screen and every popup: the painted background and text colour, or
    /// the terminal's own when a preset does not paint.
    #[must_use]
    pub fn base_style(&self) -> Style {
        Style::default().bg(self.surface).fg(self.text)
    }

    /// luckgrid.net dark; the default.
    #[must_use]
    pub fn dark() -> Self {
        Self::from_palette(&LUCKGRID_DARK)
    }

    /// luckgrid.net light: every text colour reads on a white terminal.
    #[must_use]
    pub fn light() -> Self {
        Self::from_palette(&LUCKGRID_LIGHT)
    }

    /// The preset for one of herdr's `[theme] name` values, and whether the name was recognised
    /// (an unknown name gets vesper). Palettes follow each theme's published colours; herdr's own
    /// values are not exposed to other programs.
    #[must_use]
    pub fn herdr(herdr_theme: &str) -> (Self, bool) {
        let name = herdr_theme.trim().to_ascii_lowercase();
        if name == "terminal" {
            return (Self::ansi(), true);
        }
        let known = herdr_palette(&name);
        (
            Self::from_palette(&known.unwrap_or(HERDR_VESPER)),
            known.is_some(),
        )
    }

    fn from_palette(p: &Palette) -> Self {
        let tint = |c: Rgb, t: f32| mix(p.bg, c, t);
        let neutral = |t: f32| tint(p.fg, t);
        let risk = mix(p.red, p.purple, 0.5);
        let panel = p.site.panel.unwrap_or_else(|| neutral(0.06));
        let raised = p.site.raised.unwrap_or_else(|| neutral(0.10));
        let (surface, text) = if p.site.paint {
            (col(p.bg), col(p.fg))
        } else {
            (Color::Reset, Color::Reset)
        };
        Self {
            surface,
            surface_muted: col(raised),
            border: col(p.site.stroke.unwrap_or_else(|| neutral(0.30))),
            border_focus: col(p.focus),
            text,
            text_muted: col(p.muted),
            text_secondary: col(p.secondary),
            selection: col(tint(p.warm, 0.30)),
            nav_folder: col(p.folder),
            peach: col(p.warm),
            on_peach: col(p.on_warm),
            accent: col(p.accent),
            cursor_line: col(tint(p.warm, 0.16)),
            search_box: col(raised),
            tab_active: col(tint(p.warm, 0.22)),
            tab_inactive: col(raised),
            link: col(p.link),
            link_broken: col(p.red),
            link_external: col(p.purple),
            link_unsupported: col(p.muted),
            code_bg: col(panel),
            code_fg: col(p.green),
            quote_text: col(p.fg),
            quote_bar: col(raised),
            heading: [
                col(p.accent),
                col(p.h2),
                col(p.h2),
                col(p.h2),
                col(p.secondary),
                col(p.muted),
            ],
            alert: [
                col(p.muted),
                col(p.blue),   // NOTE
                col(p.green),  // TIP
                col(p.purple), // IMPORTANT
                col(p.orange), // WARNING
                col(p.red),    // CAUTION
                col(p.teal),   // GOAL
                col(p.link),   // DECISION
                col(risk),     // RISK
            ],
            status_ok: col(p.green),
            status_warn: col(p.orange),
            status_plan: col(p.blue),
            syntax: p.syntax,
            diagram: DiagramPalette {
                bg: panel,
                text: tint(p.fg, 0.95),
                line: p.secondary,
                node_fill: raised,
                node_border: p.warm,
                cluster_fill: mix(panel, raised, 0.5),
                cluster_border: neutral(0.30),
                note_fill: tint(p.warm, 0.12),
                note_border: p.warm,
            },
        }
    }

    /// The terminal's own ANSI colours (herdr's `terminal` theme): no fixed palette to match.
    #[must_use]
    pub fn ansi() -> Self {
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
}

/// herdr's default theme: peach and mint on near-black.
const HERDR_VESPER: Palette = Palette {
    bg: (16, 16, 16),
    fg: (230, 230, 230),
    muted: (140, 140, 140),
    secondary: (160, 160, 160),
    accent: (153, 255, 228),
    warm: (255, 199, 153),
    on_warm: (16, 16, 16),
    focus: (255, 199, 153),
    link: (153, 255, 228),
    h2: (255, 199, 153),
    folder: (255, 199, 153),
    red: (255, 128, 128),
    green: (130, 230, 160),
    orange: (255, 170, 100),
    blue: (130, 190, 255),
    purple: (180, 160, 255),
    teal: (120, 220, 200),
    syntax: "base16-mocha.dark",
    site: Site::NONE,
};

/// Palette for a lower-case herdr theme name, if wiki-reader knows it.
#[allow(clippy::too_many_lines)] // a table of palettes
fn herdr_palette(name: &str) -> Option<Palette> {
    Some(match name {
        "vesper" => HERDR_VESPER,
        "catppuccin" => Palette {
            bg: (30, 30, 46),
            fg: (205, 214, 244),
            muted: (127, 132, 156),
            secondary: (166, 173, 200),
            accent: (203, 166, 247),
            warm: (203, 166, 247),
            on_warm: (17, 17, 27),
            focus: (203, 166, 247),
            link: (137, 180, 250),
            h2: (250, 179, 135),
            folder: (250, 179, 135),
            red: (243, 139, 168),
            green: (166, 227, 161),
            orange: (250, 179, 135),
            blue: (137, 180, 250),
            purple: (203, 166, 247),
            teal: (148, 226, 213),
            syntax: "base16-mocha.dark",
            site: Site::NONE,
        },
        "catppuccin-latte" => Palette {
            bg: (239, 241, 245),
            fg: (76, 79, 105),
            muted: (108, 111, 133),
            secondary: (92, 95, 119),
            accent: (136, 57, 239),
            warm: (136, 57, 239),
            on_warm: (255, 255, 255),
            focus: (136, 57, 239),
            link: (30, 102, 245),
            h2: (176, 66, 0),
            folder: (176, 66, 0),
            red: (210, 15, 57),
            green: (28, 120, 18),
            orange: (176, 66, 0),
            blue: (30, 102, 245),
            purple: (136, 57, 239),
            teal: (14, 110, 116),
            syntax: "InspiredGitHub",
            site: Site::NONE,
        },
        "tokyo-night" => Palette {
            bg: (26, 27, 38),
            fg: (192, 202, 245),
            muted: (121, 130, 169),
            secondary: (169, 177, 214),
            accent: (122, 162, 247),
            warm: (122, 162, 247),
            on_warm: (26, 27, 38),
            focus: (122, 162, 247),
            link: (125, 207, 255),
            h2: (255, 158, 100),
            folder: (255, 158, 100),
            red: (247, 118, 142),
            green: (158, 206, 106),
            orange: (255, 158, 100),
            blue: (122, 162, 247),
            purple: (187, 154, 247),
            teal: (115, 218, 202),
            syntax: "base16-ocean.dark",
            site: Site::NONE,
        },
        "tokyo-night-day" => Palette {
            bg: (225, 226, 231),
            fg: (52, 59, 88),
            muted: (96, 100, 130),
            secondary: (72, 80, 120),
            accent: (36, 96, 200),
            warm: (36, 98, 212),
            on_warm: (255, 255, 255),
            focus: (36, 96, 200),
            link: (14, 110, 160),
            h2: (150, 80, 0),
            folder: (150, 80, 0),
            red: (180, 40, 70),
            green: (40, 96, 10),
            orange: (150, 80, 0),
            blue: (36, 96, 200),
            purple: (110, 70, 190),
            teal: (0, 110, 110),
            syntax: "InspiredGitHub",
            site: Site::NONE,
        },
        "gruvbox" => Palette {
            bg: (40, 40, 40),
            fg: (235, 219, 178),
            muted: (146, 131, 116),
            secondary: (168, 153, 132),
            accent: (250, 189, 47),
            warm: (250, 189, 47),
            on_warm: (40, 40, 40),
            focus: (250, 189, 47),
            link: (131, 165, 152),
            h2: (254, 128, 25),
            folder: (254, 128, 25),
            red: (251, 73, 52),
            green: (184, 187, 38),
            orange: (254, 128, 25),
            blue: (131, 165, 152),
            purple: (211, 134, 155),
            teal: (142, 192, 124),
            syntax: "base16-ocean.dark",
            site: Site::NONE,
        },
        "gruvbox-light" => Palette {
            bg: (251, 241, 199),
            fg: (60, 56, 54),
            muted: (110, 98, 88),
            secondary: (80, 73, 69),
            accent: (7, 102, 120),
            warm: (250, 189, 47),
            on_warm: (40, 40, 40),
            focus: (7, 102, 120),
            link: (7, 102, 120),
            h2: (175, 58, 3),
            folder: (175, 58, 3),
            red: (157, 0, 6),
            green: (90, 86, 8),
            orange: (175, 58, 3),
            blue: (7, 102, 120),
            purple: (143, 63, 113),
            teal: (50, 100, 66),
            syntax: "InspiredGitHub",
            site: Site::NONE,
        },
        "one-dark" => Palette {
            bg: (40, 44, 52),
            fg: (171, 178, 191),
            muted: (130, 137, 151),
            secondary: (160, 167, 181),
            accent: (97, 175, 239),
            warm: (97, 175, 239),
            on_warm: (40, 44, 52),
            focus: (97, 175, 239),
            link: (86, 182, 194),
            h2: (209, 154, 102),
            folder: (209, 154, 102),
            red: (224, 108, 117),
            green: (152, 195, 121),
            orange: (209, 154, 102),
            blue: (97, 175, 239),
            purple: (198, 120, 221),
            teal: (86, 182, 194),
            syntax: "base16-ocean.dark",
            site: Site::NONE,
        },
        "kanagawa" => Palette {
            bg: (31, 31, 40),
            fg: (220, 215, 186),
            muted: (138, 137, 128),
            secondary: (170, 168, 150),
            accent: (126, 156, 216),
            warm: (255, 160, 102),
            on_warm: (31, 31, 40),
            focus: (255, 160, 102),
            link: (127, 180, 202),
            h2: (255, 160, 102),
            folder: (255, 160, 102),
            red: (228, 104, 118),
            green: (152, 187, 108),
            orange: (255, 160, 102),
            blue: (126, 156, 216),
            purple: (149, 127, 184),
            teal: (122, 168, 159),
            syntax: "base16-mocha.dark",
            site: Site::NONE,
        },
        _ => return None,
    })
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

    /// Darkened accent for the active tab while focus is elsewhere.
    /// White text remains legible in both dark and light presets.
    #[must_use]
    pub fn tab_unfocused(&self) -> Color {
        match self.peach {
            Color::Rgb(r, g, b) => Color::Rgb(r / 3, g / 3, b / 3),
            _ => self.tab_inactive,
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

    #[test]
    fn unfocused_tab_selection_is_darker_and_white_text_is_readable() {
        for theme in [Theme::dark(), Theme::light(), Theme::herdr("vesper").0] {
            let warm = rgb(theme.peach).unwrap();
            let dim = rgb(theme.tab_unfocused()).unwrap();
            assert!(luminance(dim.0, dim.1, dim.2) < luminance(warm.0, warm.1, warm.2));
            assert!(contrast((255, 255, 255), dim) >= 4.5);
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

    /// Herdr themes that draw on a light terminal background.
    const HERDR_LIGHT: [&str; 3] = ["catppuccin-latte", "tokyo-night-day", "gruvbox-light"];
    const HERDR_DARK: [&str; 6] = [
        "vesper",
        "catppuccin",
        "tokyo-night",
        "gruvbox",
        "one-dark",
        "kanagawa",
    ];

    #[test]
    fn dark_presets_are_readable_on_a_dark_terminal() {
        // luckgrid paints its own black; the herdr presets sit on whatever the terminal has.
        assert_readable("dark", &Theme::dark(), (0, 0, 0));
        for name in HERDR_DARK {
            let (t, known) = Theme::herdr(name);
            assert!(known, "{name}");
            assert_readable(name, &t, (16, 16, 16));
        }
    }

    #[test]
    fn light_presets_are_readable_on_a_light_terminal() {
        assert_readable("light", &Theme::light(), (255, 255, 255));
        for name in HERDR_LIGHT {
            let (t, known) = Theme::herdr(name);
            assert!(known, "{name}");
            assert_readable(name, &t, (255, 255, 255));
        }
    }

    #[test]
    fn default_is_the_dark_preset() {
        let (d, dark) = (Theme::default(), Theme::dark());
        assert_eq!(d.heading, dark.heading);
        assert_eq!(d.code_bg, dark.code_bg);
        assert_eq!(d.syntax, "base16-ocean.dark");
        assert_eq!(
            rgb(d.code_bg),
            Some(d.diagram.bg),
            "card matches the code background"
        );
    }

    #[test]
    fn dark_and_light_use_the_luckgrid_accents() {
        // luckgrid.net (theme.css, OKLCH mapped to sRGB like a browser): the pale lime accent is
        // the fill and its vivid stroke is the text accent; light's fill is the bright cyan.
        assert_eq!(Theme::dark().peach, Color::Rgb(219, 255, 164));
        assert_eq!(Theme::dark().accent, Color::Rgb(170, 247, 0));
        assert_ne!(Theme::dark().accent, Theme::dark().border_focus);
        assert_eq!(Theme::light().peach, Color::Rgb(0, 201, 254));
        assert_ne!(Theme::light().accent, Theme::light().border_focus);
    }

    #[test]
    fn luckgrid_presets_paint_the_screen_and_the_others_do_not() {
        let dark = Theme::dark();
        assert_eq!(
            (dark.surface, dark.text),
            (Color::Rgb(0, 0, 0), Color::Rgb(255, 255, 255))
        );
        let light = Theme::light();
        assert_eq!(
            (light.surface, light.text),
            (Color::Rgb(255, 255, 255), Color::Rgb(0, 0, 0))
        );
        // The site's panel and raised neutrals.
        assert_eq!(dark.code_bg, Color::Rgb(9, 9, 9));
        assert_eq!(dark.search_box, Color::Rgb(36, 36, 36));
        assert_eq!(light.code_bg, Color::Rgb(245, 245, 245));
        for t in [
            Theme::herdr("vesper").0,
            Theme::herdr("gruvbox").0,
            Theme::ansi(),
        ] {
            assert_eq!((t.surface, t.text), (Color::Reset, Color::Reset));
            assert_eq!(t.base_style().bg, Some(Color::Reset));
        }
    }

    #[test]
    fn presets_are_selected_by_name_and_differ() {
        let dark = Theme::from_name(ThemeName::Dark);
        let light = Theme::from_name(ThemeName::Light);
        let herdr = Theme::from_name(ThemeName::Herdr);
        assert_ne!(dark.diagram, light.diagram);
        assert_ne!(dark.diagram, herdr.diagram);
        assert_ne!(dark.link, light.link);
        // The dark preset no longer shares herdr's peach and mint.
        assert_ne!(dark.peach, herdr.peach);
        assert_ne!(dark.accent, herdr.accent);
        assert_eq!(light.syntax, "InspiredGitHub");
    }

    #[test]
    fn inside_herdr_the_default_is_herdr_unless_config_chose() {
        use ThemeName::{Dark, Herdr, Light};
        assert_eq!(Theme::effective_name(Dark, false, true), Herdr);
        assert_eq!(Theme::effective_name(Dark, false, false), Dark);
        // An explicit choice wins, even `dark` written by the options window.
        assert_eq!(Theme::effective_name(Dark, true, true), Dark);
        assert_eq!(Theme::effective_name(Light, true, true), Light);
    }

    #[test]
    fn herdr_follows_the_named_theme() {
        let (vesper, known) = Theme::herdr("vesper");
        assert!(known);
        assert_eq!(vesper.peach, Color::Rgb(255, 199, 153));
        let (gruv, known) = Theme::herdr(" Gruvbox ");
        assert!(known, "names are trimmed and case-insensitive");
        assert_ne!(gruv.peach, vesper.peach);
        // An unknown name is vesper, flagged so the app can say so.
        let (other, known) = Theme::herdr("nonesuch");
        assert!(!known);
        assert_eq!(other.peach, vesper.peach);
        // `terminal` is the host's ANSI palette.
        let (term, known) = Theme::herdr("terminal");
        assert!(known);
        assert_eq!(term.accent, Color::Cyan);
        assert_eq!(
            Theme::resolve(ThemeName::Herdr, "catppuccin").peach,
            Theme::herdr("catppuccin").0.peach
        );
    }

    #[test]
    fn syntax_themes_exist_in_the_bundled_set() {
        let set = syntect::highlighting::ThemeSet::load_defaults();
        let mut all = vec![Theme::dark(), Theme::light(), Theme::ansi()];
        all.extend(
            HERDR_DARK
                .iter()
                .chain(&HERDR_LIGHT)
                .map(|n| Theme::herdr(n).0),
        );
        for t in all {
            assert!(set.themes.contains_key(t.syntax), "{}", t.syntax);
        }
    }
}
