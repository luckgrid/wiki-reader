---
id: WR-ROADMAP-AUDIT-V018
title: Audit of v0.1.8
summary: Viewing-experience performance audit of v0.1.8 with a P3-31 measured baseline and scheduled fix batches.
status: active
updated: 2026-10-06
related: [phase-3-alpha, dogfood-log, audit-v0.1.5]
---

# Audit of v0.1.8

Static re-audit of **v0.1.8** aimed at the viewing experience: frame time, scroll, resize and keypress latency, idle CPU, and open and reindex cost. Three read-only code reviews covered TUI hot paths; render and core; and media, viewers, herdr, tooling and CI. Highest-impact claims were spot-checked against the code. [P3-31](phase-3-alpha.md) then measured a release harness on synthetic fixtures (see [benchmarks](../architecture/benchmarks.md#viewing-cost-p3-31)); the **Measured** column and severity below use those numbers. The Rust checklist covered blind `unwrap`/`expect`, needless clones, allocation in loops, boolean arguments, catch-all `_ =>`, silent `let _ =`, truncating casts, and O(n²) scans.

No `unwrap`/`expect` problems were found in the audited non-test code, no truncating `as` outside guarded clamps, and `unsafe_code` is forbidden. That is a real strength; the findings are about repeated work, not safety. Line numbers drift, so find items by symbol.

**V-findings** continue the style of the [v0.1.5 audit](audit-v0.1.5.md) with a `V` prefix. Many restate staged E/N/L leftovers from that register. **Status values:** `fixed`, `staged` (still scheduled), `dropped` (measured negligible — not fixed). Fix batches [P3-32](phase-3-alpha.md)…[P3-37](phase-3-alpha.md) do **not** gate the Phase 3 exit. Proposed budgets (not CI asserts): frame build ≤ 16 ms p95, keypress→frame ≤ 50 ms, width relayout ≤ 50 ms, UI-thread reindex ≤ 16 ms.

**Verified on main** at `58fe76b` (P3-34 V4, #161; P3-31b #160). Tag `v0.1.8` = release merge `cb36cec` (#154). Register landed in #158.

**Shipped in v0.1.9:** V4 (P3-34 linear wrap).

**Shipped in v0.1.10:** V22 (P3-37 orphaned-reader exit: session-leader watchdog, stdin hang-up poll, TTY guard, no panic on a dead stderr) and the V17 signal-registration part.

## High

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V4 | `render.rs` wrap helpers; `diagrams.rs` fallback | Quadratic wrapping: each cut copies the remainder. A multi-MB token or data URI stalls open and each resize. | Before: open 1 MB **26.3 s**, 400 KB resize **8.7 s**. After linear wrap: open 1 MB **8.2 ms**, 400 KB resize **6.0 ms** (two release runs; re-confirmed on `58fe76b`). | fixed (P3-34) |
| V22 | `events.rs` `run_loop`; crossterm `event::poll` | Orphaned processes burn ~100 %CPU indefinitely. Live sample (v0.1.8): main thread stuck in `poll` → `try_read` → `read` with a still-open slave TTY and no interactive shell. Not V17 hygiene. Fix: `IsTerminal` guard, early signals, session-leader watchdog (`kill(sid, 0)` via rustix), stdin POLLHUP/ERR poll via rustix (covers live-parent + closed master). Forced exit skips Drop and terminal restore (hung-up PTY writes can block forever; herdr TTL / no session flush). | Dogfood 2026-10-05: four spinners ~45 min. PTY matrix on release binary (2026-10-06): (i) parent alive + master closed + slave held → exit; (ii) parent killed → exit; (iii) `/dev/null` → exit; (iv) HUP/TERM/INT → exit. On main before fix, (i) spun at ~100 %CPU. Follow-up check `scripts/check-orphan-exit.py` (CI, ubuntu + macOS) also found a startup race: closing the PTY master during startup aborted the process (SIGABRT, macOS crash report) because `eprintln!` panics on a dead stderr; `main` and the background-panic report now use `eprint_line` (write errors ignored). Root cause pinned on macOS (v0.1.10 work): the spin follows the death of the **session leader**, with the PTY master still open and drained; killing only the parent while the leader lives leaves the reader idle (0.2 % CPU), and a stdin hang-up poll does not see the leader case. The first V22 fix keyed on the parent pid, which was both too broad (a launcher exiting would end a healthy session) and indirect. `WIKI_READER_NO_WATCHDOG=1` skips the leader check. | fixed (P3-37, v0.1.10) |
| V23 | process exit on macOS; `events.rs` forced exit, `check-orphan-exit.py` | **Known issue, investigate later.** A `wiki-reader` that is exiting while its PTY master stays open but is never read can stay in macOS state `E` (shown as `(wiki-reader)` in `ps`) and ignores SIGKILL until the master is closed. Hypothesis: closing the slave waits for pending output to drain, and nothing drains it. Seen twice in tests, both with a test harness that never read the master (an early "parent killed" case and a run against v0.1.9); not seen with a terminal emulator, which always reads. Impact is a process lingering while a frozen terminal holds the other end. Ideas to try: flush pending output (`tcflush` through `rustix::termios`) before the forced exit, and check whether a clean exit is affected the same way. | Reproduced only with an unread master; the harness now drains it. No numbers. | staged (P3-38) |

## Medium

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V3 | `watch.rs`; `session.rs` `reindex` | N12 remainder: full rebuild off-thread; deep `index ==`; real change reloads page even if open file unchanged. | UI-thread reindex identical **5.5 ms**, one-page change **15.6–16.9 ms** (at the 16 ms budget). | staged (P3-33); low priority |
| V6 | `raster.rs` `usvg_options` | E5: font copied and font DB rebuilt every call; SVG measure on UI thread; Mermaid layout 2–3×. | Not isolated (TestBackend / text-tier diagrams). | staged (P3-35); measure first |
| V12 | `render_with`, `body_and_offset` | E6: clones `ParsedPage` and body. | Folded into open/relayout times; not isolated. | dropped (re-scope): no measured cost |
| V15 | `image_viewer.rs`; Kitty path | Uncancelled pan workers; suspected re-upload per pan. | Needs Kitty / counting writer. | staged (P3-35); measure first |
| V16 | `diagrams.rs` caches | E4 unbounded width-keyed cache; E8 drops fallback reason. | Not isolated. | staged (P3-35); cheap hygiene |
| V17 | opener / herdr / signals | L10: late signals, no SIGINT, unreaped children, no herdr timeout. | Process behaviour, not timed. | staged (P3-37 signals; P3-36 reap/timeout/open errors) |
| V18 | `index.rs`; `nav/tree.rs` | Full bodies in index; key duplication; no byte budget. | RSS after 5k-page open ≈ **39–42 MB**; after heavy pages ≈ **134–140 MB** (`ps` RSS). | dropped (re-scope): no measured cost |

## Dropped (negligible under budget)

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V1 | `visible_rows` / nav flatten | Whole tree cloned per frame. | Expanded 5k-page nav frame: **0.86 ms** (TestBackend — excludes terminal diff/flush). | dropped (P3-31) |
| V2 | `relayout_after_diagram_size`, `ensure_layout_width` | N13: diagram-ready still full-reloads; width changes reload rendered pages. Cache shared (not cleared every drag). | Width relayout **1.6 ms**; diagram-ready relayout **1.1 ms** (both under budget). | dropped (P3-31b): coalesce not worth a fix alone |
| V5 | `focus_list` / link scan | Clone+sort links each frame; rows × links scan. | Link-heavy frame: **1.7 ms** (TestBackend). | dropped (P3-31) |
| V9 | breadcrumb / label DFS | Whole-tree DFS with clones per frame. | Included in frame timings above (TestBackend). | dropped (P3-31) |
| V14 | `keep_cursor_visible` (P3-30 regression), `max_expand_height` | O(rows) jump; per-cell `Vec<char>`; wrap churn. | 10k-row table: `want` **4.1 ms**, `G`+draw **2.3 ms**, `PgDn`+draw **1.0 ms**. Dropped at 10k only — loop is O(rows), so ≈25 ms at 100k. | dropped (P3-31) |
| V7 | `events.rs` event loop | L12: draw every tick. | Idle **0.0 %CPU** / idlew **0** on v0.1.8 (PTY). | dropped (P3-31b) |
| V8 | `images.rs` idle poll | 20 ms worker poll; per-frame set churn. | Same idle pass as V7 (bare PTY; no cell-size / image manager). Graphics-on remeasure still pending — confirm in Ghostty/herdr with a picture. | dropped (P3-31b); confirm in Ghostty/herdr with a picture |
| V10 | `app/mod.rs` search | Sync UI-thread search; re-sort / lowercase. | Content search keystroke+flush **5.5 ms** (under 50 ms budget). | dropped (P3-31b) |
| V11 | `match_spans`, `store_search_matches` | Rebuild glyph map / linear hit walk. | `match_spans` **0.007 ms**; next-match+draw **0.9 ms**. | dropped (P3-31b) |
| V13 | `push_media`; `resolve_related` | O(n²) media push; O(pages × related). | Media-heavy open (200 images) **7.2 ms**; related edges folded into index open (~30 ms vs prior 5k baseline). | dropped (P3-31b) |

## Supply chain and tooling

| ID | Where | Finding | Measured | Status |
|---|---|---|---|---|
| V19 | `release.yml` | N18: write token during dependency build scripts. | Spot-checked (static). | staged (P3-36) |
| V20 | repo / workflows | L15: no `deny.toml` / Dependabot / `SECURITY.md`; tag-pinned actions; advisories. | `cargo audit`: `bincode` 1.3.3 and `ttf-parser` 0.25.1 **unmaintained** warnings. `cargo deny` not installed (skipped). Clippy `-W perf/nursery/unwrap_used/…`: ~1121 warnings, mostly test `unwrap`/`expect`; 7 `redundant_clone` (see benchmarks). | staged (P3-36) |
| V21 | `check_doc` | N19: resolver mismatch vs reader. | Static. | staged (P3-36) |

## Low and code quality

| ID | Finding | Status |
|---|---|---|
| L13 remainder | `nav_scroll` is `u16`. | staged (low; was bundled with V11) |
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
| [P3-31](phase-3-alpha.md) | Baseline (measure) | Done: harness + [benchmarks](../architecture/benchmarks.md#viewing-cost-p3-31); register re-ranked. P3-31b top-up: idle CPU, V2/V10/V11/V13/V4-scale |
| [P3-32](phase-3-alpha.md) | Frame cost | **Empty / dropped** — V7/V8/V11 dropped after measurement (V1/V5/V9 already dropped). V8 still needs graphics-on idle confirm in Ghostty/herdr |
| [P3-33](phase-3-alpha.md) | Relayout and reindex | **V3 only** (low priority). V12/V18 dropped unless re-measured |
| [P3-34](phase-3-alpha.md) | Algorithmic | **V4 fixed** (linear wrap; shipped v0.1.9). V10/V13/V14 dropped |
| [P3-35](phase-3-alpha.md) | Media | Measure first: V6 (SVG harness), V15 (Kitty), V16 (LRU + reason), L1–L5 caps |
| [P3-36](phase-3-alpha.md) | Process and tooling | V17 remainder (reap/timeout/open errors), V19, V20, V21, boolean-arg tidy-ups |
| [P3-37](phase-3-alpha.md) | Orphan spin + early signals | **V22 fixed** (v0.1.10), V17 signal registration before raw mode |

Each fix PR starts with a failing test or a measurement; `./scripts/check.sh all`; regression tests with exact assertions; operator manual pass for user-visible changes (P3-34 wrap, P3-35 media); a dogfood-log line; release (`v0.1.10`+) only after the pass and explicit go-ahead before tagging.

P3-31 re-confirmed scope: items that measured negligible are **dropped** above, not fixed.

## Related

- [Audit of v0.1.5](audit-v0.1.5.md)
- [Phase 3](phase-3-alpha.md)
- [Dogfood log](dogfood-log.md)
- [Benchmarks](../architecture/benchmarks.md)
