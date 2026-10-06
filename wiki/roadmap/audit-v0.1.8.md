---
id: WR-ROADMAP-AUDIT-V018
title: Audit of v0.1.8
summary: Viewing-experience performance and Rust anti-pattern audit of v0.1.8 (frame time, scroll, resize and keypress latency, idle CPU, open and reindex cost), with severity, status and scheduled fix batches.
status: active
updated: 2026-10-05
related: [phase-3-alpha, dogfood-log, audit-v0.1.5]
---

# Audit of v0.1.8

Static re-audit of **v0.1.8** aimed at the viewing experience: frame time, scroll, resize and keypress latency, idle CPU, and open and reindex cost. Three read-only code reviews covered TUI hot paths; render and core; and media, viewers, herdr, tooling and CI. Highest-impact claims were spot-checked against the code (see Spot-checked notes in findings where relevant). Nothing was built or run for this register, so every item is **traced, not run** until [P3-31](phase-3-alpha.md) measures a baseline. The Rust checklist covered blind `unwrap`/`expect`, needless clones, allocation in loops, boolean arguments, catch-all `_ =>`, silent `let _ =`, truncating casts, and O(n²) scans.

No `unwrap`/`expect` problems were found in the audited non-test code, no truncating `as` outside guarded clamps, and `unsafe_code` is forbidden. That is a real strength; the findings are about repeated work, not safety. Line numbers drift, so find items by symbol.

**V-findings** continue the style of the [v0.1.5 audit](audit-v0.1.5.md) with a `V` prefix. Many restate staged E/N/L leftovers from that register; the Finding cell cites the prior ID where known. Severity is by effect on viewing. **Status values:** `planned` (next PR), `staged` (recorded, scheduled for a later release). Fix batches [P3-31](phase-3-alpha.md)…[P3-36](phase-3-alpha.md) do **not** gate the Phase 3 exit.

