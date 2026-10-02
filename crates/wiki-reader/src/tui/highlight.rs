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

/// The preset's syntect theme, or any bundled one if the name is unknown.
fn pick_theme<'a>(ts: &'a ThemeSet, name: &str) -> Option<&'a Theme> {
    ts.themes.get(name).or_else(|| ts.themes.values().next())
}

/// Highlight `source` as markdown. Falls back to plain lines on error.
///
/// Call off the UI thread — full-page syntect is ~370 ms / 50 KB (release) and
/// applies asynchronously; the UI paints plain text until the worker result lands.
#[must_use]
pub fn highlight_markdown(source: &str, theme_name: &str) -> Vec<Vec<HlSpan>> {
    let ps = syntax_set();
    let ts = theme_set();
    let Some(syntax) = markdown_syntax(ps) else {
        return plain(source);
    };
    let Some(theme) = pick_theme(ts, theme_name) else {
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
        let _ = highlight_markdown("warm", "base16-ocean.dark");
        let t_small = {
            let start = Instant::now();
            assert!(!highlight_markdown(&small, "base16-ocean.dark").is_empty());
            start.elapsed()
        };
        let t_large = {
            let start = Instant::now();
            assert!(!highlight_markdown(&large, "base16-ocean.dark").is_empty());
            start.elapsed()
        };
        let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 3.0,
            "doubling input slowed highlight {ratio:.2}× (small={t_small:?}, large={t_large:?})"
        );
    }

    /// UI path: `RawDoc` for ~50 KB stays well under syntect cost (no sync highlight).
    /// `cargo test -p wiki-reader --release -- raw_load_budget --ignored`
    #[test]
    #[ignore = "release budget; run with --ignored --release"]
    fn raw_load_budget_50kb_under_50ms() {
        use crate::tui::viewer_doc::{RawDoc, ViewerDoc};
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let source = chunk.repeat(700);
        assert!(source.len() >= 40_000);
        // Warm parse / allocator; highlight must stay unused on this path.
        let _ = RawDoc::from_source(chunk, None);
        let start = Instant::now();
        let doc = RawDoc::from_source(&source, None);
        let elapsed = start.elapsed();
        assert!(!doc.lines().is_empty());
        assert!(doc.highlights.is_empty(), "UI path must not sync-highlight");
        // ponytail: ~25 ms is sync markdown parse for 50 KB; syntect was ~367 ms.
        // Raise only if parse itself becomes the freeze; upgrade = parse off UI thread.
        assert!(
            elapsed.as_millis() < 50,
            "raw load took {elapsed:?}, expected < 50ms (release)"
        );
    }

    #[test]
    fn raw_doc_does_not_highlight_on_construct() {
        use crate::tui::viewer_doc::RawDoc;
        let doc = RawDoc::from_source("# Hi\n\npara\n", None);
        assert!(doc.highlights.is_empty());
    }
}
