//! Syntect markdown highlighting for raw view (line gutter applied at draw).

use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Highlighted span for one run of text.
#[derive(Debug, Clone)]
pub struct HlSpan {
    /// Ratatui style.
    pub style: Style,
    /// Text fragment.
    pub text: String,
}

/// Highlight `source` as markdown. Falls back to plain lines on error.
#[must_use]
pub fn highlight_markdown(source: &str) -> Vec<Vec<HlSpan>> {
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let Some(syntax) = ps
        .find_syntax_by_token("md")
        .or_else(|| ps.find_syntax_by_name("Markdown"))
    else {
        return plain(source);
    };
    let Some(theme) = ts
        .themes
        .get("base16-ocean.dark")
        .or_else(|| ts.themes.values().next())
        .cloned()
    else {
        return plain(source);
    };
    let mut h = HighlightLines::new(syntax, &theme);
    let mut out = Vec::new();
    for line in LinesWithEndings::from(source) {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        match h.highlight_line(line, &ps) {
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
