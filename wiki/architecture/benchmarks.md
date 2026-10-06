---
id: WR-BENCH
title: Benchmarks
summary: Binary size, memory and start-up compared with other terminal markdown tools.
status: draft
updated: 2026-10-05
related: [prior-art-and-libs, audit-v0.1.8]
nav_order: 5
---

# Benchmarks

Installed binary, memory, start-up and crate count were measured on 2026-10-02 (not re-measured since) (local time) on macOS arm64, wiki-reader at `main` `976e3ff` (release profile, after the options window, viewers and themes) against the markdown-reader 1.34.75 Homebrew binary. These measurements were carried forward, not re-measured at `a153696`. Memory and start-up used a pseudo-terminal opened on this repo's `wiki/` folder (42 pages), three runs each; no graphics protocol answered, so Mermaid stayed on the text tier. Only the release download size was updated for `a153696` (**v0.1.1**): the published macOS arm64 tarball is 6.7 MiB. The original measurement note used the UTC date 2026-10-03.

| | wiki-reader | wiki-reader lite (P3-19) | markdown-reader | treemd 0.9.1 | glow 3.0.0 |
|---|---|---|---|---|---|
| Installed binary | 16.7 MiB (16.6 MiB re-measured, see below) | 8.4 MiB | 13.1 MiB | not measured | not measured |
| Release download (macOS arm64 tarball) | 6.7 MiB (v0.1.1) | no lite download ([ADR-0023](../decisions/0023-lite-build-is-a-cargo-feature.md)) | 6.0 MiB | 4.4 MiB | 6.1 MiB |
| Peak memory at start | 14.0 MiB | not measured | 12.3 MiB | not measured | not measured |
| Time to first output (warm) | ≈ 35 ms | not measured | ≈ 20 ms | not measured | not measured |
| Crates built (unique, normal deps, incl. workspace) | 229 (230 now, with `wiki-reader-media`) | 142 | not measured | not measured | Go, n/a |

What this says: wiki-reader's binary is about 27 % larger than markdown-reader's on disk (16.7 against 13.1 MiB) and uses about 14 % more memory at start (14.0 against 12.3 MiB). Both start in well under a tenth of a second once warm; the first run after a fresh build took 0.57 s for wiki-reader, so treat start-up as indicative only. The release downloads are about 12 % apart (6.7 against 6.0 MiB).

Where the extra size comes from: the Mermaid image tier (`mermaid-rs-renderer`, `resvg`, an embedded font) was +6.7 MiB in P3-12c, the largest single step. Everything since alpha.5 (themes, nav position, the options window with `toml_edit`, both viewers) added about 0.3 MiB in total. Subtracting the image step gives roughly 10 MiB, which is below markdown-reader's 13.1 MiB. The lite column is now a build, not an estimate: [P3-19](../roadmap/phase-3-alpha.md) measured 8.4 MiB, below markdown-reader (see the next section). markdown-reader also draws Mermaid as terminal images (Kitty, Sixel, iTerm2), so the two binaries carry comparable image stacks and the comparison is like for like on that feature.

Not measured: treemd and glow memory and start-up (not installed; only the published archive sizes were read), md-tui and Frogmouth (no comparable download; Frogmouth is Python), and memory while viewing a large Mermaid diagram or many images. Treat the table as one machine and one collection, not a benchmark.

## Lite build (P3-19)

Measured 2026-10-03 on macOS arm64 (Apple M2 Max, 12 cores, rustc 1.98.1), default `release` profile, the same checkout for both builds: base commit `0517adc` plus the uncommitted P3-19 working tree (the commit will differ). The lite build is `--no-default-features`, which drops `image`, `resvg`, `mermaid-rs-renderer` and `ratatui-image` (asserted by `scripts/check.sh` and CI).

Method, per variant, each in its own empty `CARGO_TARGET_DIR`, nothing else running:

