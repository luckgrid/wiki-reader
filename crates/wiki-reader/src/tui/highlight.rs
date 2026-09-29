//! Syntect markdown highlighting for raw view (line gutter applied at draw).

use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Highlighted span for one run of text.
#[derive(Debug, Clone)]
pub struct HlSpan {
    /// Ratatui style.
    pub style: Style,
    /// Text fragment.
    pub text: String,
}

fn syntax_set() -> &'static SyntaxSet {
    static PS: OnceLock<SyntaxSet> = OnceLock::new();
    PS.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static TS: OnceLock<ThemeSet> = OnceLock::new();
    TS.get_or_init(ThemeSet::load_defaults)
}

fn markdown_syntax(ps: &SyntaxSet) -> Option<&SyntaxReference> {
    ps.find_syntax_by_token("md")
        .or_else(|| ps.find_syntax_by_name("Markdown"))
}

fn pick_theme(ts: &ThemeSet) -> Option<&Theme> {
    ts.themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
}

/// Highlight `source` as markdown. Falls back to plain lines on error.
#[must_use]
pub fn highlight_markdown(source: &str) -> Vec<Vec<HlSpan>> {
    let ps = syntax_set();
    let ts = theme_set();
    let Some(syntax) = markdown_syntax(ps) else {
        return plain(source);
    };
    let Some(theme) = pick_theme(ts) else {
        return plain(source);
    };
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    for line in LinesWithEndings::from(source) {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        match h.highlight_line(line, ps) {
            Ok(ranges) => {
                let spans: Vec<HlSpan> = ranges
                    .into_iter()
                    .map(|(sty, text)| HlSpan {
                        style: syn_to_ratatui(sty),
                        text: text.trim_end_matches(['\r', '\n']).to_owned(),
                    })
                    .collect();
                if spans.is_empty() {
                    out.push(vec![HlSpan {
                        style: Style::default(),
                        text: trimmed.to_owned(),
                    }]);
                } else {
                    out.push(spans);
                }
            }
            Err(_) => out.push(vec![HlSpan {
                style: Style::default(),
                text: trimmed.to_owned(),
            }]),
        }
    }
    if out.is_empty() {
        out.push(vec![HlSpan {
            style: Style::default(),
            text: String::new(),
        }]);
    }
    out
}

fn plain(source: &str) -> Vec<Vec<HlSpan>> {
    let mut lines: Vec<Vec<HlSpan>> = source
        .lines()
        .map(|l| {
            vec![HlSpan {
                style: Style::default(),
                text: l.to_owned(),
            }]
        })
        .collect();
    if lines.is_empty() {
        lines.push(vec![HlSpan {
            style: Style::default(),
            text: String::new(),
        }]);
    }
    lines
}

fn syn_to_ratatui(s: syntect::highlighting::Style) -> Style {
    let fg = s.foreground;
    let mut style = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    if s.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn highlight_timing_sanity() {
        // ~50 KB of markdown-ish text.
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let source = chunk.repeat(700); // ≈ 50 KB
        let source_len = source.len();
        assert!(source_len >= 40_000, "fixture too small: {source_len}");
        // Warm OnceLock first so the ceiling measures highlight work, not asset load.
        let _ = highlight_markdown("warm");
        let start = Instant::now();
        let lines = highlight_markdown(&source);
        let elapsed = start.elapsed();
        assert!(!lines.is_empty());
        // CI-safe generous ceiling (debug syntect is slow; manual budget < 20 ms release).
        assert!(
            elapsed.as_millis() < 30_000,
            "highlight took {elapsed:?}, expected < 30s"
        );
    }
}
