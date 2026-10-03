---
id: WR-ROADMAP-P3
title: Phase 3 — Alpha polish
summary: Diagrams and images first, then themes, sticky headers, nav chrome, and early herdr niceties.
status: active
updated: 2026-10-03
related: [phase-2-mvp, phase-4-beta, dogfood-log]
nav_order: 3
---

# Phase 3 — Alpha polish

Phase 1 is closed and Phase 2 is feature complete (dogfood hold until ≈ 2026-10-13, fixes only; see [phase-2-mvp.md](phase-2-mvp.md)). Phase 3 was activated on 2026-10-01. Images, diagrams, themes, nav position, the options window and the table/image viewers have shipped; next is cheap chrome, a lite install, then viewer polish. Dogfood bites live in the [dogfood log](dogfood-log.md).

## Proposed order

**Done:** P3-08 install; P3-06 Help; P3-12a–d images and diagrams; P3-07 themes / P3-11 layout; P3-13 options window; P3-14 table viewer; P3-15 image and diagram viewer.

1. **P3-16** — Nav label modes: drop `title+filename`; `filename` shows real file-system names (folds into P3-03).
2. **P3-17** — Chrome pass: tab titles end in `.md`; tabs and footer link buttons get top and bottom borders; active tab keeps a darker selection when nav is focused (extends P3-01/02/04).
3. **P3-18** — Footer buttons: ⚙ moves from the header to the footer's bottom right, with a `?` help button to its left (part of P3-02).
4. **P3-19** — Lite build: move image/SVG/Mermaid-raster into `wiki-reader-media` behind a default-on cargo feature (before the viewer rework so that code lands once).
5. **P3-20** — Viewer sizing: content-sized windows capped at today's size; fit / actual-size toggle.
6. **P3-21** — Image/diagram carousel (Tab / Shift+Tab).
7. **P3-22** — Diagram state toggle (image / text / source, session-only).
8. **P3-23** — Docs refresh once the UI settles (screenshots, README ASCII, carousel keys).
9. **P3-09 / P3-10** — herdr integration (plugin pane; publish current page to herdr sidebar) after the herdr API is confirmed on a real install.

