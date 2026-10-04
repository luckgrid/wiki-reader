//! Mermaid diagram tiers (ADR-0004). Text via `mermaid-text`; image via slots + size cache.

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
    /// Confirmed graphics protocol from the TUI probe — never inferred from env vars alone.
    pub kitty_graphics: bool,
}

impl DiagramEnv {
    /// Read tmux / herdr env vars.
    ///
    /// `kitty_graphics` is always `false` here: graphics capability comes only from the
    /// TUI startup probe (set by the caller on a copy of this env, or via `RenderOpts::graphics`).
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            tmux: std::env::var_os("TMUX").is_some(),
            herdr: std::env::var_os("HERDR_ENV").as_deref() == Some(std::ffi::OsStr::new("1")),
            // Probe result only — never guess from TERM / HERDR_ENV.
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
    // tmux never gets the image tier, even for an explicit `diagrams = "image"` request.
    if env.tmux {
        return match mode {
            DiagramMode::Source => DiagramTier::Source,
            _ => DiagramTier::Text,
        };
    }
    match mode {
        DiagramMode::Image | DiagramMode::Auto => {
            if env.kitty_graphics {
                DiagramTier::Image
            } else {
                DiagramTier::Text
            }
        }
        DiagramMode::Text => DiagramTier::Text,
        DiagramMode::Source => DiagramTier::Source,
    }
}

/// True when a fenced code language is Mermaid.
#[must_use]
pub fn is_mermaid_lang(lang: &str) -> bool {
    let lang = lang.trim();
    lang.eq_ignore_ascii_case("mermaid") || lang.eq_ignore_ascii_case("mmd")
}

/// Stable content fingerprint for size/slot cache keys.
#[must_use]
pub fn content_hash(src: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    h.finish()
}

/// Text-tier render clamped to `width`; on failure returns the reason for a source fallback.
pub fn render_text_tier(src: &str, width: u16) -> Result<String, String> {
    mermaid_text::render_with_width(src, Some(usize::from(width.max(1)))).map_err(|e| e.to_string())
}

// ponytail: process-local sync cache for text/source lines; image size lives in DiagramSizeCache
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

fn text_tier_header_lines(reason: &str, width: u16) -> Vec<String> {
    let pane = usize::from(width.max(1));
    let mut header = format!("│ diagram (text; {reason})");
    let mut lines = Vec::new();
    while !header.is_empty() {
        if header.width() <= pane {
            lines.push(std::mem::take(&mut header));
            break;
        }
        let (take, next) = split_fallback_line(&header, pane);
        lines.push(take);
        header = next;
    }
    lines
}

/// Lines to paint for a mermaid fence under `tier`.
///
/// `width` is part of the cache key. Text tier uses `render_with_width`; if any
/// row is still wider than the pane, falls back to the fenced source with reason.
/// Image tier is handled by slots in the renderer; calling this with [`DiagramTier::Image`]
/// falls through to text (safety net).
#[must_use]
pub fn diagram_lines(src: &str, tier: DiagramTier, width: u16) -> (Vec<String>, Option<String>) {
    let tier = if tier == DiagramTier::Image {
        DiagramTier::Text
    } else {
        tier
    };
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
        DiagramTier::Image => unreachable!("mapped to Text above"),
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

/// Text-tier lines with an optional visible reason header (image→text fallback).
#[must_use]
pub fn diagram_lines_with_reason(
    src: &str,
    tier: DiagramTier,
    width: u16,
    reason: Option<&str>,
) -> (Vec<String>, Option<String>) {
    let (mut lines, fallback_reason) = diagram_lines(src, tier, width);
    if let Some(reason) = reason
        && tier == DiagramTier::Text
        && fallback_reason.is_none()
        && !lines.first().is_some_and(|l| l.starts_with("│ diagram ("))
    {
        let mut headed = text_tier_header_lines(reason, width);
        headed.append(&mut lines);
        return (headed, Some(reason.to_owned()));
    }
    (lines, fallback_reason)
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
        // tmux → text even for explicit image mode (ADR-0004).
        let tmux = DiagramEnv {
            tmux: true,
            kitty_graphics: true,
            ..DiagramEnv::default()
        };
        assert_eq!(select_tier(DiagramMode::Image, &tmux), DiagramTier::Text);
        assert_eq!(select_tier(DiagramMode::Auto, &tmux), DiagramTier::Text);
        assert_eq!(select_tier(DiagramMode::Source, &tmux), DiagramTier::Source);
        // herdr without confirmed Kitty stays text.
        let herdr_no_kitty = DiagramEnv {
            herdr: true,
            kitty_graphics: false,
            ..DiagramEnv::default()
        };
        assert_eq!(
            select_tier(DiagramMode::Auto, &herdr_no_kitty),
            DiagramTier::Text
        );
        let herdr_kitty = DiagramEnv {
            herdr: true,
            kitty_graphics: true,
            ..DiagramEnv::default()
        };
        assert_eq!(
            select_tier(DiagramMode::Auto, &herdr_kitty),
            DiagramTier::Image
        );
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
        // ponytail: CACHE_HITS is process-global and other tests render concurrently, so only
        // assert growth. Ceiling: a per-call hit flag would make this exact.
        assert!(hits_after > hits_before, "{hits_before} -> {hits_after}");
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

    #[test]
    fn text_reason_header_is_visible() {
        let src = "graph LR; A --> B";
        let (lines, reason) =
            diagram_lines_with_reason(src, DiagramTier::Text, 80, Some("no graphics protocol"));
        assert_eq!(reason.as_deref(), Some("no graphics protocol"));
        assert!(
            lines[0].contains("diagram (text; no graphics protocol)"),
            "{lines:?}"
        );
    }
}
