---
id: WR-ROADMAP-P3
title: Phase 3 — Alpha polish
summary: Diagrams and images first, then themes, sticky headers, nav chrome, and early herdr niceties.
status: active
updated: 2026-10-01
related: [phase-2-mvp, phase-4-beta]
nav_order: 3
---

# Phase 3 — Alpha polish

Phase 1 is closed and Phase 2 is feature complete (dogfood hold until ≈ 2026-10-13, fixes only; see [phase-2-mvp.md](phase-2-mvp.md)). Phase 3 was activated on 2026-10-01. Order is **reader value first**: the spec's biggest remaining gap is images and diagrams, so P3-12 leads. Install (P3-08) already shipped.

## Proposed order

1. **P3-12 Images and diagrams** — split into four steps, each its own PR:
   1. **P3-12a Spike** — does `ratatui-image` detect the right protocol in Ghostty and inside a herdr pane (`HERDR_ENV=1`, `terminal.kitty_graphics`)? Its startup query must run before our raw-mode/kitty-keyboard setup, and herdr may not answer it. Output: an accepted [ADR-0004](../decisions/0004-diagram-rendering.md) with the detection rule.
   2. **P3-12b Image slots and local images** — the renderer reserves N rows for an image, the TUI draws into them, and the text placeholder is the fallback. Policy is [ADR-0017](../decisions/0017-static-local-images-only.md): static, inside the collection, never fetched. This step builds the hard parts (scroll clipping, clearing under the Help/Search popups and on tab/page switch) with the simplest content.
   3. **P3-12c Mermaid image tier** — `mermaid-rs-renderer` → `resvg` → PNG into the same slots, off the UI thread, cached by content/width/background; embedded font so output is deterministic; falls back to the text tier.
   4. **P3-12d `diagrams` config and tier selection** — `auto | image | text | source`, tmux → text, never Sixel under herdr.
2. **P3-07 Themes / P3-11 Layout config** — wire the stored `theme` config key to the semantic token table in `theme.rs`; ship dark/light presets plus a herdr-matching preset; add nav placement (left/right) and related layout options. (R35 already warns when the key is set but inert.)
3. **P3-01 / P3-02 / P3-04** — sticky section header, side nav header/footer regions, optional ‹ › header buttons.
4. **P3-05 / P3-03** — link hover preview; nav label options (R35 already draws a dim `(filename)` suffix for `title+filename`; full “alt text below” remains here).
5. **P3-09 / P3-10** — herdr integration (plugin pane; publish current page to herdr sidebar) after the herdr API is confirmed on a real install.

Detail each row into spikes/acceptance only when it is next up. Dogfood bites recorded in [phase-2-mvp.md](phase-2-mvp.md) still feed this list.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | done | daf8fc5; hygiene f90c378; release hardening 9012fce; [v0.1.0-alpha.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.1); [v0.1.0-alpha.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.2); [v0.1.0-alpha.3](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.3); [v0.1.0-alpha.4](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4); [v0.1.0-alpha.4.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.4.1) |
| P3-06 | Help overlay with clickable keys | | done | done via P2-20 (#71) |
| P3-07 | Themes | | todo | wire stored theme key; include a herdr-matching preset; fix fixed-RGB colours that look wrong on light terminals |
| P3-11 | Layout and theme config | C1 | todo | nav placement (left/right) and other layout options; owns config keys (nav width stays in session for P2-14); feature request from 2026-09-30 dogfood. The formatted view was removed ([ADR-0014](../decisions/0014-remove-formatted-view-toggle.md)), so it owns no view-mode key |
| P3-12 | Image diagrams and images (D1) | D1 | doing | was P2-24b; umbrella for P3-12a…d |
| P3-12a | Spike: protocol detection in Ghostty and herdr; accept ADR-0004 | D1 | doing | [P3-S1](spikes/p3-s1-image-protocol.md): ratatui-image 11.1 + ratatui 0.30.2 compile; Kitty query and real PNG verified through herdr; direct Ghostty and non-Kitty rows pending |
| P3-12b | Image slots + local images (static, in-collection, never fetched) | D1 | todo | [ADR-0017](../decisions/0017-static-local-images-only.md); carry startup Picker/cell size; setup is fallible; scroll clip, clear under popups and on tab/page switch, `[image: alt]` fallback, size/pixel caps |
| P3-12c | Mermaid image tier | D1 | todo | `mermaid-rs-renderer` → `resvg` → PNG; off-thread, cached, embedded font; text-tier fallback; do not infer Kitty from `HERDR_ENV` alone |
| P3-12d | `diagrams` config + tier selection | D1 / C1 | todo | `auto \| image \| text \| source`; tmux → text; never Sixel under herdr |
| P3-01 | Sticky viewer section header | U1 | todo | |
| P3-02 | Side nav header/footer sub-regions | U2 | todo | |
| P3-04 | Optional header ‹ › buttons | U4 | todo | |
| P3-05 | Link hover preview popover | W6 | todo | |
| P3-03 | Nav label options | U3 | todo | dim suffix partial via R35 |
| P3-09 | herdr: launch as a herdr plugin pane | | todo | after real herdr API |
| P3-10 | herdr: publish the current page to herdr's sidebar | | todo | after real herdr API |

## Related

- [Phase 2](phase-2-mvp.md)
- [Integrations](../architecture/integrations.md)
