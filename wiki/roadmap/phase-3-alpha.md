---
id: WR-ROADMAP-P3
title: Phase 3 — Alpha polish
summary: Diagrams and images first, then themes, sticky headers, nav chrome, and early herdr niceties.
status: active
updated: 2026-10-02
related: [phase-2-mvp, phase-4-beta]
nav_order: 3
---

# Phase 3 — Alpha polish

Phase 1 is closed and Phase 2 is feature complete (dogfood hold until ≈ 2026-10-13, fixes only; see [phase-2-mvp.md](phase-2-mvp.md)). Phase 3 was activated on 2026-10-01. Order is **reader value first**: the spec's biggest remaining gap is images and diagrams, so P3-12 leads. Install (P3-08) already shipped.

## Proposed order

1. **P3-12 Images and diagrams** — split into four steps, each its own PR:
   1. **P3-12a Spike** — does `ratatui-image` detect the right protocol in Ghostty and inside a herdr pane (`HERDR_ENV=1`, `terminal.kitty_graphics`)? Its startup query must run before our raw-mode/kitty-keyboard setup, and herdr may not answer it. Output: an accepted [ADR-0004](../decisions/0004-diagram-rendering.md) with the detection rule.
   2. **P3-12b Image slots and local images** — the renderer reserves N rows for an image, the TUI draws into them, and the text placeholder is the fallback. Policy is [ADR-0017](../decisions/0017-static-local-images-only.md): static, inside the collection, never fetched. This step builds the hard parts (scroll clipping, clearing under the Help/Search popups and on tab/page switch) with the simplest content.
   3. **P3-12c Mermaid image tier** — `mermaid-rs-renderer` → `resvg` → PNG into the same slots, off the UI thread, cached by content/background (width is a layout-time legibility check); embedded font so output is deterministic; falls back to the text tier.
   4. **P3-12d `diagrams` config and tier selection** — `auto | image | text | source`, tmux → text, never Sixel under herdr. Landed with P3-12c in the same PR.
2. **P3-07 Themes / P3-11 Layout config** — wire the stored `theme` config key to the semantic token table in `theme.rs`; ship dark/light presets plus a herdr-matching preset; add nav placement (left/right) and related layout options. (R35 already warns when the key is set but inert.)
3. **P3-01 / P3-02 / P3-04** — sticky section header, side nav header/footer regions, optional ‹ › header buttons.
4. **P3-05 / P3-03** — link hover preview; nav label options (R35 already draws a dim `(filename)` suffix for `title+filename`; full “alt text below” remains here).
5. **P3-13 / P3-14 / P3-15** — three requests from 2026-10-02 dogfood, in this order. **P3-13 Options window** needs P3-07 (theme) and P3-11 (nav position), both config-key owners, so it follows them; it is also where the light and herdr presets finally get their visual pass. **P3-14 Table viewer** and **P3-15 Image and diagram viewer** are the full-screen counterparts of the inline "expand" block actions (BA); they share one modal-viewer shell (focus, `Esc`, scroll, the image → `Clear` → popup draw order from [ADR-0004](../decisions/0004-diagram-rendering.md)), so build the shell once in P3-14 and reuse it in P3-15.
6. **P3-09 / P3-10** — herdr integration (plugin pane; publish current page to herdr sidebar) after the herdr API is confirmed on a real install.

