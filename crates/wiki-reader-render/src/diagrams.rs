//! Mermaid diagram tiers (ADR-0004). Text tier via `mermaid-text`; image deferred.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};

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

/// Lines to paint for a mermaid fence under `tier` (never image yet).
///
/// `width` is part of the cache key so a later width-aware text renderer can
/// land without callers changing.
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
        let flow = render_text_tier("graph LR; A[Build] --> B[Deploy]").unwrap();
        assert!(flow.contains("Build"), "{flow}");
        assert!(flow.contains("Deploy"), "{flow}");
        let seq = render_text_tier("sequenceDiagram\nAlice->>Bob: Hi\n").unwrap();
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
        let src = "graph LR; CacheA --> CacheB";
        CACHE_HITS.store(0, std::sync::atomic::Ordering::Relaxed);
        let (a, _) = diagram_lines(src, DiagramTier::Text, 72);
        let hits_before = CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
        let (b, _) = diagram_lines(src, DiagramTier::Text, 72);
        let hits_after = CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(a, b);
        assert_eq!(hits_before, 0);
        assert_eq!(hits_after, 1);
    }
}
