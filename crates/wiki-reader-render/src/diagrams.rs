//! Mermaid diagram tiers (ADR-0004). Text tier via `mermaid-text`; image deferred.

use wiki_reader_core::config::DiagramMode;

/// Environment hints for tier selection (no terminal probes here).
#[derive(Debug, Clone, Default)]
pub struct DiagramEnv {
    pub tmux: bool,
    pub herdr: bool,
    pub kitty_graphics: bool,
}

impl DiagramEnv {
    /// Read common env vars once at startup.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            tmux: std::env::var_os("TMUX").is_some(),
            herdr: std::env::var_os("HERDR_ENV").as_deref() == Some(std::ffi::OsStr::new("1")),
            // Image path not wired yet; never claim Kitty unless probe exists later.
            kitty_graphics: false,
        }
    }
}

/// Chosen render tier for a diagram block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramTier {
    Image,
    Text,
    Source,
}

/// Pick a tier per ADR-0004.
#[must_use]
pub fn select_tier(mode: DiagramMode, env: &DiagramEnv) -> DiagramTier {
    match mode {
        DiagramMode::Image => {
            if env.kitty_graphics {
                DiagramTier::Image
            } else {
                DiagramTier::Text
            }
        }
        DiagramMode::Text => DiagramTier::Text,
        DiagramMode::Source => DiagramTier::Source,
        DiagramMode::Auto => {
            if env.tmux {
                DiagramTier::Text
            } else if env.herdr {
                if env.kitty_graphics {
                    DiagramTier::Image
                } else {
                    DiagramTier::Text
                }
            } else if env.kitty_graphics {
                DiagramTier::Image
            } else {
                DiagramTier::Text
            }
        }
    }
}

/// True when a fenced code language is Mermaid.
#[must_use]
pub fn is_mermaid_lang(lang: &str) -> bool {
    let lang = lang.trim();
    lang.eq_ignore_ascii_case("mermaid") || lang.eq_ignore_ascii_case("mmd")
}

/// Text-tier render; on failure returns the reason for a source fallback line.
pub fn render_text_tier(src: &str) -> Result<String, String> {
    mermaid_text::render(src).map_err(|e| e.to_string())
}

/// Lines to paint for a mermaid fence under `tier` (never image yet).
#[must_use]
pub fn diagram_lines(src: &str, tier: DiagramTier) -> (Vec<String>, Option<String>) {
    match tier {
        DiagramTier::Image => (
            vec!["│ diagram: image tier not wired (falling back to text)".into()],
            Some("image tier unavailable".into()),
        ),
        DiagramTier::Text => match render_text_tier(src) {
            Ok(out) => (out.lines().map(str::to_owned).collect(), None),
            Err(reason) => {
                let mut lines = vec![format!("│ diagram (source; {reason})")];
                lines.extend(src.lines().map(str::to_owned));
                (lines, Some(reason))
            }
        },
        DiagramTier::Source => {
            let mut lines = vec!["│ diagram (source)".into()];
            lines.extend(src.lines().map(str::to_owned));
            (lines, Some("source tier".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mermaid_lang() {
        assert!(is_mermaid_lang("mermaid"));
        assert!(is_mermaid_lang("Mermaid"));
        assert!(is_mermaid_lang("mmd"));
        assert!(!is_mermaid_lang("rust"));
    }

    #[test]
    fn text_tier_flowchart_and_sequence() {
        let flow = render_text_tier("graph LR; A[Build] --> B[Deploy]").unwrap();
        assert!(flow.contains("Build"), "{flow}");
        assert!(flow.contains("Deploy"), "{flow}");
        let seq = render_text_tier("sequenceDiagram\nAlice->>Bob: Hi\n").unwrap();
        assert!(seq.contains("Alice") || seq.contains("Bob"), "{seq}");
    }

    #[test]
    fn tier_table_matches_adr() {
        let env = DiagramEnv {
            tmux: true,
            herdr: false,
            kitty_graphics: true,
        };
        assert_eq!(select_tier(DiagramMode::Auto, &env), DiagramTier::Text);
        let herdr = DiagramEnv {
            tmux: false,
            herdr: true,
            kitty_graphics: false,
        };
        assert_eq!(select_tier(DiagramMode::Auto, &herdr), DiagramTier::Text);
        assert_eq!(
            select_tier(DiagramMode::Source, &DiagramEnv::default()),
            DiagramTier::Source
        );
    }
}
