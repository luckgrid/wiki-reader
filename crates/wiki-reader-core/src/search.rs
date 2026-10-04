//! Full-text and title search (S1 core).

use crate::Index;
use crate::provider::PageKey;

/// One page-level hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHit {
    /// Page key.
    pub page: PageKey,
    /// Match score (lower is better).
    pub score: u32,
    /// How many fuzzy/substring matches contributed (title/path).
    pub match_count: u32,
}

/// One in-body text hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextHit {
    /// Page containing the match.
    pub page: PageKey,
    /// 1-based **source** line.
    pub line: u32,
    /// Short snippet around the match.
    pub snippet: String,
}

/// Grouped text hits for one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPageGroup {
    /// Page key.
    pub page: PageKey,
    /// Hits on this page (same order as [`search_text`]).
    pub hits: Vec<TextHit>,
}

/// Fuzzy rank pages by title/path (deterministic). Uses subsequence match so
/// `tkn` hits `token` (nucleo-style fuzzy without the full nucleo worker pool).
#[must_use]
pub fn search_pages(query: &str, index: &Index) -> Vec<PageHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<PageHit> = index
        .pages
        .values()
        .filter_map(|p| {
            let title = p.title.to_lowercase();
            let path = p.key.relative_path.to_string_lossy().to_lowercase();
            let (score, match_count) = page_score(&q, &title, &path)?;
            Some(PageHit {
                page: p.key.clone(),
                score,
                match_count,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| b.match_count.cmp(&a.match_count))
            .then_with(|| a.page.relative_path.cmp(&b.page.relative_path))
    });
    hits
}

fn page_score(q: &str, title: &str, path: &str) -> Option<(u32, u32)> {
    let words: Vec<&str> = q.split_whitespace().filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return None;
    }
    // Single token: existing subsequence fuzzy (tkn → token).
    if words.len() == 1 {
        return page_score_one(words[0], title, path);
    }
    // Multi-word any order: each word must be a subsequence of title or path.
    let mut score = 0u32;
    for w in &words {
        let mut best: Option<u32> = None;
        if let Some(s) = fuzzy_score(w, title) {
            best = Some(s);
        }
        if let Some(s) = fuzzy_score(w, path) {
            let s = s.saturating_add(10);
            best = Some(best.map_or(s, |b| b.min(s)));
        }
        score = score.saturating_add(best?);
    }
    Some((score, u32::try_from(words.len()).unwrap_or(1)))
}

fn page_score_one(q: &str, title: &str, path: &str) -> Option<(u32, u32)> {
    let mut score = u32::MAX;
    let mut count = 0u32;
    if let Some(s) = fuzzy_score(q, title) {
        score = score.min(s);
        count = count.saturating_add(1);
    }
    if let Some(s) = fuzzy_score(q, path) {
        score = score.min(s.saturating_add(10));
        count = count.saturating_add(1);
    }
    (count > 0).then_some((score, count))
}

/// Subsequence fuzzy: all query chars in order. Score = total gap (lower better).
fn fuzzy_score(query: &str, hay: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    if hay.contains(query) {
        return Some(0);
    }
    let mut qchars = query.chars();
    let mut need = qchars.next()?;
    let mut gap = 0u32;
    let mut matched = 0u32;
    for ch in hay.chars() {
        if ch == need {
            matched = matched.saturating_add(1);
            match qchars.next() {
                Some(n) => need = n,
                None => return Some(gap),
            }
        } else if matched > 0 {
            gap = gap.saturating_add(1);
        }
    }
    None
}

/// Case-insensitive find of `needle` in `text`, as a byte range of the **original** `text`.
///
/// Folding can change a character's byte length (`İ` grows, the Kelvin sign shrinks), so offsets
/// found in a lowercased copy must never be reused on the original. This folds char by char and
/// keeps each folded char's original byte span; a match that starts or ends inside one
/// character's expansion widens to that whole character.
#[must_use]
pub fn find_case_insensitive(text: &str, needle: &str) -> Option<std::ops::Range<usize>> {
    if needle.is_empty() {
        return None;
    }
    if text.is_ascii() && needle.is_ascii() {
        let n = needle.as_bytes();
        return text
            .as_bytes()
            .windows(n.len())
            .position(|w| w.eq_ignore_ascii_case(n))
            .map(|start| start..start + n.len());
    }
    let want: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    let mut folded: Vec<(char, usize, usize)> = Vec::new();
    for (start, ch) in text.char_indices() {
        let end = start + ch.len_utf8();
        folded.extend(ch.to_lowercase().map(|c| (c, start, end)));
    }
    if want.is_empty() || want.len() > folded.len() {
        return None;
    }
    (0..=folded.len() - want.len())
        .find(|&i| {
            folded[i..i + want.len()]
                .iter()
                .map(|f| f.0)
                .eq(want.iter().copied())
        })
        .map(|i| folded[i].1..folded[i + want.len() - 1].2)
}