Remaining table todos (P3-01, P3-04, P3-05) stay until detailed; P3-03 and P3-02 are advanced by P3-16 and P3-18. Detail each row into spikes/acceptance only when it is next up.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | done | daf8fc5; hygiene f90c378; release hardening 9012fce; [v0.1.0-alpha.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.1); [v0.1.0-alpha.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.2); [v0.1.0-alpha.3](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.3); [v0.1.0-alpha.4](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4); [v0.1.0-alpha.4.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4.1); [v0.1.0-alpha.5](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.5) (P3-12c/d Mermaid image tier); [v0.1.0-alpha.5.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.5.1) (P2-55 copy path); [v0.1.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.1) (P3-07/11/13/14/15, the first `v0.1.N` tag) |
| P3-06 | Help overlay with clickable keys | | done | done via P2-20 (#71) |
| P3-07 | Themes | | done | Reworked 2026-10-02 (local time; [ADR-0019](../decisions/0019-theme-presets.md) retains its immutable UTC date, 2026-10-03): `dark` / `light` take the luckgrid.net palette, `herdr` follows the theme named in herdr's config and is the default inside herdr; live follow is P4-05. Original: `theme = dark \| light \| herdr` wired to `Theme::from_name`; light preset with a contrast test; syntect theme and Mermaid palette follow the preset (diagrams were always drawn light before); herdr preset is vesper-based; quadrant/xy-chart/ER-key fills stay light (renderer hard-codes them). Checked 2026-10-02: Mermaid fixtures look right in the default (dark) theme. Light and herdr presets got their in-app visual pass with the options window (P3-13) |
| P3-11 | Layout and theme config | C1 | done | `nav.position = left \| right` (default left); divider on the inner edge; Viewer ←/→ at the edge toward the nav hands focus over. Nav width stays session-only (P2-14). No other layout keys (formatted view removed, [ADR-0014](../decisions/0014-remove-formatted-view-toggle.md)). Feature request from 2026-09-30 dogfood |
| P3-12 | Image diagrams and images (D1) | D1 | done | was P2-24b; umbrella for P3-12a…d; closed with P3-12d (landed with 12c) |
| P3-12a | Spike: protocol detection in Ghostty and herdr; accept ADR-0004 | D1 | done | [P3-S1](spikes/p3-s1-image-protocol.md): Kitty works direct/Herdr; iTerm2 must be forced from env; unknown terminals are not probed; crop/clear/swap pass; Mermaid mixed-go (12 valid blocks/6 common types pass, wide graphs fall back); ADR-0004 accepted |
| P3-12b | Image slots + local images (static, in-collection, never fetched) | D1 | done | [ADR-0017](../decisions/0017-static-local-images-only.md); implemented: core path policy and caps, `ImageSlot` rows, `ImageManager` decode worker, startup probe, draw pass with top/bottom crop, `[image: alt]` fallback, `fixtures/images`. SVG moved to P3-12c with `resvg`. Manual check 2026-10-02 (`cargo run -p wiki-reader` at the repo root): pictures display in Ghostty, in a herdr pane and in iTerm2; Terminal.app and tmux show the `no graphics protocol` placeholder, as designed. Scroll-crop and popup behaviour were not itemised in the report; hardening: visible-only queue, LRU cache, failure cache |
| P3-12c | Mermaid image tier | D1 | done | `mermaid-rs-renderer` → `resvg` → slots; off-thread size cache `(hash, bg)` + re-layout (preserves selection); SVG-only measure, raster on decode; Decode before Measure; embedded Noto Sans; legibility gate ≥ 0.55 → text with reason; SVG local images; release binary **+~6.7 MiB** (9.7 → 16.4 MiB, measured 2026-10-02 vs P3-12b). Manual check 2026-10-02 (`cargo run -p wiki-reader fixtures/mermaid`): Ghostty launches and shows the collection; tmux stays on the text tier (ASCII diagram, no image slots); automated coverage for text→image swap, Help overlay, selection-survives-relayout, TooWide/Failed reasons, Decode-before-Measure. herdr/iTerm2/Terminal.app probe behaviour unchanged from P3-12b matrix |
| P3-12d | `diagrams` config + tier selection | D1 / C1 | done | landed with P3-12c: `auto \| image \| text \| source`; tmux → text even for `image`; `text`/`source` skip probe; herdr Kitty-only (never Sixel); fallback reasons in tier headers |
| P3-01 | Sticky viewer section header | U1 | todo | |
| P3-02 | Side nav header/footer sub-regions | U2 | todo | advanced by P3-18 (footer ⚙ / `?`) |
| P3-04 | Optional header ‹ › buttons | U4 | todo | |
| P3-05 | Link hover preview popover | W6 | todo | |
| P3-03 | Nav label options | U3 | todo | dim suffix partial via R35; further work is P3-16 |
| P3-09 | herdr: launch as a herdr plugin pane | | todo | after real herdr API |
| P3-10 | herdr: publish the current page to herdr's sidebar | | todo | after real herdr API |
| P3-13 | Options window | U5 | done | feature request 2026-10-02. A popup like markdown-reader's, opened by a key and a footer/header button, that edits config from inside the app: **theme** selector (dark / light / herdr, live preview), **nav position** (left / right, P3-11), **nav labels** (titles ↔ filenames, `nav.labels`), **Mermaid** settings (`diagrams` tier: auto / image / text / source), **image** settings, **copy path** format (`copy.path`: relative / absolute, key added by P2-55). Implemented: `,` or the header ⚙ opens the overlay; writes via [ADR-0018](../decisions/0018-config-write-path.md); new `[images] enabled \| max_slot_rows`; live apply for every row (images re-enable after a disable needs a restart when the startup probe was skipped). Reworked 2026-10-02 (local time) after dogfood: grouped radio rows in the style of markdown-reader's settings window, `,` and `c` both open and close it, and the copy-path key is listed in Help as "Copy file path" |
| P3-14 | Table viewer | U6 | done | feature request 2026-10-02. Open a large table in a modal / window (like markdown-reader): scroll in both axes with a fixed header row and first column, **filter** rows by text, **sort** by a column, copy a cell or row (reuse the tab-separated table copy from P2-R38). Entry point: the existing "expand table" block action (BA), plus `Enter` on a focused table. Also the shell for P3-15. Implemented: `RenderedDoc::tables` (`DocTable`) from `render.rs`; `ExpandTable` block action and `Enter` open `tui/table_viewer.rs` inside `tui/modal_viewer.rs`. Keys: arrows / `hjkl`, `PgUp` / `PgDn`, `g` / `G`, `/` filter, `s` sort, `y` cell, `Y` row. See [UI spec: Modal viewers](../product/ui-spec.md#modal-viewers) |
| P3-15 | Image and diagram viewer | U7 | done | feature request 2026-10-02. Open an image or Mermaid diagram full size in a modal: **zoom** in/out and **pan**. Reuses the P3-12 pipeline at a larger pixel budget. Entry: `ExpandDiagram` / `Enter` on a slot. Keys: arrows / `hjkl` pan, `PgUp` / `PgDn`, `g` / `G`, `+` / `-` zoom, `0` reset. SVG/Mermaid re-rasterise at zoom; raster files scale from the bitmap. See [UI spec: Modal viewers](../product/ui-spec.md#modal-viewers). Deferred: mouse drag to pan, click on a picture to open it, carousel (P3-21), diagram state toggle (P3-22), viewer sizing (P3-20) |
| P3-16 | Nav label modes: drop `title+filename`; `filename` shows real file-system names | U3 | todo | dogfood 2026-10. Folders keep on-disk names (lowercase, hyphens). Files show the extension (`README.md`). Folds into P3-03. Needs a new ADR superseding the affected nav-label decisions in ADR-0008 and ADR-0013 (including the folder-name rule); accepted ADRs stay immutable. Settled migration: existing `title+filename` maps to `title` with a one-time warning. Touch `crates/wiki-reader-core/src/config.rs` (config parser), spec U3, configuration.md, ui-spec.md, content-model.md, the options window labels and snapshot tests |
| P3-17 | Chrome pass: tab titles end in `.md`; tabs and footer link buttons get top and bottom borders; active tab keeps a darker selection background when nav is focused | U1 / U2 / U4 | todo | dogfood 2026-10. Extends P3-01/02/04. `tabs.rs` currently uses `stem()`. Open design: tabs sit on the View's top border today, so closed boxes may need a 3-row strip — settle in ui-spec.md first |
| P3-18 | Footer buttons: ⚙ moves from the header to the footer's bottom right, with a `?` help button to its left | U2 / U8 | todo | dogfood 2026-10. Part of P3-02. Update the `?` row in the generated keymap docs and the Help window with the icon. The README ASCII diagram changes |
| P3-19 | Lite build: move image, SVG and Mermaid-raster code into its own crate (`wiki-reader-media`) behind a default-on cargo feature | U9 | todo | dogfood 2026-10. `cargo install --no-default-features` gives a build with no image, resvg, mermaid-rs-renderer or ratatui-image. Diagrams render as text through mermaid-text. `diagrams = image` falls back to text with a reason; the Images options group is hidden. Spike first: measure a build without the four heavy dependencies. Then record the cargo-feature-only distribution decision in a new ADR, add a CI check for both builds, document the lite install in README and configuration.md, and fill the lite column in [benchmarks](../architecture/benchmarks.md). No separate `-lite` release artifact; revisit only if a prebuilt lite binary is requested |
| P3-20 | Viewer sizing | U6 / U7 | todo | dogfood 2026-10. Table and image/diagram windows size to their content and stay capped at today's size. Wide pictures keep today's width. Add a fit / actual-size toggle on top of the existing zoom. Table viewer keeps in-view table styles (bold header, borders, link colour). Opening a picture should show it larger than its inline slot |
| P3-21 | Image/diagram carousel | U7 | todo | dogfood 2026-10. Tab / Shift+Tab step through every image and diagram in the document. The picture you opened shows first. Hints go in the modal hint bar, the Help window and ui-spec.md. Arrows stay pan |
| P3-22 | Diagram state toggle | U7 | todo | dogfood 2026-10. A key cycles image / text / source for the open diagram. Session-only. Builds on P3-21 |
| P3-23 | Docs refresh once the UI settles | — | todo | dogfood 2026-10. Retake `assets/wiki-reader.png` and the `wiki/assets/` copies (new theme, footer icons, borders). Redraw the README ASCII diagram. Add the carousel keys. Done last so screenshots are not retaken twice |

## Related

- [Phase 2](phase-2-mvp.md)
- [Dogfood log](dogfood-log.md)
- [Integrations](../architecture/integrations.md)