```bash
cargo build --locked --release -p wiki-reader                          # full
cargo build --locked --release -p wiki-reader --no-default-features    # lite
```

- **Clean:** the first build into an empty target directory (all dependencies compiled), one run each.
- **Warm:** `cargo clean --release -p wiki-reader` then the same build, dependencies already cached, three runs each. A second row also cleans `wiki-reader-render`, `wiki-reader-media` and `wiki-reader-core` first. (`cargo clean -p` without `--release` removes the dev profile only and times a no-op.)
- **Size:** `ls -l` on `target/release/wiki-reader`, not stripped.

| | full | lite | lite vs full |
|---|---|---|---|
| Binary | 17,417,296 B (16.6 MiB) | 8,856,384 B (8.4 MiB) | −49 % |
| Clean release build | 50.7 s | 19.9 s | −61 % |
| Warm, `wiki-reader` only | 2.9–3.2 s (≈ 3.0) | 2.6–2.7 s (≈ 2.6) | −13 % |
| Warm, all four workspace crates | 6.4–6.7 s (≈ 6.6) | 5.4–5.7 s (≈ 5.5) | −17 % |
| Crates in the normal dependency tree | 230 | 142 | −88 |

The earlier rough estimate for a build without the image tier was about 10 MiB; the measured lite binary is 8.4 MiB, below markdown-reader's 13.1 MiB. The clean-build win is large because the four dependencies and their trees are most of the compile time; warm builds barely differ because only workspace code is rebuilt. The lite binary contains no `resvg`, `mermaid_rs_renderer` or `ratatui_image` symbols or strings.

**Boundary.** A plain `cfg` boundary was enough, no trait: `wiki-reader-media` has a `raster` feature (decode, SVG and Mermaid raster, the embedded font), `wiki-reader-render` forwards it as `media` and falls back to placeholders and the text tier when it is off, and the `wiki-reader` TUI gates its `ratatui-image` probe, decode worker, drawing and viewer on `media`. The spike found no place where a second implementation behind a trait would be needed.

Caveats: a single clean run per variant, one machine, and an tree.

## Viewing cost (P3-31)

Measured on **v0.1.8** (P3-31 / P3-31b); V4 linear wrap numbers updated after **v0.1.9**.

Measured 2026-10-05 on macOS arm64 (Apple silicon, darwin 26.5.2), `rustc` 1.98.1, wiki-reader on `main` at `5b8351e` (P3-31) with P3-31b harness top-up, **release** profile. Method: deterministic tempfile fixtures from `tui/app/perf_baseline.rs` (`FULL` scale: 5k pages, 10k-row table, 50k-line code, 1 MB token, 100/200/400 KB token curve, 30 diagrams, 500 links, 200 related pages, 200 media placeholders); `App::for_tests` + `ratatui::TestBackend` (excludes terminal diff/flush); medians of warm `Instant` samples; RSS via `ps -o rss=`. Harness is `#[ignore]` — CI only compiles it and runs the MINI determinism test.

```bash
cargo test -p wiki-reader --locked --release -- --ignored --nocapture viewing_cost_baseline
```

Fixture fingerprint (both P3-31b runs): `58fa6358b41688b2`. Second run medians within about 10 % of the first on timed rows (absolute ms).

