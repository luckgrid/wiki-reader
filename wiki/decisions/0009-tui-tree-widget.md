---
id: WR-ADR-0009
title: ADR-0009: Adopt tui-tree-widget for side nav
summary: Use tui-tree-widget; it exposes position→identifier hit-testing via rendered_at/click_at.
status: superseded
updated: 2026-09-29
related: [0008-side-nav-as-site-nav, 0010-flat-side-nav-rows]
---

# ADR-0009: Adopt tui-tree-widget for side nav

**Status:** Superseded by [ADR-0010](0010-flat-side-nav-rows.md) · **Date:** 2026-09-29 · **Spike:** P1-S2

## Context

P1-07/08 need a collapsible side-nav with mouse hit-testing. The prior-art note suggested evaluating `tui-tree-widget` and falling back to a flat visible-row list if it does not expose row rects.

## Options

**A. Adopt `tui-tree-widget`.** Collapsible tree with `TreeState`; hit-test via `rendered_at(Position) → Option<&[Identifier]>` and `click_at(Position)`.

**B. Custom flat list of visible rows.** Full control over rects in our `HitMap`, more code to own expand/scroll/selection.

## Decision

**A.** `tui-tree-widget` 0.24.x stores last-render row Y positions and maps a mouse `Position` to the identifier path. That is enough for the hit map (`NavItem` / `NavGroupToggle`) without requiring public `Rect`s per row. Wire identifiers to core `NodeId` when the TUI lands.

## Consequences

- ➕ Less UI code; open/close/select/scroll helpers included.
- ➕ Mouse clicks work through `click_at` / `rendered_at`.
- ➖ Hit regions are whole rows in the widget area, not arbitrary sub-rects (acceptable for v1 nav).
- ➖ Dependency pinned when P1-07 adds it to `wiki-reader` (not to core).

## Follow-up (P1-R5 compile gate)

Verified 2026-09-29 in a throwaway crate (`tui-tree-widget` 0.24.1 + `ratatui` 0.30.2, `TestBackend`): `TreeState::rendered_at(Position)` and `TreeState::click_at(Position)` compile and return hits after a stateful render. Gate crate was not kept in-tree.

## Supersession (P1-R10)

P1-07c shipped option **B** (flat rows into `HitMap`). Syncing `TreeState` from core `NavState` each frame duplicated state. See [ADR-0010](0010-flat-side-nav-rows.md).
