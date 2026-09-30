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

Start after Phase 1's real-keyboard / real-collection gate and Phase 2 dogfood notes land. Order is **adoption value first** — install early so real use does not depend on a checkout.

## Proposed order

1. **P3-08 Install** — `cargo install` path + GitHub Actions release binaries (macOS/Linux, checksums); document herdr setup.
2. **P3-06 Help overlay (`?`)** — generated from the keymap table so it stays correct; clickable keys.
3. **P3-07 Themes** — wire the stored `theme` config key to the semantic token table in `theme.rs`; ship dark/light presets. (R35 already warns when the key is set but inert.)
4. **P3-01 / P3-02 / P3-04** — sticky section header, side nav header/footer regions, optional ‹ › header buttons.
5. **P3-05 / P3-03** — link hover preview; nav label options (R35 already draws a dim `(filename)` suffix for `title+filename`; full “alt text below” remains here).
6. **P3-09 / P3-10** — herdr integration (plugin pane; publish current page to herdr sidebar) after the herdr API is confirmed on a real install.
7. **D1 image tier** (follow-up) — mermaid-rs-renderer → resvg → Kitty once herdr/Kitty detection is confirmed; text tier stays the default.

Detail each row into spikes/acceptance only after the Phase 2 dogfood notes in [phase-2-mvp.md](phase-2-mvp.md).

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P3-08 | Install via `cargo install` + release binaries | | todo | first |
| P3-06 | Help overlay with clickable keys | | todo | from keymap table |
| P3-07 | Themes | | todo | wire stored theme key |
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