| Metric | Run 1 | Run 2 | Notes |
|---|---:|---:|---|
| open_index_5k (ms) | 235.3 | 262.2 | includes 200 related pages |
| frame_large_tree_expanded (ms) | 0.88 | 0.90 | nav fully expanded |
| open_link_heavy (ms) | 4.56 | 4.64 | |
| open_huge_token (ms) | 26427 | 26482 | V4 — quadratic wrap |
| open_token_100k (ms) | 281 | 285 | V4 scale |
| open_token_200k (ms) | 1105 | 1119 | V4 scale |
| open_token_400k (ms) | 4439 | 4471 | V4 scale |
| open_diagrams (ms) | 2.50 | 2.57 | text-tier fences |
| open_long_code (ms) | 15.8 | 15.6 | |
| open_big_table (ms) | 28.7 | 30.5 | |
| open_media_heavy (ms) | 7.18 | 7.30 | V13 `push_media` (200 images) |
| frame_link_heavy (ms) | 1.53 | 1.52 | |
| width_relayout_diagrams (ms) | 1.62 | 1.60 | `ensure_layout_width` |
| diagram_ready_relayout (ms) | 1.09 | 1.05 | V2 `relayout_after_diagram_size` |
| width_relayout_token_400k (ms) | 8707 | 8725 | V4 resize cost |
| search_keystroke_flush (ms) | 5.53 | 5.65 | V10 Content search burst+draw |
| match_spans (ms) | 0.007 | 0.007 | V11 |
| search_next_match_draw (ms) | 0.93 | 0.93 | V11 |
| reindex_ui_identical (ms) | 5.42 | 5.37 | |
| reindex_ui_changed (ms) | 16.9 | 16.1 | at 16 ms budget |
| table_want (ms) | 4.27 | 4.17 | 10k-row `want()` |
| table_G_plus_draw (ms) | 2.46 | 2.39 | includes `keep_cursor_visible` |
| table_PgDn_plus_draw (ms) | 1.08 | 1.11 | |
| rss_kb_after_5k_open | 42224 | 42144 | ≈ 41 MiB |
| rss_kb_end | 138736 | 135136 | after heavy pages |

Proposed budgets (documented only, not asserted in CI): frame build ≤ 16 ms p95, keypress→frame ≤ 50 ms, width relayout ≤ 50 ms, UI-thread reindex ≤ 16 ms.

**Caveats.** Frame timings use `TestBackend`, so they exclude terminal diff and flush. V14 was dropped at 10k rows only (the loop is O(rows); ≈25 ms at 100k).

**Not measured in-process:** Kitty pan upload bytes (V15). Idle CPU / wakeups (V7/V8) measured on installed v0.1.8 under a PTY: **0.0 %CPU**, idlew **0** (images on/off) — dropped.

**One-off tooling (not committed to config):**

- `cargo audit`: warnings only — `bincode` 1.3.3 (`RUSTSEC-2025-0141`), `ttf-parser` 0.25.1 (`RUSTSEC-2026-0192`), both unmaintained.
- `cargo deny`: **not run** (`cargo-deny` not installed; install deferred to operator).
- Clippy with `-W clippy::perf -W clippy::nursery -W clippy::unwrap_used -W clippy::expect_used -W clippy::redundant_clone -W clippy::needless_pass_by_value`: ≈1121 warnings; dominant are test `unwrap`/`expect`; 7 `redundant_clone` (e.g. `nav/session.rs`, `rendered_doc.rs`, tests).

Register re-rank: [audit-v0.1.8.md](../roadmap/audit-v0.1.8.md).

### After P3-34 (linear wrap (P3-34, shipped v0.1.9))

Same machine / fingerprint `58fa6358b41688b2`, two release runs after rewriting wrap helpers:

| Metric | Before (P3-31b) | After run 1 | After run 2 |
|---|---:|---:|---:|
| open_huge_token (ms) | 26427 / 26482 | 8.25 | 8.09 |
| open_token_100k (ms) | 281 / 285 | 4.53 | 4.40 |
| open_token_200k (ms) | 1105 / 1119 | 4.99 | 4.94 |
| open_token_400k (ms) | 4439 / 4471 | 5.91 | 6.07 |
| width_relayout_token_400k (ms) | 8707 / 8725 | 5.97 | 6.00 |

Target ≪ 100 ms open; width relayout budget ≤ 50 ms — both met.

## Related

- [Prior art & libraries](prior-art-and-libs.md)
- [Audit of v0.1.8](../roadmap/audit-v0.1.8.md)
