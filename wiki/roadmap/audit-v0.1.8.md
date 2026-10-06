---
id: WR-ROADMAP-AUDIT-V018
title: Audit of v0.1.8
summary: Viewing-experience performance audit of v0.1.8 with a P3-31 measured baseline and scheduled fix batches.
status: active
updated: 2026-10-05
related: [phase-3-alpha, dogfood-log, audit-v0.1.5]
---

# Audit of v0.1.8

Static re-audit of **v0.1.8** aimed at the viewing experience: frame time, scroll, resize and keypress latency, idle CPU, and open and reindex cost. Three read-only code reviews covered TUI hot paths; render and core; and media, viewers, herdr, tooling and CI. Highest-impact claims were spot-checked against the code. [P3-31](phase-3-alpha.md) then measured a release harness on synthetic fixtures (see [benchmarks](../architecture/benchmarks.md#viewing-cost-p3-31)); the **Measured** column and severity below use those numbers. The Rust checklist covered blind `unwrap`/`expect`, needless clones, allocation in loops, boolean arguments, catch-all `_ =>`, silent `let _ =`, truncating casts, and O(n²) scans.

No `unwrap`/`expect` problems were found in the audited non-test code, no truncating `as` outside guarded clamps, and `unsafe_code` is forbidden. That is a real strength; the findings are about repeated work, not safety. Line numbers drift, so find items by symbol.

**V-findings** continue the style of the [v0.1.5 audit](audit-v0.1.5.md) with a `V` prefix. Many restate staged E/N/L leftovers from that register. **Status values:** `fixed`, `staged` (still scheduled), `dropped` (measured negligible — not fixed). Fix batches [P3-32](phase-3-alpha.md)…[P3-36](phase-3-alpha.md) do **not** gate the Phase 3 exit. Proposed budgets (not CI asserts): frame build ≤ 16 ms p95, keypress→frame ≤ 50 ms, width relayout ≤ 50 ms, UI-thread reindex ≤ 16 ms.

**Verified on main** at `378f9fa`: tag `v0.1.8` = release merge `cb36cec` (#154). Register landed in #158. P3-31 harness on `feat/p3-31-perf-baseline`.

## High

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V4 | `render.rs` wrap helpers; `diagrams.rs` fallback | Quadratic wrapping: each cut copies the remainder. A multi-MB token or data URI stalls open and each resize. | Open 1 MB token page: **26.3 s** median (two release runs). | staged (P3-34) |

## Medium

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V2 | `relayout_after_diagram_size`, `ensure_layout_width` | N13: diagram-ready still full-reloads; width changes reload rendered pages. Cache shared (not cleared every drag). | Width relayout on diagrams page: **1.6 ms** (under budget). Diagram-ready path not isolated in the harness. | staged (P3-33): keep diagram-ready coalesce; width path not worth a fix alone |
| V3 | `watch.rs`; `session.rs` `reindex` | N12 remainder: full rebuild off-thread; deep `index ==`; real change reloads page even if open file unchanged. | UI-thread reindex identical **5.5 ms**, one-page change **15.6 ms** (at the 16 ms budget). | staged (P3-33) |
| V6 | `raster.rs` `usvg_options` | E5: font copied and font DB rebuilt every call; SVG measure on UI thread; Mermaid layout 2–3×. | Not isolated (TestBackend / text-tier diagrams). | staged (P3-35) |
| V7 | `events.rs` event loop | L12: `terminal.draw` every tick, no dirty flag. | Idle CPU not measured here (needs a real pane). Frame build itself is cheap once drawn. | staged (P3-32) |
| V8 | `images.rs` `recv_prefer_decode` | L12: 20 ms poll when graphics on; per-frame `HashSet` / `PathBuf` churn. | Idle wakeups not measured (needs a real pane). | staged (P3-32) |
| V10 | `app/mod.rs` search; `core/search.rs` | Sync UI-thread search; re-sort and lowercase each call. E2 remainder. | Not timed in harness (5k-page collection would stress it). | staged (P3-34) |
| V11 | `match_spans`, `store_search_matches` | L13: rebuild glyph map / linear hit walk. | Not isolated (no active Content search in harness). | staged (P3-32) |
| V12 | `render_with`, `body_and_offset` | E6: clones `ParsedPage` and body. | Folded into open/relayout times; not isolated. | staged (P3-33) |
| V13 | `push_media`; `resolve_related` | L6: O(n²) media push; O(pages × related). | Not isolated. | staged (P3-34) |
| V15 | `image_viewer.rs`; Kitty path | Uncancelled pan workers; suspected re-upload per pan. | Needs Kitty / counting writer. | staged (P3-35) |
| V16 | `diagrams.rs` caches | E4 unbounded width-keyed cache; E8 drops fallback reason. | Not isolated. | staged (P3-35) |
| V17 | opener / herdr / signals | L10: late signals, no SIGINT, unreaped children, no herdr timeout. | Process behaviour, not timed. | staged (P3-36) |
| V18 | `index.rs`; `nav/tree.rs` | Full bodies in index; key duplication; no byte budget. | RSS after 5k-page open ≈ **39 MB**; after heavy pages ≈ **134–140 MB** (`ps` RSS). | staged (P3-33) |

## Dropped (negligible under budget)

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V1 | `visible_rows` / nav flatten | Whole tree cloned per frame. | Expanded 5k-page nav frame: **0.86 ms**. | dropped (P3-31) |
| V5 | `focus_list` / link scan | Clone+sort links each frame; rows × links scan. | Link-heavy frame: **1.7 ms**. | dropped (P3-31) |
| V9 | breadcrumb / label DFS | Whole-tree DFS with clones per frame. | Included in frame timings above. | dropped (P3-31) |
| V14 | `keep_cursor_visible` (P3-30 regression), `max_expand_height` | O(rows) jump; per-cell `Vec<char>`; wrap churn. | 10k-row table: `want` **4.1 ms**, `G`+draw **2.3 ms**, `PgDn`+draw **1.0 ms**. | dropped (P3-31) |

## Supply chain and tooling

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V19 | `release.yml` | N18: write token during dependency build scripts. | Spot-checked (static). | staged (P3-36) |
| V20 | repo / workflows | L15: no `deny.toml` / Dependabot / `SECURITY.md`; tag-pinned actions; advisories. | `cargo audit`: `bincode` 1.3.3 and `ttf-parser` 0.25.1 **unmaintained** warnings. `cargo deny` not installed (skipped). Clippy `-W perf/nursery/unwrap_used/…`: ~1121 warnings, mostly test `unwrap`/`expect`; 7 `redundant_clone` (see benchmarks). | staged (P3-36) |
| V21 | `check_doc` | N19: resolver mismatch vs reader. | Static. | staged (P3-36) |

## Low and code quality

| ID | Finding | Status |
|---|---|---|
| L13 remainder | `nav_scroll` is `u16`. | staged (P3-32 with V11) |
| L11 remainder | Boolean-flag arguments; silent `let _ =`. | staged (P3-36) |
| — | Render `strip_fm` vs parse frontmatter drift; parse lacks `ENABLE_GFM`; `Event::Html` dropped. | staged |
| — | Public fields on `Tab` / `NavState` / `NavTree.items`; exclude-pattern diagnostics discarded; session expanded hash order. | staged |
| L7 | Tabs not expanded in code. | staged |
| — | `fold_dir` lowercase allocs; alert-prefix allocs; wheel hit-map scans; `pos_at` / `patch_cols` allocs. | staged |
| — | No `[profile.release]` tuning; no `unwrap_used` / `expect_used` lints by default. | staged (P3-36 / V20) |

## Done well

Input coalescing of up to 256 events per redraw; width clamp at 100 so wide resizes do not relayout; adaptive poll; decode, highlight and reindex off the UI thread with cancellation; viewer paints only visible rows; `SlugAllocator`, `partition_point` line lookups, `MAX_TEXT_HITS`; casts use `try_from`; guarded workers; atomic config and session writes; CI is `--locked` with a lite-tree guard and env indirection for `ref_name`. Everyday frame cost on a 5k-page expanded nav stayed under 2 ms in the P3-31 harness.

## Checked and sound

`FsProvider::read` path checks; link resolution (index-only, cannot leave the root); the image policy of [ADR-0017](../decisions/0017-static-local-images-only.md) (canonicalise, root check, size and pixel caps, no external SVG references); the config trust split (a collection file cannot set `opener`, `editor` or `keys`); the URL opener (scheme allowlist, confirm, no shell); editor launch; herdr argument cleaning; the OSC 52 size cap; `u16` layout arithmetic at tiny sizes.

## Fix batches

| Task | Batch | Items |
|---|---|---|
| [P3-31](phase-3-alpha.md) | Baseline (measure) | Done: harness + [benchmarks](../architecture/benchmarks.md#viewing-cost-p3-31); register re-ranked |
| [P3-32](phase-3-alpha.md) | Frame cost | V7, V8, V11 (V1/V5/V9 dropped) |
| [P3-33](phase-3-alpha.md) | Relayout and reindex | V2 (diagram-ready), V3, V12, V18 |
| [P3-34](phase-3-alpha.md) | Algorithmic | V4, V13, V10 (V14 dropped) |
| [P3-35](phase-3-alpha.md) | Media | V6, V15, V16, L1–L5 caps |
| [P3-36](phase-3-alpha.md) | Process and tooling | V17, V19, V20, V21, boolean-arg tidy-ups |

Each fix PR starts with a failing test or a measurement; `./scripts/check.sh all`; regression tests with exact assertions; operator manual pass for user-visible changes (P3-32, P3-35); a dogfood-log line; release (`v0.1.9`+) only after the pass and explicit go-ahead before tagging.

P3-31 re-confirmed scope: items that measured negligible are **dropped** above, not fixed.

## Related

- [Audit of v0.1.5](audit-v0.1.5.md)
- [Phase 3](phase-3-alpha.md)
- [Dogfood log](dogfood-log.md)
- [Benchmarks](../architecture/benchmarks.md)