Detail each row into spikes/acceptance only when it is next up. Dogfood bites recorded in [phase-2-mvp.md](phase-2-mvp.md) still feed this list.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | done | daf8fc5; hygiene f90c378; release hardening 9012fce; [v0.1.0-alpha.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.1); [v0.1.0-alpha.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.2); [v0.1.0-alpha.3](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.3); [v0.1.0-alpha.4](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4); [v0.1.0-alpha.4.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4.1); [v0.1.0-alpha.5](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.5) (P3-12c/d Mermaid image tier) |
| P3-06 | Help overlay with clickable keys | | done | done via P2-20 (#71) |
| P3-07 | Themes | | done | `theme = dark \| light \| herdr` wired to `Theme::from_name`; light preset with a contrast test; syntect theme and Mermaid palette follow the preset (diagrams were always drawn light before); herdr preset is vesper-based; quadrant/xy-chart/ER-key fills stay light (renderer hard-codes them). Checked 2026-10-02: Mermaid fixtures look right in the default (dark) theme. The light and herdr presets are covered by the contrast tests only; their visual pass waits for the planned options window, which will switch theme (and the other planned config options) from inside the app |
| P3-11 | Layout and theme config | C1 | todo | nav placement (left/right) and other layout options; owns config keys (nav width stays in session for P2-14); feature request from 2026-09-30 dogfood. The formatted view was removed ([ADR-0014](../decisions/0014-remove-formatted-view-toggle.md)), so it owns no view-mode key |
| P3-12 | Image diagrams and images (D1) | D1 | done | was P2-24b; umbrella for P3-12a…d; closed with P3-12d (landed with 12c) |
| P3-12a | Spike: protocol detection in Ghostty and herdr; accept ADR-0004 | D1 | done | [P3-S1](spikes/p3-s1-image-protocol.md): Kitty works direct/Herdr; iTerm2 must be forced from env; unknown terminals are not probed; crop/clear/swap pass; Mermaid mixed-go (12 valid blocks/6 common types pass, wide graphs fall back); ADR-0004 accepted |
| P3-12b | Image slots + local images (static, in-collection, never fetched) | D1 | done | [ADR-0017](../decisions/0017-static-local-images-only.md); implemented: core path policy and caps, `ImageSlot` rows, `ImageManager` decode worker, startup probe, draw pass with top/bottom crop, `[image: alt]` fallback, `fixtures/images`. SVG moved to P3-12c with `resvg`. Manual check 2026-10-02 (`cargo run -p wiki-reader` at the repo root): pictures display in Ghostty, in a herdr pane and in iTerm2; Terminal.app and tmux show the `no graphics protocol` placeholder, as designed. Scroll-crop and popup behaviour were not itemised in the report; hardening: visible-only queue, LRU cache, failure cache |
| P3-12c | Mermaid image tier | D1 | done | `mermaid-rs-renderer` → `resvg` → slots; off-thread size cache `(hash, bg)` + re-layout (preserves selection); SVG-only measure, raster on decode; Decode before Measure; embedded Noto Sans; legibility gate ≥ 0.55 → text with reason; SVG local images; release binary **+~6.7 MiB** (9.7 → 16.4 MiB, measured 2026-10-02 vs P3-12b). Manual check 2026-10-02 (`cargo run -p wiki-reader fixtures/mermaid`): Ghostty launches and shows the collection; tmux stays on the text tier (ASCII diagram, no image slots); automated coverage for text→image swap, Help overlay, selection-survives-relayout, TooWide/Failed reasons, Decode-before-Measure. herdr/iTerm2/Terminal.app probe behaviour unchanged from P3-12b matrix |
| P3-12d | `diagrams` config + tier selection | D1 / C1 | done | landed with P3-12c: `auto \| image \| text \| source`; tmux → text even for `image`; `text`/`source` skip probe; herdr Kitty-only (never Sixel); fallback reasons in tier headers |
| P3-01 | Sticky viewer section header | U1 | todo | |
| P3-02 | Side nav header/footer sub-regions | U2 | todo | |
| P3-04 | Optional header ‹ › buttons | U4 | todo | |
| P3-05 | Link hover preview popover | W6 | todo | |
| P3-03 | Nav label options | U3 | todo | dim suffix partial via R35 |
| P3-09 | herdr: launch as a herdr plugin pane | | todo | after real herdr API |
| P3-10 | herdr: publish the current page to herdr's sidebar | | todo | after real herdr API |
| P3-13 | Options window | U5 | todo | feature request 2026-10-02. A popup like markdown-reader's, opened by a key and a footer/header button, that edits config from inside the app: **theme** selector (dark / light / herdr, live preview), **nav position** (left / right, P3-11), **nav labels** (titles ↔ filenames, `nav.labels`), **Mermaid** settings (`diagrams` tier: auto / image / text / source), **image** settings. Needs: a write path for config (today it is read-only; decide user file vs `--config`, keep comments and unknown keys), live apply without restart, and **new config keys for images** (none exist yet; candidates: enable/disable, max slot rows). Invalid values keep the earlier-file-wins rule from P3-07. Open: which keys persist vs session-only (nav width is session-only today, P2-14) |
| P3-14 | Table viewer | U6 | todo | feature request 2026-10-02. Open a large table in a modal / window (like markdown-reader): scroll in both axes with a fixed header row and first column, **filter** rows by text, **sort** by a column, copy a cell or row (reuse the tab-separated table copy from P2-R38). Entry point: the existing "expand table" block action (BA), plus `Enter` on a focused table. Also the shell for P3-15. The inline expand stays as the quick path |
| P3-15 | Image and diagram viewer | U7 | todo | feature request 2026-10-02. Open an image or Mermaid diagram full size in a modal: **zoom** in/out and **pan** with keys and mouse when the picture is larger than the window (a detailed or wide diagram is cut to a slot of at most 30 rows and a legibility gate falls back to text; this removes that limit on demand). Reuses the P3-12 pipeline (`SlotSource`, `resvg` raster, palette) at a larger pixel budget, decoded off-thread and evicted on close; text-tier users get the source view. Entry point: `Enter` on a focused slot / the expand-diagram action. Needs the pixel and file-size caps from [ADR-0017](../decisions/0017-static-local-images-only.md) to apply at zoom, and a decision on whether zoom re-rasterises SVG/Mermaid (sharp) or scales the bitmap |

## Related

- [Phase 2](phase-2-mvp.md)
- [Integrations](../architecture/integrations.md)
