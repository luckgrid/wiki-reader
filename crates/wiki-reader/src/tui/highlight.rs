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
    fn highlight_scaling_not_quadratic() {
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let small = chunk.repeat(350);
        let large = chunk.repeat(700);
        let _ = highlight_markdown("warm");
        let t_small = {
            let start = Instant::now();
            assert!(!highlight_markdown(&small).is_empty());
            start.elapsed()
        };
        let t_large = {
            let start = Instant::now();
            assert!(!highlight_markdown(&large).is_empty());
            start.elapsed()
        };
        let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 3.0,
            "doubling input slowed highlight {ratio:.2}× (small={t_small:?}, large={t_large:?})"
        );
    }

    /// Manual / release budget: `cargo test -p wiki-reader --release -- highlight_budget --ignored`
    #[test]
    #[ignore = "release budget; run with --ignored --release"]
    fn highlight_budget_50kb_under_20ms() {
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let source = chunk.repeat(700);
        assert!(source.len() >= 40_000);
        let _ = highlight_markdown("warm");
        let start = Instant::now();
        let lines = highlight_markdown(&source);
        let elapsed = start.elapsed();
        assert!(!lines.is_empty());
        assert!(
            elapsed.as_millis() < 20,
            "highlight took {elapsed:?}, expected < 20ms (release)"
        );
    }
}