**Verified on main** at `378f9fa`: tag `v0.1.8` = release merge `cb36cec` (#154); assets and Cargo.lock at 0.1.8; #155 follow-ups for the table viewer; operator manual pass on the release binary; no open PRs. No remediation was needed before this audit.

## High: per-frame and relayout cost

| ID | Where | Finding | Status |
|---|---|---|---|
| V1 | `tui/app/draw.rs` → `nav_ui` `clamp_nav_scroll`; `regions/side_nav.rs` `visible_rows` | The whole expanded nav tree is flattened and every `NodeId` and label cloned twice per frame, plus 2–4 more times per nav wheel or key event. Cost scales with the tree. Spot-checked. | staged (P3-32) |
| V2 | `app/mod.rs` `ensure_layout_width`, `relayout_after_diagram_size`, `reload_page_keeping_view_ex`; `events.rs` diagram-ready and nav-divider drag | N13 still present: diagram-ready calls `relayout_after_diagram_size` → full page reload and `RenderedViewerDoc::build_with`; nav-divider drag calls `resize_nav_to_column` every move and the next frame’s `ensure_layout_width` reloads when text width changes. The diagram size cache is shared (not cleared every mouse move). Spot-checked. | staged (P3-33) |
| V3 | `core/watch.rs`; `nav/session.rs` `reindex`; `core/index.rs` | N12 remainder: every watcher event rebuilds the whole index (off-thread), then the UI thread does a deep `index == index` compare and, on any real change, reloads the page and resets selection even when the open page is unchanged. Spot-checked. | staged (P3-33) |
| V4 | `render.rs` `push_span`, `split_at_width`, `split_at_word_boundary`, `wrap_cell`; `diagrams.rs` fallback | Quadratic wrapping: each cut copies the remainder. A multi-MB token or data URI stalls open and each resize. | staged (P3-34) |
| V5 | `app/viewer_state.rs` `focus_list`; `viewer_doc.rs`; `regions/viewer.rs` | `focus_list()` clones every link target and sorts it each frame; the viewer scans all link spans and focus items for each visible row (rows × links × ~4). Spot-checked. Restates L13 in part. | staged (P3-32) |
| V6 | `render/images.rs`; `media/raster.rs` `usvg_options` | E5 still open: the embedded font is copied and a font DB rebuilt on every `usvg_options()`; sizing a local SVG reads and parses it on the UI thread per image per relayout; Mermaid layout runs 2–3 times per diagram. Spot-checked (font). | staged (P3-35) |

## Medium

| ID | Where | Finding | Status |
|---|---|---|---|
| V7 | `events.rs` event loop | L12: `terminal.draw` every loop iteration, no dirty flag (≈4 full frame builds/s idle); V1/V5/V9 repeat on each. Spot-checked. | staged (P3-32) |
| V8 | `images.rs` `recv_prefer_decode` | L12 remainder: polls every 20 ms forever (≈50 wakeups/s) when graphics are on. Also per-frame `HashSet<SlotKey>` with `PathBuf` clones, `retain_for` with `Vec::contains`, and `has_pending` scanning all entries. | staged (P3-32) |
| V9 | `app/draw.rs` breadcrumb/page labels; `core/nav/tree.rs` | Breadcrumb and page labels recomputed each frame by a whole-tree DFS with clones; `page_order()` clones every key where still used for labels. | staged (P3-32) |
| V10 | `app/mod.rs` search; `core/search.rs` | Content and page search run synchronously on the UI thread; each call re-sorts all pages and lowercases titles and paths. Restates E2 remainder. | staged (P3-34) |
| V11 | `app/mod.rs` `match_spans`, `store_search_matches`; `draw.rs` | L13 remainder: `match_spans` rebuilds a case-folded glyph map over matching source lines every frame; `store_search_matches` walks hits with per-hit `display_cursor`. No binary search on `source_map` and no cached match spans. | staged (P3-32) |
| V12 | `render.rs` `render_with`, `body_and_offset` | E6: `render_with` clones `ParsedPage` (body, links, headings) and every event; `body_and_offset` copies the body again. | staged (P3-33) |
| V13 | `render.rs` `push_media`; `index.rs` `resolve_related` | L6: `push_media` is O(n²); `resolve_related` is O(pages × related). | staged (P3-34) |
| V14 | `table_viewer.rs` `keep_cursor_visible`, `max_expand_height`; `viewer_doc.rs` `wrap_starts` | `keep_cursor_visible` is O(rows) per jump (introduced with P3-30 in v0.1.8 — the only regression in this register rather than an old gap); `max_expand_height` allocates `Vec<char>` per cell; the cursor row is re-wrapped 2–3× per frame; `wrap_starts` is duplicated in `viewer_doc.rs`. | staged (P3-34) |
| V15 | `image_viewer.rs`; `images.rs` Kitty re-encode | A thread per `set_view` or Tab with no cancel; every pan step re-crops, re-composes the canvas and re-uploads the Kitty image (suspected churn, needs measurement). Inline Kitty re-encode while scrolling clipped slots also suspected. | staged (P3-35) |
| V16 | `diagrams.rs`; `render/images.rs` | E4: diagram cache unbounded and keyed by width; `DiagramSizeCache::clear` never called. E8: a cache hit on open silently drops the fallback reason. | staged (P3-35) |
| V17 | opener / herdr / `events.rs` signals | L10: signal handlers registered late, no SIGINT; opener children never reaped; herdr CLI has no timeout; open and signal failures are silent `let _ =`. | staged (P3-36) |
| V18 | `core/index.rs`; `nav/tree.rs` | Every page keeps its full body and duplicates headings, keys and diagnostics; ~4 copies of each key in the nav tree; no total memory budget (pathological collections). | staged (P3-33) |

## Supply chain and tooling

| ID | Where | Finding | Status |
|---|---|---|---|
| V19 | `.github/workflows/release.yml` | N18: the build job has `contents: write` and persisted credentials while `cargo build` runs dependency build scripts. Split into a read-only build job and a minimal upload job; `persist-credentials: false`. Spot-checked. | staged (P3-36) |
| V20 | repo root, workflows | L15: no `deny.toml`, `SECURITY.md` or `dependabot.yml` (spot-checked); `checkout@v5`, `rust-cache@v2`, `upload-artifact@v4` pinned by tag while others are SHA-pinned; `bincode` 1.3.3 (via syntect) advisory to verify with `cargo audit` / `cargo deny`. | staged (P3-36) |
| V21 | `wiki-reader-tools` `check_doc` | N19: uses raw `join` + `canonicalize` instead of the reader's resolver, so it gives false failures for extensionless, `%20`, `?query` and root-relative links and passes non-indexed targets. Fix: reuse `core::nav::resolve` against a built `Index`. | staged (P3-36) |

## Low and code quality

| ID | Finding | Status |
|---|---|---|
| L13 remainder | `nav_scroll` is `u16`. | staged (with V5/V11 in P3-32) |
| L11 remainder | Boolean-flag arguments (`viewer::draw` with many parameters, `reload_page_keeping_view_ex`, `App::build`, `Theme::effective_name`, `flush_session(force)`, `search_jump`, `help_jump`). | staged (P3-36) |
| — | Render `strip_fm` duplicates parse's frontmatter logic and disagrees on a BOM and EOF fence; parse lacks `ENABLE_GFM` while render sets it; `Event::Html` dropped silently. | staged |
| — | Public fields on `Tab` / `NavState` / `NavTree.items`; `provider.rs` discards exclude-pattern diagnostics; session expanded saved in hash order. | staged |
| L7 | Tabs not expanded in code. | staged |
| — | `fold_dir` sorts with `to_lowercase` allocation per compare and nondeterministic case-only ties; alert-prefix parsing ~16 allocations per quote event; per-event wheel hit-map scans; `pos_at` clones the line per drag event; `patch_cols` allocates per char. | staged |
| — | No `[profile.release]` tuning (LTO, codegen-units, panic); no `unwrap_used` / `expect_used` lints. | staged (with V20 / P3-31 one-off) |

## Done well

Input coalescing of up to 256 events per redraw; width clamp at 100 so wide resizes do not relayout; adaptive poll; decode, highlight and reindex off the UI thread with cancellation; viewer paints only visible rows; `SlugAllocator`, `partition_point` line lookups, `MAX_TEXT_HITS`; casts use `try_from`; guarded workers; atomic config and session writes; CI is `--locked` with a lite-tree guard and env indirection for `ref_name`.

## Checked and sound

`FsProvider::read` path checks; link resolution (index-only, cannot leave the root); the image policy of [ADR-0017](../decisions/0017-static-local-images-only.md) (canonicalise, root check, size and pixel caps, no external SVG references); the config trust split (a collection file cannot set `opener`, `editor` or `keys`); the URL opener (scheme allowlist, confirm, no shell); editor launch; herdr argument cleaning; the OSC 52 size cap; `u16` layout arithmetic at tiny sizes.

## Fix batches

| Task | Batch | Items |
|---|---|---|
| [P3-31](phase-3-alpha.md) | Baseline (measure) | Large-fixture generator and timing harness; record in [benchmarks](../architecture/benchmarks.md); one-off `cargo audit` / `cargo deny` and extra clippy lints; re-rank this register with numbers |
| [P3-32](phase-3-alpha.md) | Frame cost | V1, V5, V9, V7, V8, V11 |
| [P3-33](phase-3-alpha.md) | Relayout and reindex | V2, V3, V12, V18 |
| [P3-34](phase-3-alpha.md) | Algorithmic | V4, V13, V10, V14 |
| [P3-35](phase-3-alpha.md) | Media | V6, V15, V16, L1–L5 caps |
| [P3-36](phase-3-alpha.md) | Process and tooling | V17, V19, V20, V21, boolean-arg tidy-ups |

Each fix PR starts with a failing test or a measurement; `./scripts/check.sh all`; regression tests with exact assertions; operator manual pass for user-visible changes (P3-32, P3-35); a dogfood-log line; release (`v0.1.9`+) only after the pass and explicit go-ahead before tagging.

Severity above is provisional until [P3-31](phase-3-alpha.md) measures a baseline. P3-32…P3-36 scope is then re-confirmed against those numbers: an item that measures negligible is dropped, not fixed.

## Related

- [Audit of v0.1.5](audit-v0.1.5.md)
- [Phase 3](phase-3-alpha.md)
- [Dogfood log](dogfood-log.md)
- [Benchmarks](../architecture/benchmarks.md)