/// Case-insensitive scan of indexed page bodies (deterministic grouping).
#[must_use]
pub fn search_text(query: &str, index: &Index) -> Vec<TextHit> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<TextHit> = Vec::new();
    let mut pages: Vec<_> = index.pages.values().collect();
    pages.sort_by(|a, b| a.key.relative_path.cmp(&b.key.relative_path));
    for page in pages {
        let offset = page.parsed.body_line_offset;
        for (i, line) in page.parsed.body.lines().enumerate() {
            if find_case_insensitive(line, q).is_some() {
                let line_no = offset.saturating_add(u32::try_from(i).unwrap_or(0));
                hits.push(TextHit {
                    page: page.key.clone(),
                    line: line_no,
                    snippet: snippet_line(line, q),
                });
            }
        }
    }
    hits
}

/// Group [`search_text`] hits by page, preserving order.
#[must_use]
pub fn group_text_hits(hits: &[TextHit]) -> Vec<TextPageGroup> {
    let mut groups: Vec<TextPageGroup> = Vec::new();
    for hit in hits {
        if let Some(last) = groups.last_mut()
            && last.page == hit.page
        {
            last.hits.push(hit.clone());
        } else {
            groups.push(TextPageGroup {
                page: hit.page.clone(),
                hits: vec![hit.clone()],
            });
        }
    }
    groups
}

fn snippet_line(line: &str, query: &str) -> String {
    let Some(found) = find_case_insensitive(line, query) else {
        return line.chars().take(60).collect();
    };
    let byte_at = found.start;
    // Work in char indices so CJK never panics on mid-char slices (U5).
    let char_starts: Vec<usize> = line
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(line.len()))
        .collect();
    let match_char = char_starts
        .partition_point(|&b| b < byte_at)
        .saturating_sub(1);
    let q_chars = line[found].chars().count();
    let start_char = match_char.saturating_sub(20);
    let end_char = (match_char + q_chars + 40).min(char_starts.len().saturating_sub(1));
    let start = char_starts[start_char];
    let end = char_starts[end_char];
    line[start..end].trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::FsProvider;
    use std::path::Path;

    fn worked() -> Index {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        Index::build(&provider).unwrap()
    }

    #[test]
    fn case_insensitive_find_maps_back_to_original_bytes() {
        // ASCII fast path.
        assert_eq!(find_case_insensitive("Hello World", "WORLD"), Some(6..11));
        assert_eq!(find_case_insensitive("abc", "abcd"), None);
        assert_eq!(find_case_insensitive("abc", ""), None);
        // `İ` lowercases to two chars and three bytes; offsets must stay in the original.
        let text = "İstanbul guide";
        let r = find_case_insensitive(text, "stanbul").unwrap();
        assert_eq!(&text[r], "stanbul");
        let r = find_case_insensitive(text, "guide").unwrap();
        assert_eq!(&text[r], "guide");
        // The Kelvin sign shrinks from three bytes to one when folded.
        let text = "\u{212A}elvin and more";
        let r = find_case_insensitive(text, "elvin").unwrap();
        assert_eq!(&text[r], "elvin");
        let r = find_case_insensitive(text, "kelvin").unwrap();
        assert_eq!(&text[r], "\u{212A}elvin");
        // CJK and mixed text never split a character.
        let r = find_case_insensitive("日本語のTEXT", "text").unwrap();
        assert_eq!(&"日本語のTEXT"[r], "TEXT");
    }

    #[test]
    fn snippet_keeps_the_match_after_many_expanding_chars() {
        let line = format!("{}needle tail", "İ".repeat(200));
        let snip = snippet_line(&line, "needle");
        assert!(snip.contains("needle"), "{snip}");
    }

    #[test]
    fn fuzzy_pages_tkn_matches_token() {
        let index = worked();
        // worked-example has architecture/design-system/tokens.md
        let hits = search_pages("tkn", &index);
        assert!(
            hits.iter()
                .any(|h| h.page.relative_path.to_string_lossy().contains("token")),
            "hits={hits:?}"
        );
    }

    #[test]
    fn multi_word_any_order_finds_token_projection() {
        let index = worked();
        let hits = search_pages("projection token", &index);
        assert!(
            hits.iter()
                .any(|h| { h.page.relative_path.to_string_lossy().contains("tokens.md") }),
            "expected Token Projection page, hits={hits:?}"
        );
    }

    #[test]
    fn snippet_cjk_no_panic() {
        let cjk = "中".repeat(30) + "needle" + &"文".repeat(30);
        let s = snippet_line(&cjk, "needle");
        assert!(s.contains("needle"), "{s}");
    }

    #[test]
    fn text_hit_line_includes_frontmatter_offset() {
        let index = worked();
        let key = index
            .pages
            .keys()
            .find(|k| k.relative_path.ends_with("tokens.md"))
            .cloned()
            .expect("tokens.md");
        let page = &index.pages[&key];
        let body_line = page
            .parsed
            .body
            .lines()
            .enumerate()
            .find(|(_, l)| l.to_lowercase().contains("token"))
            .map(|(i, _)| i)
            .expect("token in body");
        let hits = search_text("token", &index);
        let hit = hits.iter().find(|h| h.page == key).expect("text hit");
        let expected = page
            .parsed
            .body_line_offset
            .saturating_add(u32::try_from(body_line).unwrap());
        assert_eq!(hit.line, expected);
    }
}
