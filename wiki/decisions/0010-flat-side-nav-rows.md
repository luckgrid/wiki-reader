---
id: WR-ADR-0010
title: "ADR-0010: Flat visible rows for side nav (supersedes ADR-0009)"
summary: Ship a custom flat list of visible rows into HitMap instead of tui-tree-widget; TreeState would duplicate NavState.
status: accepted
updated: 2026-09-29
related: [0008-side-nav-as-site-nav, 0009-tui-tree-widget]
---

# ADR-0010: Flat visible rows for side nav

**Status:** Accepted · **Date:** 2026-09-29 · **Supersedes:** [ADR-0009](0009-tui-tree-widget.md)

## Context

ADR-0009 chose `tui-tree-widget` after a compile-gate spike showed `rendered_at` / `click_at`. P1-07c implementation found that keeping `TreeState` in sync with core `NavState` (expanded set, cursor/`NavStop`, current page) each frame duplicated the source of truth while our `HitMap` already owns row rectangles.

## Options

**A. Keep `tui-tree-widget` (ADR-0009).** Widget owns open/close/scroll; map identifiers to `NodeId`.

**B. Custom flat list of visible rows.** Walk the tree with `expanded`, emit rows + rects into `HitMap` (search row + tree).

## Decision

**B.** Flat visible rows. Core remains the only expand/cursor authority; the TUI draws what core says and registers hits. No `tui-tree-widget` dependency on `wiki-reader`.

## Consequences

- ➕ One source of truth for expand/cursor (`NavState` / `NavStop`).
- ➕ Hit regions are exact cells we paint (search row, group toggles, pages).
- ➖ Own scroll-follow and clamp (done in P1-R7).
- ➖ Revisit the widget only if we need features that are cheaper as a dependency than as code.
