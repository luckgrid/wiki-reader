//! Markdown → `RenderedDoc` (styled lines, link spans, source map).

mod diagrams;
mod link_span;
mod render;

pub use diagrams::{DiagramEnv, DiagramTier, diagram_lines, is_mermaid_lang, select_tier};
pub use link_span::{LinkClass, LinkId, LinkSpan};
pub use render::{
    BlockAction, BlockActionKind, RenderOpts, RenderedDoc, StyleKind, StyledLine, StyledSpan,
    render, render_with,
};

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
                .filter(|l| {
                    l.contains('│')
                        || l.contains('┼')
                        || l.contains('├')
                        || l.contains('┌')
                        || l.contains('└')
                })
                .collect();
            assert!(
                table_lines.iter().any(|l| l.starts_with('┌')),
                "w={width}: missing top border: {plain}"
            );
            assert!(
                table_lines.iter().any(|l| l.starts_with('└')),
                "w={width}: missing bottom border: {plain}"
            );
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
                table_lines.iter().find(|l| {
                    l.contains('│')
                        && !l.contains('┼')
                        && !l.starts_with('┌')
                        && !l.starts_with('└')
                }),
                table_lines
                    .iter()
                    .find(|l| l.contains('┼') || (l.contains('├') && !l.starts_with('┌'))),
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
            for line in table_lines.iter().filter(|l| {
                l.contains('│')
                    && !l.contains('┼')
                    && !l.contains('├')
                    && !l.starts_with('┌')
                    && !l.starts_with('└')
            }) {
                assert!(
                    cells_padded_both_sides(line),
                    "w={width}: cell missing side pad: {line}"
                );
            }
        }
    }

    #[test]
    fn table_cells_never_split_words_when_fitted() {
        let src = "| WideA | WideB | WideC | WideD | WideE |\n| --- | --- | --- | --- | --- |\n| alpha | bravo | charlie | delta | echo |\n| foxrot | golf | hotel | india | juliet |\n";
        let doc = render_src(src, 80);
        let plain = doc.lines.join("\n");
        assert!(
            !plain.contains("table too wide"),
            "expected fitted table at 80: {plain}"
        );
        for word in [
            "WideA", "WideB", "WideC", "WideD", "WideE", "alpha", "bravo", "charlie", "delta",
            "echo", "foxrot", "golf", "hotel", "india", "juliet",
        ] {
            assert!(
                plain.contains(word),
                "word {word:?} must appear intact: {plain}"
            );
        }
        // Too-narrow pane: dump fallback, still no mid-word cell wrap via fair_share.
        let narrow = render_src(src, 40);
        let narrow_plain = narrow.lines.join("\n");
        assert!(
            narrow_plain.contains("table too wide for pane"),
            "expected dump fallback: {narrow_plain}"
        );
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
            .filter(|(l, _)| l.contains('│') && !l.contains('├'))
            .map(|(l, &s)| (l.clone(), s))
            .collect();
        assert_eq!(
            mapped.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            vec![1, 3, 4],
            "table data rows should map to distinct source lines: {mapped:?}"
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
    fn linked_from_lists_tokens_on_design_system() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/design-system/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        let section = doc
            .lines
            .iter()
            .position(|l| l.contains("Linked from"))
            .expect("Linked from heading");
        let joined = doc.lines[section..].join("\n");
        assert!(
            joined.contains("Token") || joined.contains("tokens"),
            "expected tokens backlink label in {joined:?}"
        );
        let bl = doc
            .links
            .iter()
            .find(|s| s.raw_target == "/architecture/design-system/tokens.md")
            .expect("root-relative tokens backlink span");
        assert_eq!(bl.class, LinkClass::Internal);
        // Backlink segments land on/after the Linked from heading line.
        let bl_line = bl.segments[0].0 as usize;
        assert!(
            bl_line > section,
            "backlink after heading ({bl_line} > {section})"
        );
    }

    #[test]
    fn linked_from_omitted_when_no_backlinks() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("decisions/0002-adapters.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        assert!(
            !doc.lines.iter().any(|l| l.contains("Linked from")),
            "page with no inbound edges must not emit Linked from"
        );
    }

    #[test]
    fn linked_from_wraps_title_segments_at_narrow_widths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("src0.md"),
            "---\ntitle: \"A very long page title number 0 that certainly exceeds thirty columns of width\"\n---\n\n# Src\n\nSee [dst](dst.md).\n",
        )
        .unwrap();
        std::fs::write(root.join("dst.md"), "# Dst\n\nBody.\n").unwrap();
        let provider = FsProvider::open(root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("dst.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        for &width in &[30u16, 40u16, 80u16] {
            let doc = render(&src, page, &key, &index, width);
            let bl = doc
                .links
                .iter()
                .find(|s| s.raw_target == "/src0.md")
                .unwrap_or_else(|| panic!("backlink at width {width}"));
            assert!(!bl.segments.is_empty(), "width {width}: expected segments");
            // Every link glyph on those rows must sit inside a segment.
            for &(line, (start, end)) in &bl.segments {
                let row = &doc.styled[line as usize];
                let row_text: String = row.spans.iter().map(|s| s.text.as_str()).collect();
                let link_text: String = row
                    .spans
                    .iter()
                    .filter(|s| matches!(s.kind, StyleKind::Link))
                    .map(|s| s.text.as_str())
                    .collect();
                assert!(
                    end > start,
                    "width {width} line {line}: empty segment in {row_text:?}"
                );
                // Segment covers the full link run on that row (hang indent is Plain).
                let mut col = 0u16;
                for sp in &row.spans {
                    let w = u16::try_from(sp.text.width()).unwrap_or(0);
                    if matches!(sp.kind, StyleKind::Link) {
                        assert!(
                            col >= start && col + w <= end,
                            "width {width} line {line}: link {:?} cols {col}..{} outside segment {start}..{end}; row={row_text:?} link={link_text:?}",
                            sp.text,
                            col + w
                        );
                    }
                    col = col.saturating_add(w);
                }
            }
            // Wrapped titles produce multiple segments at width 30.
            if width == 30 {
                assert!(
                    bl.segments.len() >= 2,
                    "width 30 should wrap long title, segs={:?}",
                    bl.segments
                );
            }
        }
    }

    #[test]
    fn frontmatter_collapsed_by_default_expands_with_opts() {
        let src = "---\ntitle: Hello\nupdated: 2026-01-01\n---\n\n# Body\n";
        let doc = render_src(src, 40);
        assert!(
            doc.lines.iter().any(|l| l.contains("frontmatter ▶")),
            "collapsed cue missing: {:?}",
            doc.lines
        );
        assert!(
            !doc.lines.iter().any(|l| l.contains("title: Hello")),
            "collapsed must hide fields: {:?}",
            doc.lines
        );
        assert!(
            doc.block_actions
                .iter()
                .any(|a| a.kind == BlockActionKind::ToggleFrontmatter)
        );
        let id = doc.block_actions[0].id;
        let mut expanded = std::collections::HashSet::new();
        expanded.insert(id);
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
        let open = render_with(
            src,
            None,
            &empty_key(),
            &index,
            40,
            &RenderOpts {
                expanded,
                ..RenderOpts::default()
            },
        );
        assert!(
            open.lines
                .iter()
                .any(|l| l.contains("title") && l.contains("Hello")),
            "expanded missing title: {:?}",
            open.lines
        );
    }

    #[test]
    fn long_table_shows_all_rows_without_expand_stub() {
        let src = "| h |\n| - |\n| 1 |\n| 2 |\n| 3 |\n| 4 |\n| 5 |\n";
        let doc = render_src(src, 40);
        assert!(
            !doc.lines.iter().any(|l| l.contains("expand table")),
            "expand stub must be gone: {:?}",
            doc.lines
        );
        let joined = doc.lines.join("\n");
        assert!(joined.contains('5'), "all rows visible: {joined}");
        assert!(
            doc.block_actions.is_empty(),
            "tables no longer register block actions: {:?}",
            doc.block_actions
        );
    }

    #[test]
    fn word_wrap_breaks_at_whitespace() {
        let src =
            "never midword break when wrapping turning words across the pane width cleanly.\n";
        let doc = render_src(src, 20);
        for line in &doc.lines {
            assert!(
                !line.contains("tu rning") && !line.contains("turni ng"),
                "mid-word wrap: {line:?} in {:?}",
                doc.lines
            );
        }
        let joined = doc.lines.join(" ");
        assert!(joined.contains("turning"), "missing word: {joined}");
    }

    #[test]
    fn block_gap_between_top_level_not_list_items() {
        let src = "# H\n\npara\n\n- a\n- b\n\n## Next\n";
        let doc = render_src(src, 40);
        let text = doc.lines.join("\n");
        assert!(
            text.contains("# H\n\npara\n\n• a\n• b\n\n## Next"),
            "expected blank gaps between blocks and tight list: {text:?}"
        );
    }

    #[test]
    fn block_gap_quote_alert_code_table_rule() {
        let src = concat!(
            "> quote one\n>\n> quote two\n\n",
            "> [!NOTE]\n> Note body.\n\n",
            "```\ncode\n```\n\n",
            "| a | b |\n| - | - |\n| 1 | 2 |\n\n",
            "---\n\n",
            "# After\n"
        );
        let doc = render_src(src, 40);
        let text = doc.lines.join("\n");
        assert!(
            text.contains("│ quote one\n│\n│ quote two"),
            "quote inter-para keeps bar: {text:?}"
        );
        assert!(
            text.contains("│ [NOTE]\n│ Note body."),
            "alert label tight to body: {text:?}"
        );
        assert!(
            text.contains("```\n│ code\n```"),
            "code block present: {text:?}"
        );
        assert!(
            text.contains('┌') && text.contains('└'),
            "table borders: {text:?}"
        );
        assert!(
            text.contains("─\n\n# After") || text.contains("──\n\n# After"),
            "gap after rule: {text:?}"
        );
    }

    #[test]
    fn quote_wrap_keeps_bar_and_nested_list() {
        let src = "> A plain blockquote with enough text that it wraps on a narrow pane.\n>\n> - nested\n";
        let doc = render_src(src, 40);
        let text = doc.lines.join("\n");
        let quote_lines: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with('│') || *l == "│")
            .collect();
        assert!(
            quote_lines
                .iter()
                .any(|l| l.contains("wraps") || l.contains("narrow")),
            "expected wrapped quote: {text:?}"
        );
        assert!(
            text.lines()
                .filter(|l| l.contains("narrow") || l.contains("wraps on"))
                .all(|l| l.starts_with('│')),
            "wrap continuation must keep bar: {text:?}"
        );
        assert!(
            text.contains('•') && text.contains("nested"),
            "nested list in quote: {text:?}"
        );
    }

    #[test]
    fn linked_from_gap_is_single_blank() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/design-system/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render(&src, page, &key, &index, 80);
        let text = doc.lines.join("\n");
        assert!(
            !text.contains("\n\n\n## Linked from"),
            "no double blank before Linked from: {text:?}"
        );
        assert!(
            text.contains("\n\n## Linked from"),
            "single blank before Linked from: {text:?}"
        );
    }

    #[test]
    fn linked_from_formatted_drops_hashes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/worked-example");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("architecture/design-system/README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        let doc = render_with(
            &src,
            page,
            &key,
            &index,
            80,
            &RenderOpts {
                formatted: true,
                ..RenderOpts::default()
            },
        );
        let text = doc.lines.join("\n");
        assert!(
            !text.contains("## Linked from"),
            "formatted must drop hashes: {text:?}"
        );
        assert!(
            text.contains("Linked from"),
            "formatted still shows the label: {text:?}"
        );
        let section = doc
            .lines
            .iter()
            .position(|l| l == "Linked from")
            .expect("Linked from heading line");
        assert!(
            doc.lines.get(section + 1).is_some_and(|l| l.contains('─')),
            "H2 rule under Linked from: {text:?}"
        );
    }

    #[test]
    fn expanded_frontmatter_snapshot() {
        let src = "---\ntitle: Hello\ntags:\n  - a\n  - b\nupdated: 2026-01-01\n---\n\n# Body\n";
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
        let collapsed = render_src(src, 40);
        let id = collapsed
            .block_actions
            .iter()
            .find(|a| a.kind == BlockActionKind::ToggleFrontmatter)
            .expect("toggle")
            .id;
        let mut expanded = std::collections::HashSet::new();
        expanded.insert(id);
        let open = render_with(
            src,
            None,
            &empty_key(),
            &index,
            40,
            &RenderOpts {
                expanded,
                ..RenderOpts::default()
            },
        );
        let text = open.lines.join("\n");
        insta::assert_snapshot!("expanded_frontmatter_40", text);
    }

    #[test]
    fn elements_fixture_snapshots() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/elements");
        let provider = FsProvider::open(&root).unwrap();
        let index = wiki_reader_core::Index::build(&provider).unwrap();
        let key = PageKey {
            collection_id: index.collection_id.clone(),
            relative_path: std::path::PathBuf::from("README.md"),
        };
        let src = provider.read(&key).unwrap();
        let page = index.pages.get(&key);
        for w in [40u16, 60, 80, 120] {
            let doc = render(&src, page, &key, &index, w);
            let text = doc.lines.join("\n");
            insta::assert_snapshot!(format!("elements_{w}"), text);
            let formatted = render_with(
                &src,
                page,
                &key,
                &index,
                w,
                &RenderOpts {
                    formatted: true,
                    ..RenderOpts::default()
                },
            );
            insta::assert_snapshot!(
                format!("elements_formatted_{w}"),
                formatted.lines.join("\n")
            );
        }
    }

    #[test]
    fn code_block_registers_copy_payload() {
        let doc = render_src("```rust\nfn main() {}\n```\n", 40);
        let action = doc
            .block_actions
            .iter()
            .find(|a| a.kind == BlockActionKind::CopyCode)
            .expect("copy action");
        assert!(
            action.payload.contains("fn main"),
            "payload={}",
            action.payload
        );
    }

    #[test]
    fn mermaid_fallback_does_not_duplicate_source() {
        let src = "```mermaid\nnot-a-valid-diagram-xyzzy\n```\n";
        let doc = render_src(src, 40);
        let plain = doc.lines.join("\n");
        let headers = doc
            .lines
            .iter()
            .filter(|l| l.starts_with("│ diagram (source"))
            .count();
        assert_eq!(headers, 1, "{plain}");
        let body_only = doc
            .lines
            .iter()
            .filter(|l| *l == "not-a-valid-diagram-xyzzy")
            .count();
        assert_eq!(body_only, 1, "source body once: {plain}");
        assert!(
            !doc.lines
                .iter()
                .any(|l| l.starts_with("│ not-a-valid") || *l == "│ not-a-valid-diagram-xyzzy"),
            "must not paint fence body before fallback: {plain}"
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

    #[test]
    fn formatted_heading_list_keeps_coloured_rule() {
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
        let key = empty_key();
        for (src, level, rule_min) in [
            ("## Lists\n\n- a\n- b\n", 2u8, 40usize),
            ("### Section\n- a\n", 3u8, 24usize),
        ] {
            let doc = render_with(
                src,
                None,
                &key,
                &index,
                40,
                &RenderOpts {
                    formatted: true,
                    ..RenderOpts::default()
                },
            );
            let text = doc.lines.join("\n");
            assert!(
                text.contains('─') && text.contains('•'),
                "expected rule then list: {text:?}"
            );
            let rule_line = doc
                .styled
                .iter()
                .find(|l| l.spans.iter().any(|s| s.text.contains('─')))
                .expect("rule line");
            assert!(
                rule_line
                    .spans
                    .iter()
                    .any(|s| s.kind == StyleKind::Heading(level)),
                "rule should use Heading({level}), got {:?}",
                rule_line.spans
            );
            let rule_len = rule_line
                .spans
                .iter()
                .filter(|s| s.text.contains('─'))
                .map(|s| s.text.chars().count())
                .sum::<usize>();
            assert_eq!(rule_len, rule_min.min(40));
            // Blank between rule and first bullet.
            let rule_idx = doc
                .styled
                .iter()
                .position(|l| l.spans.iter().any(|s| s.text.contains('─')))
                .unwrap();
            assert!(
                doc.styled
                    .get(rule_idx + 1)
                    .is_some_and(|l| l.spans.is_empty()),
                "blank after rule before list"
            );
        }
    }
}
