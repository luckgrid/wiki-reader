---
id: WR-ROADMAP-P3
title: Phase 3 — Alpha polish
summary: Sticky headers, nav chrome, help, themes, install, and early herdr niceties.
status: planned
updated: 2026-09-30
related: [phase-2-mvp, v2-widget-sidebar]
nav_order: 3
---

# Phase 3 — Alpha polish

Phase 1 is closed. Detail later rows into spikes/acceptance only after Phase 2 dogfood notes land. Order is **adoption value first** — install early so real use does not depend on a checkout.

## Proposed order

1. **P3-08 Install** — `cargo install` path + GitHub Actions release binaries (macOS/Linux, checksums); document herdr setup.
2. **P3-07 Themes / P3-11 Layout config** — wire the stored `theme` config key to the semantic token table in `theme.rs`; ship dark/light presets plus a herdr-matching preset; add nav placement (left/right) and related layout options. (R35 already warns when the key is set but inert. The help overlay moved to P2-20.)
3. **P3-01 / P3-02 / P3-04** — sticky section header, side nav header/footer regions, optional ‹ › header buttons.
4. **P3-05 / P3-03** — link hover preview; nav label options (R35 already draws a dim `(filename)` suffix for `title+filename`; full “alt text below” remains here).
5. **P3-09 / P3-10** — herdr integration (plugin pane; publish current page to herdr sidebar) after the herdr API is confirmed on a real install.
6. **P3-12 D1 image tier** — diagrams/images via Kitty (was P2-24b; slipped 2026-09-30). Needs ADR-0004 herdr/Kitty spike before deps.

Detail each row into spikes/acceptance only after the Phase 2 dogfood notes in [phase-2-mvp.md](phase-2-mvp.md).

Dogfood → Phase 3 feed (2026-09-30): start with **P3-07/11** (light-terminal contrast, eye default, nav placement); then chrome (P3-01/02/04); then P3-05/03; herdr (P3-09/10) after API; **P3-12** after ADR-0004 spike. Known formatted-view deviations (checkbox/`[NOTE]`/quote bar) stay unless they bite during freeze.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | done | daf8fc5; hygiene f90c378; release hardening 9012fce; [v0.1.0-alpha.1](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.1); [v0.1.0-alpha.2](https://github.com/luckgrid/wiki-reader/releases/tag/v0.1.0-alpha.2) |
| P3-06 | Help overlay with clickable keys | | done | done via P2-20 (#71) |
| P3-07 | Themes | | todo | wire stored theme key; include a herdr-matching preset; fix fixed-RGB colours that look wrong on light terminals |
| P3-11 | Layout and theme config | C1 | todo | nav placement (left/right), syntax vs formatted view (eye toggle from P2-24a), and other layout options; owns config keys (nav width stays in session for P2-14); feature request from 2026-09-30 dogfood; formatted mode still shows `[ ]`/`[x]` (and related `[NOTE]`/quote-bar marker residuals) |
| P3-12 | Image diagrams and images (D1) | D1 | todo | was P2-24b; ADR-0004 spike → mermaid-rs-renderer / resvg / Kitty; alt-text fallback |
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
