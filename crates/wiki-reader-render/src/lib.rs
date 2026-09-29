//! Markdown → `RenderedDoc` (styled lines, link spans, source map).

mod link_span;
mod render;

pub use link_span::{LinkClass, LinkId, LinkSpan};
pub use render::{RenderedDoc, StyleKind, StyledLine, StyledSpan, render};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;
    use unicode_width::UnicodeWidthStr;
    use wiki_reader_core::provider::{CollectionProvider, FsProvider, PageKey};

    fn empty_key() -> PageKey {
        PageKey {
            collection_id: "t".into(),
            relative_path: Path::new("x.md").into(),
        }
    }

    fn render_src(src: &str, width: u16) -> RenderedDoc {
        let index = wiki_reader_core::Index {
            collection_id: "t".into(),
            pages: HashMap::default(),
            edges: vec![],
            by_from: HashMap::default(),
            by_to: HashMap::default(),
            by_id: HashMap::default(),
            by_path: HashMap::default(),
            diagnostics: vec![],
        };
        render(src, None, &empty_key(), &index, width)
    }

    #[test]
    fn renders_heading_line() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        assert!(doc.lines.iter().any(|l| l.contains("Architecture")));
        assert!(!doc.links.is_empty());
    }

    #[test]
    fn paragraphs_wrap_separately_not_glued() {
        let src = "First paragraph with enough words to wrap at forty columns here.\n\nSecond paragraph also wraps independently of the first one.\n";
        let doc = render_src(src, 40);
        assert!(
            doc.lines.len() >= 4,
            "expected multiple wrapped lines, got {:?}",
            doc.lines
        );
        // No single monster line from glued paragraphs
        assert!(
            doc.lines.iter().all(|l| l.chars().count() <= 45),
            "line too wide: {:?}",
            doc.lines
        );
        let joined = doc.lines.join("\n");
        assert!(joined.contains("First paragraph"));
        assert!(joined.contains("Second paragraph"));
    }

    #[test]
    fn list_items_not_concatenated() {
        let src = "- a\n- b\n- c\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("|");
        assert!(!plain.contains("abc"), "list items glued: {plain:?}");
        assert!(
            doc.lines
                .iter()
                .filter(|l| l.contains('a') || l.contains('b') || l.contains('c'))
                .count()
                >= 3
        );
    }

    #[test]
    fn code_block_preserves_newlines() {
        let src = "```\nlet x = 1;\nlet y = 2;\n```\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            plain.contains("let x = 1;") && plain.contains("let y = 2;"),
            "{plain}"
        );
        assert!(
            !doc.lines.iter().any(|l| l.contains("let x = 1;let y")),
            "code lines merged: {:?}",
            doc.lines
        );
    }

    #[test]
    fn table_not_raw_pipes_only() {
        let src = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            doc.lines.iter().any(|l| l.contains('│') || l.contains('a')),
            "expected table layout, got {plain}"
        );
    }

    #[test]
    fn source_map_monotonic_and_nonzero() {
        let src = "# Hello\n\nPara one.\n\nPara two.\n";
        let doc = render_src(src, 40);
        assert!(!doc.source_map.is_empty());
        assert!(doc.source_map.iter().all(|&s| s >= 1));
        for w in doc.source_map.windows(2) {
            assert!(
                w[1] >= w[0],
                "source_map not monotonic: {:?}",
                doc.source_map
            );
        }
    }

    #[test]
    fn source_map_monotonic_across_fenced_code() {
        let src = "```rust\nfn a() {}\nfn b() {}\n```\n";
        let doc = render_src(src, 40);
        assert!(
            doc.source_map.windows(2).all(|w| w[1] >= w[0]),
            "source_map not monotonic: {:?}",
            doc.source_map
        );
        let plain = doc.lines.join("\n");
        assert!(
            !doc.lines.iter().any(|l| l == "│ " || l == "│"),
            "trailing empty code gutter: {plain:?}"
        );
        // Closing fence is its own row with its own source line (≥ last content).
        assert!(
            doc.lines.iter().any(|l| l.trim() == "```"),
            "missing close fence: {plain}"
        );
    }

    #[test]
    fn tight_lists_have_markers_and_nest_indent() {
        let src = "- a\n- b\n  - nested\n1. one\n2. two\n- [ ] task\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            doc.lines.iter().any(|l| l.contains('•') && l.contains('a')),
            "missing bullet: {plain}"
        );
        assert!(
            doc.lines
                .iter()
                .any(|l| l.contains("1.") && l.contains("one")),
            "missing ordered: {plain}"
        );
        assert!(
            doc.lines
                .iter()
                .any(|l| l.contains("nested") && l.starts_with("  ")),
            "missing nest indent: {plain}"
        );
        assert!(
            doc.lines
                .iter()
                .any(|l| l.contains('•') && l.contains("[ ]")),
            "task should keep bullet + marker: {plain}"
        );
    }

    #[test]
    fn table_fair_share_separator_and_width() {
        let src = "| short | a_very_long_header_cell |\n| --- | --- |\n| 1 | 2 |\n";
        for width in [40_u16, 60, 80] {
            let doc = render_src(src, width);
            let plain = doc.lines.join("\n");
            let table_lines: Vec<&String> = doc
                .lines
                .iter()
                .filter(|l| l.contains('│') || l.contains('┼') || l.contains('├'))
                .collect();
            assert!(
                table_lines
                    .iter()
                    .any(|l| l.contains('┼') || l.contains('├')),
                "w={width}: missing header separator: {plain}"
            );
            assert!(
                table_lines.iter().all(|l| l.width() <= usize::from(width)),
                "w={width}: row exceeds width: {plain}"
            );
            let widths: Vec<usize> = table_lines.iter().map(|l| l.width()).collect();
            assert!(
                widths.windows(2).all(|w| w[0] == w[1]),
                "w={width}: row/sep width mismatch {widths:?}: {plain}"
            );
            // Junction columns: find │ positions on a data row and ┼/┤ on sep.
            if let (Some(data), Some(sep)) = (
                table_lines
                    .iter()
                    .find(|l| l.contains('│') && !l.contains('┼')),
                table_lines
                    .iter()
                    .find(|l| l.contains('┼') || l.contains('├')),
            ) {
                let data_borders: Vec<usize> = data
                    .char_indices()
                    .filter(|(_, c)| *c == '│')
                    .map(|(i, _)| data[..i].width())
                    .collect();
                let sep_borders: Vec<usize> = sep
                    .char_indices()
                    .filter(|(_, c)| matches!(*c, '├' | '┼' | '┤'))
                    .map(|(i, _)| sep[..i].width())
                    .collect();
                assert_eq!(
                    data_borders, sep_borders,
                    "w={width}: border columns misaligned data={data:?} sep={sep:?}"
                );
            }
            for line in table_lines
                .iter()
                .filter(|l| l.contains('│') && !l.contains('┼') && !l.contains('├'))
            {
                assert!(
                    cells_padded_both_sides(line),
                    "w={width}: cell missing side pad: {line}"
                );
            }
        }
    }

    /// Each cell between `│` has a leading and trailing space.
    fn cells_padded_both_sides(line: &str) -> bool {
        let mut parts = line.split('│').filter(|p| !p.is_empty());
        parts.all(|cell| cell.starts_with(' ') && cell.ends_with(' '))
    }

    #[test]
    fn table_rows_map_to_own_source_lines() {
        let src = "| a | b |\n| --- | --- |\n| 1 | 2 |\n| NEEDLE | x |\n";
        let doc = render_src(src, 40);
        let mapped: Vec<(String, u32)> = doc
            .lines
            .iter()
            .zip(doc.source_map.iter())
            .filter(|(l, _)| l.contains('│') || l.contains('├'))
            .map(|(l, &s)| (l.clone(), s))
            .collect();
        assert_eq!(
            mapped.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            vec![1, 2, 3, 4],
            "table rows should map to distinct source lines: {mapped:?}"
        );
        let needle = mapped
            .iter()
            .find(|(l, _)| l.contains("NEEDLE"))
            .expect("NEEDLE row");
        assert_eq!(needle.1, 4);
    }

    #[test]
    fn source_map_monotonic_mixed_with_table() {
        let src = "# H\n\nPara.\n\n| a |\n| --- |\n| 1 |\n\nTail.\n";
        let doc = render_src(src, 40);
        assert!(
            doc.source_map.windows(2).all(|w| w[1] >= w[0]),
            "source_map not monotonic: {:?}",
            doc.source_map
        );
    }

    #[test]
    fn table_full_width_cells_keep_trailing_space() {
        // Wide content + narrow width → clipped cells must still pad both sides.
        let src = "| abcdefghijklmnopqrstuvwxyz |\n| --- |\n| abcdefghijklmnopqrstuvwxyz |\n";
        let doc = render_src(src, 20);
        for line in doc
            .lines
            .iter()
            .filter(|l| l.contains('│') && !l.contains('┼') && !l.contains('├'))
        {
            assert!(
                cells_padded_both_sides(line),
                "full-width cell missing pad: {line}"
            );
        }
    }

    #[test]
    fn alert_note_is_labelled() {
        let src = "> [!NOTE]\n> hello world\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(
            plain.contains("[NOTE]"),
            "expected labelled alert, got {plain}"
        );
    }

    #[test]
    fn wrapped_link_one_segment_per_line() {
        let src =
            "See [this is a fairly long link label that should wrap](https://example.com) end.\n";
        let doc = render_src(src, 30);
        assert!(!doc.links.is_empty());
        let segs = &doc.links[0].segments;
        assert!(!segs.is_empty());
        let mut lines: Vec<u32> = segs.iter().map(|(l, _)| *l).collect();
        lines.sort_unstable();
        lines.dedup();
        assert_eq!(
            segs.len(),
            lines.len(),
            "expected one segment per display line, got {segs:?}"
        );
    }

    #[test]
    fn footnote_reference_renders() {
        let src = "Note[^1]\n\n[^1]: body\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        assert!(plain.contains("[^1]"), "expected footnote ref, got {plain}");
    }

    #[test]
    fn wrapped_link_has_segments() {
        let src =
            "See [this is a fairly long link label that should wrap](https://example.com) end.\n";
        let doc = render_src(src, 30);
        assert!(!doc.links.is_empty());
        let segs = &doc.links[0].segments;
        assert!(!segs.is_empty());
    }

    #[test]
    fn anchor_line_is_rendered_index_in_range() {
        let src = "# Existing Heading\n\nBody text here.\n";
        let doc = render_src(src, 40);
        let (_, line) = doc
            .headings
            .iter()
            .find(|(s, _)| s == "existing-heading")
            .expect("heading");
        let idx = line.saturating_sub(1) as usize;
        assert!(
            idx < doc.lines.len(),
            "anchor {line} out of range for {} lines",
            doc.lines.len()
        );
        assert!(doc.lines[idx].to_lowercase().contains("existing"));
    }

    #[test]
    fn block_starts_are_rendered_lines() {
        let src = "Para.\n\n# Head\n\nMore.\n";
        let doc = render_src(src, 40);
        assert!(!doc.block_starts.is_empty());
        for &b in &doc.block_starts {
            assert!(b >= 1);
            assert!((b as usize) <= doc.lines.len().saturating_add(1));
        }
    }

    #[test]
    fn styled_lines_cover_headings() {
        let src = "# Title\n\n*em* **strong**\n";
        let doc = render_src(src, 80);
        assert!(
            doc.styled
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| matches!(s.kind, StyleKind::Heading(1)))
        );
    }

    #[test]
    fn render_scaling_not_quadratic() {
        use std::time::{Duration, Instant};
        let chunk = "# H\n\nPara with [link](https://ex.com) and a list:\n\n- a\n- b\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n```\ncode\n```\n\n";
        let small = chunk.repeat(200); // ≈ 25 KB
        let large = chunk.repeat(400); // ≈ 50 KB
        assert!(small.len() * 2 <= large.len() + chunk.len());

        // Warm caches / allocator on the real sizes (CI macOS is noisy).
        let _ = render_src(chunk, 80);
        let _ = render_src(&small, 80);

        let timed = |src: &str| -> Duration {
            let start = Instant::now();
            let doc = render_src(src, 80);
            assert!(!doc.lines.is_empty());
            start.elapsed()
        };
        // Best paired ratio of three: contested cores inflate a single shot.
        // Quadratic ≈ 4× per doubling; 3.5 leaves CI headroom without hiding O(n²).
        let (ratio, t_small, t_large) = (0..3)
            .map(|_| {
                let t_s = timed(&small);
                let t_l = timed(&large);
                let r = t_l.as_secs_f64() / t_s.as_secs_f64().max(1e-9);
                (r, t_s, t_l)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();
        assert!(
            ratio < 3.5,
            "doubling input slowed render {ratio:.2}× (small={t_small:?}, large={t_large:?}); expected < 3.5×"
        );
    }

    /// Manual / release budget: `cargo test -p wiki-reader-render --release -- --ignored`
    #[test]
    #[ignore = "release budget; run with --ignored --release"]
    fn render_budget_50kb_under_20ms() {
        use std::time::Instant;
        let chunk = "# H\n\nPara with [link](https://ex.com) and a list:\n\n- a\n- b\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n```\ncode\n```\n\n";
        let src = chunk.repeat(400);
        assert!(src.len() >= 40_000);
        let _ = render_src(chunk, 80);
        let start = Instant::now();
        let doc = render_src(&src, 80);
        let elapsed = start.elapsed();
        assert!(!doc.lines.is_empty());
        assert!(
            elapsed.as_millis() < 20,
            "render took {elapsed:?}, expected < 20ms (release)"
        );
    }
}
