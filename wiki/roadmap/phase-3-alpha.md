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

## Exit criteria

End with a recorded decision, not an automatic status change:

- Chrome, lite build and viewer polish (Batches A–C below) shipped and dogfooded from the installed binary.
- Default and lite builds pass CI; the lite binary excludes the four heavy media dependencies.
- herdr integration confirmed on a real install or explicitly deferred to Phase 4.
- P3-01/02/03/04/05 and P3-09/10 resolved: completed, explicitly deferred or dropped.
- Documentation and screenshots refreshed after the UI settles.
- No unresolved dogfood bites against shipped Phase 3 features; any accepted deferrals are recorded in the exit decision.

## Proposed order

**Done:** P3-08 install; P3-06 Help; P3-12a–d images and diagrams; P3-07 themes / P3-11 layout; P3-13 options window; P3-14 table viewer; P3-15 image and diagram viewer.

1. **P3-16** — Nav label modes: drop `title+filename`; `filename` shows real file-system names (folds into P3-03).
2. **P3-18** — Footer buttons: ⚙ moves from the header to the footer's bottom right, with a `?` help button to its left (part of P3-02).
3. **P3-17** — Chrome pass: tab titles end in `.md`; tabs and footer link buttons get top and bottom borders; active tab keeps a darker selection when nav is focused. P3-18 lands first so footer geometry changes once.
4. **P3-19** — Lite build: move image/SVG/Mermaid-raster into `wiki-reader-media` behind a default-on cargo feature (before the viewer rework so that code lands once).
5. **P3-20** — Viewer sizing: content-sized windows capped at today's size; fit / actual-size toggle.
6. **P3-21** — Image/diagram carousel (Tab / Shift+Tab).
7. **P3-22** — Diagram state toggle (image / text / source, session-only).
8. **P3-23** — Docs refresh once the UI settles (screenshots, README ASCII, carousel keys).
9. **P3-09 / P3-10** — herdr integration only after API confirmation. Run a half-day spike after Batch A, alongside Batch B; record API evidence and a confirm/defer decision. P4-05 shares this dependency.

### Batches and dependencies

- **A — chrome:** P3-16 (medium), P3-18 (small), P3-17 (medium). Intended next release: v0.1.3. Approved layout: dedicated three-row strips for fully closed tabs and footer controls; reserve the separate layout footer's right side for `?` / ⚙ before status text. Specify collision and short-terminal behavior before P3-17 coding. Snapshots at 40/60/80/120 columns, the full gate and Ghostty/herdr manual checks close the batch.
- **B — lite:** P3-19 (large). Spike before extraction; compare full/lite binary size and clean/warm build time on the same commit, profile and machine. Record methodology and results in [benchmarks](../architecture/benchmarks.md). Land extraction without behavior changes, then feature gating, then CI/docs. A small `cfg` boundary is sufficient unless the spike justifies a trait; keep terminal protocols in the TUI. Design a tier-independent document media inventory during this seam work so text/source diagrams and placeholders are not lost by the carousel.
- **C — viewers:** P3-20 (medium–large), P3-21 (medium after inventory), P3-22 (medium). Intended following release: v0.1.4. P3-20 must define natural-pixel actual size, small-image enlargement, per-item carousel sizing and existing pixel caps. Table inline styling needs richer data: `DocTable` currently flattens styles/links into strings. P3-21 needs occurrence identity separate from content hashes/rendered rows, and stable ordering through relayout; P3-22 skips unavailable image states in lite or unsupported terminals.
- **D — close-out:** P3-23 (small), then the exit decision. Behavioral docs change with each PR; only final screenshots and ASCII assets wait until the UI settles.

Release numbers are intentions, not reservations: intervening fixes or a separately released Batch B consume the next `v0.1.N` number. Tag only a green main merge commit, following [releasing](../guides/releasing.md).

P3-01 is explicitly deferred by operator decision (2026-10-03) before P3-17 settles vertical geometry; no sticky-heading row is reserved. P3-17 does not itself complete sticky headers or back/forward buttons. Defer P3-04/P3-05 to Phase 4 at exit unless dogfood justifies them. P3-03 is completed by P3-16; P3-02 is advanced, not necessarily closed, by P3-18. Detail later rows only when next up.

### Verification and housekeeping

