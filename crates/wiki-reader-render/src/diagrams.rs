//! Mermaid diagram tiers (ADR-0004). Text tier via `mermaid-text`; image deferred.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use unicode_width::UnicodeWidthStr;
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

/// Text-tier render clamped to `width`; on failure returns the reason for a source fallback.
pub fn render_text_tier(src: &str, width: u16) -> Result<String, String> {
    mermaid_text::render_with_width(src, Some(usize::from(width.max(1)))).map_err(|e| e.to_string())
}

fn content_hash(src: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    h.finish()
}

// ponytail: process-local sync cache; off-thread/shared cache when image tier lands
#[allow(clippy::type_complexity)]
type DiagramCacheKey = (u64, u16, u8);
type DiagramCache = HashMap<DiagramCacheKey, Vec<String>>;
static DIAGRAM_CACHE: LazyLock<Mutex<DiagramCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(test)]
static CACHE_HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn source_fallback_lines(src: &str, reason: &str) -> Vec<String> {
    let mut lines = vec![format!("│ diagram (source; {reason})")];
    lines.extend(src.lines().map(str::to_owned));
    lines
}

fn max_line_width(lines: &[String]) -> usize {
    lines.iter().map(|l| l.width()).max().unwrap_or(0)
}

/// Lines to paint for a mermaid fence under `tier` (never image yet).
///
/// `width` is part of the cache key. Text tier uses `render_with_width`; if any
/// row is still wider than the pane, falls back to the fenced source with reason.
#[must_use]
pub fn diagram_lines(src: &str, tier: DiagramTier, width: u16) -> (Vec<String>, Option<String>) {
    let tier_key = match tier {
        DiagramTier::Image => 0u8,
        DiagramTier::Text => 1,
        DiagramTier::Source => 2,
    };
    let key = (content_hash(src), width, tier_key);
    if let Ok(cache) = DIAGRAM_CACHE.lock()
        && let Some(hit) = cache.get(&key)
    {
        #[cfg(test)]
        CACHE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return (hit.clone(), None);
    }

    let (lines, reason) = match tier {
        DiagramTier::Image => (
            source_fallback_lines(src, "image tier not wired"),
            Some("image tier unavailable".into()),
        ),
        DiagramTier::Text => match render_text_tier(src, width) {
            Ok(out) => {
                let rendered: Vec<String> = out.lines().map(str::to_owned).collect();
                let max_w = max_line_width(&rendered);
                let pane = usize::from(width.max(1));
                if max_w > pane {
                    let reason = format!("diagram {max_w} cols > pane {pane}");
                    (source_fallback_lines(src, &reason), Some(reason))
                } else {
                    (rendered, None)
                }
            }
            Err(reason) => (source_fallback_lines(src, &reason), Some(reason)),
        },
        DiagramTier::Source => (
            source_fallback_lines(src, "source tier"),
            Some("source tier".into()),
        ),
    };

    if let Ok(mut cache) = DIAGRAM_CACHE.lock() {
        cache.insert(key, lines.clone());
    }
    (lines, reason)
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
        let flow = "flowchart LR\n  A --> B\n";
        let out = render_text_tier(flow, 80).expect("flowchart");
        assert!(!out.trim().is_empty());
        let seq = "sequenceDiagram\n  Alice->>Bob: hi\n";
        let out = render_text_tier(seq, 80).expect("sequence");
        assert!(!out.trim().is_empty());
    }

    #[test]
    fn tier_table_matches_adr() {
        let env = DiagramEnv {
            kitty_graphics: false,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Auto, &env), DiagramTier::Text);
        assert_eq!(select_tier(DiagramMode::Image, &env), DiagramTier::Text);
        assert_eq!(select_tier(DiagramMode::Source, &env), DiagramTier::Source);
        let env = DiagramEnv {
            kitty_graphics: true,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Auto, &env), DiagramTier::Image);
    }

    #[test]
    fn fallback_emits_source_once() {
        let src = "not-valid-{{{{";
        let (lines, reason) = diagram_lines(src, DiagramTier::Text, 40);
        assert!(reason.is_some());
        let body_hits = lines.iter().filter(|l| l.as_str() == src).count();
        assert_eq!(body_hits, 1, "source body must appear once: {lines:?}");
    }

    #[test]
    fn cache_hit_on_second_call() {
        CACHE_HITS.store(0, std::sync::atomic::Ordering::Relaxed);
        let src = "flowchart LR\n  A --> B\n";
        let _ = diagram_lines(src, DiagramTier::Text, 60);
        let _ = diagram_lines(src, DiagramTier::Text, 60);
        assert!(
            CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed) >= 1,
            "expected cache hit"
        );
    }

    #[test]
    fn text_tier_respects_width_or_falls_back() {
        let src = "flowchart LR\n  AAAAAAAAAA[VeryLongNodeLabelAlpha] --> BBBBBBBBBB[VeryLongNodeLabelBravo] --> CCCCCCCCCC[VeryLongNodeLabelCharlie]\n";
        let (lines, _) = diagram_lines(src, DiagramTier::Text, 40);
        let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        assert!(
            max_w <= 40
                || lines
                    .iter()
                    .any(|l| l.contains("diagram") && l.contains("source")),
            "expected width clamp or source fallback, max_w={max_w} lines={lines:?}"
        );
    }
}
