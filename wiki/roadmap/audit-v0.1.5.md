---
id: WR-ROADMAP-AUDIT-V015
title: Audit of v0.1.5
summary: Findings from a pre-0.1.5 audit and a static re-audit of v0.1.5 (crashes, performance, memory, robustness, supply chain), with severity, status and the PR that fixes each.
status: active
updated: 2026-10-05
related: [phase-3-alpha, dogfood-log, audit-v0.1.8]
---

# Audit of v0.1.5

Staged viewing, performance and supply-chain leftovers from this register continue under V-IDs in the [v0.1.8 audit](audit-v0.1.8.md).

Two audits feed this register. **E-findings** came from an audit done before v0.1.5 (build, clippy, 681 tests, targeted probes). **N-findings** come from a static re-audit of v0.1.5: the code new in 0.1.5, the core crates and OS touchpoints, and the TUI layer, checked against a Rust anti-pattern list (blind `unwrap`/`expect`, over-cloning, allocation in loops, boolean arguments, catch-all error hiding). The re-audit read code only: nothing was built or probed, so each N-finding is "traced, not run" unless the table says otherwise. Four were re-read and confirmed in the code (N1 to N4).

No `unsafe` Rust exists in the workspace (`unsafe_code = "forbid"` in every crate); that says nothing about third-party dependencies. Line numbers drift, so find items by symbol.

**Shipped in v0.1.6:** every `fixed` and `fixed in part` item below that cites P3-27 or P3-28. **Shipped in v0.1.7:** P3-29 (N8, N14, N15, N16, N20 and L1–L5 in part). **Shipped in v0.1.8:** P3-30 (table-viewer expand). **Shipped in v0.1.9:** V4 / L6 quadratic wrap ([audit-v0.1.8](audit-v0.1.8.md) P3-34). **Status values:** `fixed` (merged or in an open PR), `planned` (next PR), `staged` (recorded, scheduled for a later release), `dropped` (measured negligible — see v0.1.8 register). Tasks: [P3-27](phase-3-alpha.md) (crash and terminal fixes), [P3-28](phase-3-alpha.md) (performance) and [P3-29](phase-3-alpha.md) (robustness); viewing leftovers continue as V-IDs in [audit-v0.1.8](audit-v0.1.8.md).

## High: crashes and terminal corruption

| ID | Where | Finding | Status |
|---|---|---|---|
| N1 | `tui/app/draw.rs` `highlight_spans` | Query found in a lowercased copy, then the original sliced with those byte offsets: `İ` or the Kelvin sign makes the search overlay panic every frame. | fixed (P3-27): `search::find_case_insensitive` |
| N2 | `app/mod.rs` `help_activate` | Help "Open in editor" ran `$EDITOR` on the live raw-mode TUI. | fixed (P3-27): `update` only requests it, the event loop suspends the terminal |
| N3 | `app/events.rs` panic hook | The hook restored the terminal for a panic on any thread; worker panics corrupted the screen. | fixed (P3-27): main-thread-only restore, worker messages printed after exit |
| N4 | `keymap.rs` overrides | `[keys]` overrides fired in search, Help, Options and confirm modes; `len()` compared bytes. | fixed (P3-27): Normal mode only, compared by char |
| E7 | core `search.rs` `snippet_line` | Snippet offsets taken from the lowercased line. | fixed (P3-27), with N1 |

## High: performance (pre-0.1.5 audit)

| ID | Where | Finding | Status |
|---|---|---|---|
| E1 | core `parse.rs` `unique_slug` | O(n²) for repeated headings (4,000 identical: 2.8 s in a debug probe). | fixed (P3-28): `SlugAllocator` keeps a per-base counter |
| E2 | core `search.rs`, `app/mod.rs` | Full-text search runs synchronously per keystroke; N5 below adds the paste and cap details. | fixed in part (P3-28): one refresh per event batch, hit cap, cached file count; off-thread search staged |
| E3 | `app/mod.rs` highlight | Detached threads are not cancelled. | fixed (P3-28): one persistent worker, newest request wins, cancelled between lines |

## Medium