Per PR: `scripts/check.sh`, intentional snapshot review, task IDs in commit messages and dated dogfood evidence. P3-19 must add actual default/lite clippy and test commands to the script and CI: the current script ignores command-line arguments, so passing `--no-default-features` alone does not work. Gate image examples for `--all-targets`, forward features across crates, and assert the lite binary dependency tree excludes `image`, `resvg`, `mermaid-rs-renderer` and `ratatui-image` even with workspace feature unification. Default snapshots must not change during extraction.

Per batch: manual passes in Ghostty and herdr, plus iTerm2/tmux for media or fallback changes, using the repo root, `fixtures/mermaid`, `fixtures/images` and `wiki/`. Per release: verify three tarballs/checksums, a downloaded binary and the Cargo-installed dogfood binary's version; add a new release heading in the dogfood log. At the ≈ 2026-10-13 Phase 2 review, record the adoption verdict; only a positive verdict changes Phase 2 to done and updates both roadmap status rows.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | done | daf8fc5; hygiene f90c378; release hardening 9012fce; [v0.1.0-alpha.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.1); [v0.1.0-alpha.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.2); [v0.1.0-alpha.3](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.3); [v0.1.0-alpha.4](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4); [v0.1.0-alpha.4.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4.1); [v0.1.0-alpha.5](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.5) (P3-12c/d Mermaid image tier); [v0.1.0-alpha.5.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.5.1) (P2-55 copy path); [v0.1.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.1) (P3-07/11/13/14/15, the first `v0.1.N` tag); [v0.1.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.2) (2026-10-03, P2-56 theme-switch fix and docs filing; three release targets and macOS arm64 download verified) |
| P3-06 | Help overlay with clickable keys | | done | done via P2-20 (#71) |
| P3-07 | Themes | | done | Reworked 2026-10-02 (local time; [ADR-0019](../decisions/0019-theme-presets.md) retains its immutable UTC date, 2026-10-03): `dark` / `light` take the luckgrid.net palette, `herdr` follows the theme named in herdr's config and is the default inside herdr; live follow is P4-05. Original: `theme = dark \| light \| herdr` wired to `Theme::from_name`; light preset with a contrast test; syntect theme and Mermaid palette follow the preset (diagrams were always drawn light before); herdr preset is vesper-based; quadrant/xy-chart/ER-key fills stay light (renderer hard-codes them). Checked 2026-10-02: Mermaid fixtures look right in the default (dark) theme. Light and herdr presets got their in-app visual pass with the options window (P3-13) |
| P3-11 | Layout and theme config | C1 | done | `nav.position = left \| right` (default left); divider on the inner edge; Viewer ←/→ at the edge toward the nav hands focus over. Nav width stays session-only (P2-14). No other layout keys (formatted view removed, [ADR-0014](../decisions/0014-remove-formatted-view-toggle.md)). Feature request from 2026-09-30 dogfood |
| P3-12 | Image diagrams and images (D1) | D1 | done | was P2-24b; umbrella for P3-12a…d; closed with P3-12d (landed with 12c) |
| P3-12a | Spike: protocol detection in Ghostty and herdr; accept ADR-0004 | D1 | done | [P3-S1](spikes/p3-s1-image-protocol.md): Kitty works direct/Herdr; iTerm2 must be forced from env; unknown terminals are not probed; crop/clear/swap pass; Mermaid mixed-go (12 valid blocks/6 common types pass, wide graphs fall back); ADR-0004 accepted |
| P3-12b | Image slots + local images (static, in-collection, never fetched) | D1 | done | [ADR-0017](../decisions/0017-static-local-images-only.md); implemented: core path policy and caps, `ImageSlot` rows, `ImageManager` decode worker, startup probe, draw pass with top/bottom crop, `[image: alt]` fallback, `fixtures/images`. SVG moved to P3-12c with `resvg`. Manual check 2026-10-02 (`cargo run -p wiki-reader` at the repo root): pictures display in Ghostty, in a herdr pane and in iTerm2; Terminal.app and tmux show the `no graphics protocol` placeholder, as designed. Scroll-crop and popup behaviour were not itemised in the report; hardening: visible-only queue, LRU cache, failure cache |
| P3-12c | Mermaid image tier | D1 | done | `mermaid-rs-renderer` → `resvg` → slots; off-thread size cache `(hash, bg)` + re-layout (preserves selection); SVG-only measure, raster on decode; Decode before Measure; embedded Noto Sans; legibility gate ≥ 0.55 → text with reason; SVG local images; release binary **+~6.7 MiB** (9.7 → 16.4 MiB, measured 2026-10-02 vs P3-12b). Manual check 2026-10-02 (`cargo run -p wiki-reader fixtures/mermaid`): Ghostty launches and shows the collection; tmux stays on the text tier (ASCII diagram, no image slots); automated coverage for text→image swap, Help overlay, selection-survives-relayout, TooWide/Failed reasons, Decode-before-Measure. herdr/iTerm2/Terminal.app probe behaviour unchanged from P3-12b matrix |
| P3-12d | `diagrams` config + tier selection | D1 / C1 | done | landed with P3-12c: `auto \| image \| text \| source`; tmux → text even for `image`; `text`/`source` skip probe; herdr Kitty-only (never Sixel); fallback reasons in tier headers |
| P3-01 | Sticky viewer section header | U1 | deferred | Operator decision 2026-10-03: defer before P3-17 geometry; no sticky row reserved. Revisit after alpha chrome settles |
| P3-02 | Side nav header/footer sub-regions | U2 | todo | advanced by P3-18 (footer ⚙ / `?`) |
| P3-04 | Optional header ‹ › buttons | U4 | todo | |
| P3-05 | Link hover preview popover | W6 | todo | |
| P3-03 | Nav label options | U3 | done | completed by P3-16; two modes, literal filenames/folders and legacy config migration |
| P3-09 | herdr: launch as a herdr plugin pane | | todo | after real herdr API |
| P3-10 | herdr: publish the current page to herdr's sidebar | | todo | after real herdr API |
| P3-13 | Options window | U5 | done | feature request 2026-10-02. A popup like markdown-reader's, opened by a key and a footer/header button, that edits config from inside the app: **theme** selector (dark / light / herdr, live preview), **nav position** (left / right, P3-11), **nav labels** (titles ↔ filenames, `nav.labels`), **Mermaid** settings (`diagrams` tier: auto / image / text / source), **image** settings, **copy path** format (`copy.path`: relative / absolute, key added by P2-55). Implemented: `,` / `c` or the layout footer ⚙ (moved by P3-18) opens the overlay; writes via [ADR-0018](../decisions/0018-config-write-path.md); new `[images] enabled \| max_slot_rows`; live apply for every row (images re-enable after a disable needs a restart when the startup probe was skipped). Reworked 2026-10-02 (local time) after dogfood: grouped radio rows in the style of markdown-reader's settings window, `,` and `c` both open and close it, and the copy-path key is listed in Help as "Copy file path" |
| P3-14 | Table viewer | U6 | done | feature request 2026-10-02. Open a large table in a modal / window (like markdown-reader): scroll in both axes with a fixed header row and first column, **filter** rows by text, **sort** by a column, copy a cell or row (reuse the tab-separated table copy from P2-R38). Entry point: the existing "expand table" block action (BA), plus `Enter` on a focused table. Also the shell for P3-15. Implemented: `RenderedDoc::tables` (`DocTable`) from `render.rs`; `ExpandTable` block action and `Enter` open `tui/table_viewer.rs` inside `tui/modal_viewer.rs`. Keys: arrows / `hjkl`, `PgUp` / `PgDn`, `g` / `G`, `/` filter, `s` sort, `y` cell, `Y` row. See [UI spec: Modal viewers](../product/ui-spec.md#modal-viewers) |
| P3-15 | Image and diagram viewer | U7 | done | feature request 2026-10-02. Open an image or Mermaid diagram full size in a modal: **zoom** in/out and **pan**. Reuses the P3-12 pipeline at a larger pixel budget. Entry: `ExpandDiagram` / `Enter` on a slot. Keys: arrows / `hjkl` pan, `PgUp` / `PgDn`, `g` / `G`, `+` / `-` zoom, `0` reset. SVG/Mermaid re-rasterise at zoom; raster files scale from the bitmap. See [UI spec: Modal viewers](../product/ui-spec.md#modal-viewers). Deferred: mouse drag to pan, click on a picture to open it, carousel (P3-21), diagram state toggle (P3-22), viewer sizing (P3-20) |
| P3-16 | Nav label modes: drop `title+filename`; `filename` shows real file-system names | U3 | done | Implemented 2026-10-03 (unreleased): title/filename only; on-disk folder names; filenames including extensions and README/index landings; curated side-nav navigation respects filename mode. Operator clarification: filename mode affects only the side-nav tree; header breadcrumbs and View footer labels stay title-based ([ADR-0021](../decisions/0021-side-nav-only-label-mode.md)). Legacy title+filename maps to title with one diagnostic per config load across layers. [ADR-0020](../decisions/0020-nav-label-modes.md) supersedes label decisions without editing accepted ADRs. Config/options/spec/content-model docs and regression snapshots updated at 40/60/80/120 columns; `scripts/check.sh` green. Operator manually confirmed filename labels in the side nav and title-based header breadcrumbs and View footer labels. This satisfies the P3-16 portion of the Batch A manual gate; the remaining Ghostty/herdr pass covers P3-18/P3-17 only |
| P3-17 | Chrome pass: tab titles end in `.md`; tabs and footer link buttons get top and bottom borders; active tab keeps a darker selection background when nav is focused | U1 / U2 / U4 | active | Implemented on `feat/p3-17-chrome` (unreleased): filename tab labels including extensions; fully closed tab/prev-next boxes in dedicated three-row strips; darker active selection with Nav focus (muted behind popups). Collision and short-terminal policy specified in ui-spec.md before coding; six chrome rows accounted for in article geometry/scrolling. `?` / ⚙ stay in the separate layout footer. P3-01 explicitly deferred; history buttons remain separate. `scripts/check.sh` passed (536 tests, two ignored); responsive snapshots reviewed. Operator Ghostty/herdr Batch A pass required before completion |
| P3-18 | Footer buttons: ⚙ moves from the header to the footer's bottom right, with a `?` help button to its left | U2 / U8 | done | Implemented in #123 (unreleased): layout footer `?` / ⚙ below both panes reserve space before status fields/messages, not prev/next; header retains ◫ / ✕ with bounded breadcrumb hits. Help icon row, generated keymap, behavioral docs and README diagram updated. Regression tests cover both nav sides, 40/60/80/120 widths, tiny layout-footer geometry and narrow overlay dismissal; intentional snapshots reviewed. Operator confirmed the corrected layout-footer terminal check passed; automated verification recorded in the dogfood log. Batch A's final Ghostty/herdr pass remains after P3-17. Advances P3-02; three-row boxes remain P3-17 |
| P3-19 | Lite build: move image, SVG and Mermaid-raster code into its own crate (`wiki-reader-media`) behind a default-on cargo feature | U9 | todo | dogfood 2026-10. `cargo install --no-default-features` gives a build with no image, resvg, mermaid-rs-renderer or ratatui-image. Diagrams render as text through mermaid-text. `diagrams = image` falls back to text with a reason; the Images options group is hidden. Spike first: measure a build without the four heavy dependencies. Then record the cargo-feature-only distribution decision in a new ADR, add a CI check for both builds, document the lite install in README and configuration.md, and fill the lite column in [benchmarks](../architecture/benchmarks.md). No separate `-lite` release artifact; revisit only if a prebuilt lite binary is requested |
| P3-20 | Viewer sizing | U6 / U7 | todo | dogfood 2026-10. Table and image/diagram windows size to their content and stay capped at today's size. Wide pictures keep today's width. Add a fit / actual-size toggle on top of the existing zoom. Table viewer keeps in-view table styles (bold header, borders, link colour). Opening a picture should show it larger than its inline slot |
| P3-21 | Image/diagram carousel | U7 | todo | dogfood 2026-10. Tab / Shift+Tab step through every image and diagram in the document. The picture you opened shows first. Hints go in the modal hint bar, the Help window and ui-spec.md. Arrows stay pan |
| P3-22 | Diagram state toggle | U7 | todo | dogfood 2026-10. A key cycles image / text / source for the open diagram. Session-only. Builds on P3-21 |
| P3-23 | Docs refresh once the UI settles | — | todo | dogfood 2026-10. Retake `assets/wiki-reader.png` and the `wiki/assets/` copies (new theme, footer icons, borders). Redraw the README ASCII diagram. Add the carousel keys. Done last so screenshots are not retaken twice |

## Related

- [Phase 2](phase-2-mvp.md)
- [Dogfood log](dogfood-log.md)
- [Integrations](../architecture/integrations.md)
