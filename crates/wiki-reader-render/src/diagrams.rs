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

fn source_fallback_lines(src: &str, reason: &str, width: u16) -> Vec<String> {
    let pane = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut header = format!("│ diagram (source; {reason})");
    while !header.is_empty() {
        if header.width() <= pane {
            lines.push(std::mem::take(&mut header));
            break;
        }
        let (take, next) = split_fallback_line(&header, pane);
        lines.push(take);
        header = next;
    }
    for raw in src.lines() {
        let mut rest = raw.to_owned();
        if rest.is_empty() {
            lines.push(String::new());
            continue;
        }
        while !rest.is_empty() {
            if rest.width() <= pane {
                lines.push(std::mem::take(&mut rest));
                break;
            }
            let (take, next) = split_fallback_line(&rest, pane);
            lines.push(take);
            rest = next;
        }
    }
    lines
}

/// Word-boundary wrap for source fallback; hard-split only when a token exceeds the pane.
fn split_fallback_line(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_owned());
    }
    if s.width() <= max {
        return (s.to_owned(), String::new());
    }
    let mut col = 0usize;
    let mut last_ws: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cw > max {
            if let Some(pos) = last_ws {
                let take = s[..pos].trim_end().to_owned();
                let rest = s[pos..].trim_start().to_owned();
                if !take.is_empty() {
                    return (take, rest);
                }
            }
            if i == 0 {
                let next = i + ch.len_utf8();
                return (s[..next].to_owned(), s[next..].to_owned());
            }
            return (s[..i].to_owned(), s[i..].to_owned());
        }
        if ch.is_whitespace() {
            last_ws = Some(i);
        }
        col += cw;
    }
    (s.to_owned(), String::new())
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
            source_fallback_lines(src, "image tier not wired", width),
            Some("image tier unavailable".into()),
        ),
        DiagramTier::Text => match render_text_tier(src, width) {
            Ok(out) => {
                let rendered: Vec<String> = out.lines().map(str::to_owned).collect();
                let max_w = max_line_width(&rendered);
                let pane = usize::from(width.max(1));
                if max_w > pane {
                    let reason = format!("diagram {max_w} cols > pane {pane}");
                    (source_fallback_lines(src, &reason, width), Some(reason))
                } else {
                    (rendered, None)
                }
            }
            Err(reason) => (source_fallback_lines(src, &reason, width), Some(reason)),
        },
        DiagramTier::Source => (
            source_fallback_lines(src, "source tier", width),
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
        let flow = render_text_tier("graph LR; A[Build] --> B[Deploy]", 80).unwrap();
        assert!(flow.contains("Build"), "{flow}");
        assert!(flow.contains("Deploy"), "{flow}");
        let seq = render_text_tier("sequenceDiagram\nAlice->>Bob: Hi\n", 80).unwrap();
        assert!(seq.contains("Alice") || seq.contains("Bob"), "{seq}");
    }

    #[test]
    fn tier_table_matches_adr() {
        let env = DiagramEnv::default();
        assert_eq!(select_tier(DiagramMode::Auto, &env), DiagramTier::Text);
        let herdr = DiagramEnv {
            herdr: true,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Auto, &herdr), DiagramTier::Text);
        assert_eq!(
            select_tier(DiagramMode::Source, &DiagramEnv::default()),
            DiagramTier::Source
        );
        let env = DiagramEnv {
            kitty_graphics: false,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Image, &env), DiagramTier::Text);
        let env = DiagramEnv {
            kitty_graphics: true,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Auto, &env), DiagramTier::Image);
    }

    #[test]
    fn fallback_emits_source_once() {
        let bad = "not a real diagram {{{";
        let (lines, reason) = diagram_lines(bad, DiagramTier::Text, 80);
        assert!(reason.is_some());
        let header = lines
            .iter()
            .filter(|l| l.starts_with("│ diagram (source"))
            .count();
        assert_eq!(header, 1, "{lines:?}");
        let body_lines = bad.lines().count();
        let repeats = lines
            .iter()
            .filter(|l| l.as_str() == bad.lines().next().unwrap_or(""))
            .count();
        // Source body appears once after the header, not doubled.
        assert_eq!(lines.len(), 1 + body_lines, "{lines:?}");
        assert_eq!(repeats, 1, "{lines:?}");
    }

    #[test]
    fn cache_hit_on_second_call() {
        let src = "graph LR; CacheUniqueP223 --> CacheUniqueP223B";
        CACHE_HITS.store(0, std::sync::atomic::Ordering::Relaxed);
        let (a, _) = diagram_lines(src, DiagramTier::Text, 72);
        let hits_before = CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
        let (b, _) = diagram_lines(src, DiagramTier::Text, 72);
        let hits_after = CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(a, b);
        assert_eq!(hits_before, 0);
        assert_eq!(hits_after, 1);
    }

    #[test]
    fn text_tier_respects_width_or_falls_back() {
        let src = "flowchart LR\n  AAAAAAAAAA[VeryLongNodeLabelAlpha] --> BBBBBBBBBB[VeryLongNodeLabelBravo] --> CCCCCCCCCC[VeryLongNodeLabelCharlie]\n";
        let (lines, _) = diagram_lines(src, DiagramTier::Text, 40);
        let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        assert!(
            max_w <= 40,
            "source fallback must wrap to pane, max_w={max_w} lines={lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("diagram") && l.contains("source")),
            "expected source fallback header: {lines:?}"
        );
    }
}