| ID | Where | Finding | Status |
|---|---|---|---|
| N5 | search overlay | No bracketed paste, up to 256 events per redraw each with a full refresh, no hit cap, a per-frame `HashSet`. | fixed (P3-28): batching, `MAX_TEXT_HITS = 500` with a "500+" footer, no per-frame `HashSet` |
| N6 | core `NavTree::prev/next` | `page_order()` clones every key per call; 4 to 6 whole-collection clones per frame. | fixed (P3-28): `NavTree` builds the page order and index once; `focus_list` is no longer rebuilt for the status line |
| N7 | image workers | A worker panic left `busy()` true forever; `Disconnected` was ignored. | fixed (P3-27): `worker::guarded`, disconnect handling |
| N8 | `image_viewer.rs`, `raster.rs` | One large bitmap peaks near 300 MB. | fixed (P3-29): `into_rgba8` retention + 2× pixel-byte budget |
| N9 | `image_viewer.rs` | After a zoom-time error, zoom keys did nothing. | fixed (P3-27) |
| N10 | `code_viewer.rs` | Width rescanned per frame, `slice_line` is O(left + width) per row. | fixed (P3-28): width measured once, column maths saturates |
| N11 | `table_viewer.rs` | Widths rebuilt per row per frame, sort and filter allocate per comparison. | fixed (P3-28): one layout per frame, sort keys cached per column, filter without allocation |
| N12 | `app/mod.rs` load and reindex | Failed `load_page` leaves a stale doc; any change reloads the page and clobbers selection. | fixed in part (P3-28): a failed read shows the removed state; an unchanged index reloads nothing. A real change still reloads the page and resets selection and focus |
| N13 | `events.rs`, `images.rs` | Relayout storm per diagram completion and per width change. | dropped after measurement (V2 in [audit-v0.1.8](audit-v0.1.8.md)) |
| N14 | core `parse.rs` | YAML alias amplification is only partly bounded. | fixed (P3-29): frontmatter size + token-start alias-marker caps |
| N15 | core `provider.rs` | One unreadable directory is fatal; no file-size or page-count caps. | fixed (P3-29): skip walk errors with diagnostics; `MAX_PAGES` / `take(MAX_PAGE_BYTES)` (above the page cap, which pages survive follows FS walk order) |
| N16 | core `config.rs` | Config write is not atomic; inline tables fail. | fixed (P3-29): canonicalize symlink target, copy perms, sync_all, rename (inline tables still unpatched) |
| N17 | session save | Failed save retried every tick, tmp files left, history unbounded, no flush after loop errors. | fixed in part (P3-27): backoff, message, tmp cleanup, flush after the loop; history cap staged |
| N18 | `release.yml` | Write token present during dependency build scripts. | staged |
| N19 | `wiki-reader-tools` `check_doc` | Link rules differ from the reader; non-`NotFound` errors abort the check. | staged |
| N20 | render | No control-character sanitising, so copied text can differ from what is shown. | fixed (P3-29): `sanitize_controls` on display spans, code body / CopyCode, table viewer, and bidi `Cf` |

## Low and code quality

| ID | Finding | Status |
|---|---|---|
| E4 | Diagram text cache is unbounded. | staged → see V16 in [audit-v0.1.8](audit-v0.1.8.md) |
| E5 | Synchronous SVG measure; font DB rebuilt per call. | staged → see V6 in [audit-v0.1.8](audit-v0.1.8.md) |
| E6 | `render_with` clones `ParsedPage`. | staged → see V12 in [audit-v0.1.8](audit-v0.1.8.md) (dropped after measurement) |
| E8 | Diagram cache hit drops the fallback reason. | staged → see V16 in [audit-v0.1.8](audit-v0.1.8.md) |
| L1 to L5 | Media: zoom size drift, premultiplied alpha, uncancelled workers, unbounded worker file read, no Mermaid size cap. | fixed in part (P3-29): bitmap retention budget (N8); remaining media items stay staged |
| L6 | `push_media` is O(n²); code and table text held several times; quadratic wrap on a long token (data URI). | fixed (V4, v0.1.9): linear wrap; `push_media` O(n²) dropped as V13 |
| L7 | Tabs not expanded in code; zero-width characters counted as one column. | staged |
| L8 | `nav_order` NaN or infinity broke the sort order. | fixed (P3-27): rejected at parse, `total_cmp` |
| L9 | `Recording*` test doubles compiled into release builds. | fixed (P3-27): `#[cfg(test)]`, unused `RecordingOpener` removed |
| L10 | Opener children never reaped; herdr CLI has no timeout; signal handlers registered late, no SIGINT. | staged |
| L11 | Boolean-argument functions (`reload_page_keeping_view_ex`, `App::build`, `Theme::effective_name`, `viewer::draw`); silent `let _ =` on opener, signal and worker failures. | staged |
| L12 | Event loop redraws every tick with no dirty flag; `recv_prefer_decode` polls every 20 ms. | dropped after measurement (V7/V8 in [audit-v0.1.8](audit-v0.1.8.md); V8 still needs Ghostty/herdr confirm) |
| L13 | `nav_scroll` is `u16`; `store_search_matches` is O(hits × lines); `focus_list` allocates per frame. | staged |
| L14 | `code_viewer` `u16` overflow past about 65,500 columns (fixed in P3-28); `status::ALL` is not checked against the enum (staged). | fixed in part (P3-28) |
| L15 | Supply chain: no `deny.toml`, `cargo audit`, Dependabot or `SECURITY.md`; actions pinned by tag; `bincode 1.3.3` advisory unverified. | staged |

## Checked and sound

`FsProvider::read` path checks; link resolution (index-only, cannot leave the root); the image policy of [ADR-0017](../decisions/0017-static-local-images-only.md) (canonicalise, root check, size and pixel caps, no external SVG references); the config trust split (a collection file cannot set `opener`, `editor` or `keys`); the URL opener (scheme allowlist, confirm, no shell); editor launch; herdr argument cleaning; the OSC 52 size cap; `u16` layout arithmetic at tiny sizes.

## Related

- [Audit of v0.1.8](audit-v0.1.8.md)
- [Phase 3](phase-3-alpha.md)
- [Dogfood log](dogfood-log.md)
