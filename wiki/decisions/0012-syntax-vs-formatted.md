---
id: WR-ADR-0012
title: "ADR-0012: Syntax vs formatted rendered view"
summary: Raw stays on r; an eye toggle switches Rendered between syntax (markers visible) and formatted (markers hidden). Default is syntax until dogfood says otherwise; config key waits for P3-11.
status: superseded
updated: 2026-09-30
related: [0011-renderer-source, phase-2-mvp]
---

# ADR-0012: Syntax vs formatted rendered view

**Status:** Superseded by [ADR-0014](0014-remove-formatted-view-toggle.md) · **Date:** 2026-09-30

## Context

P2-23 made the rendered viewer a readable *syntax* view (markdown markers stay visible). Dogfood still wants a browser-style reading mode without `#`, `**`, fences, and so on. Today `r` toggles Raw (syntect source) ↔ Rendered. Adding a third presentation must not overload `r` or break cursor / link / block-action geometry.

## Decision

- **`r`** continues to toggle **Raw ↔ Rendered** (unchanged).
- An **eye** control in the header (and matching action) toggles Rendered between:
  - **Syntax** (default): markers visible — current P2-23 behaviour.
  - **Formatted**: markers dropped at push time in the renderer (`RenderOpts.formatted`), so `source_map`, links, and block actions stay aligned with the visible spans.
- **Default is syntax** until dogfood says otherwise.
- The **config key** for the preference ships with **P3-11**; until then the choice lives in the session with a serde default so old session files load.

## Consequences

- ➕ Reading mode without a second document model.
- ➕ Cursor / Tab / link hits keep working because markers are omitted when spans are pushed, not stripped after layout.
- ➖ Heading level without `#` relies on colour / spacing; code blocks need a language label frame instead of fences.
