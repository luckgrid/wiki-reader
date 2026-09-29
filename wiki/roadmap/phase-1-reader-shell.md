---
id: WR-ROADMAP-P1
title: Phase 1 — Reader shell
summary: Browse a real collection end-to-end with correct navigation.
status: draft
updated: 2026-09-28
related: [phase-2-mvp]
nav_order: 1
---

# Phase 1 — Reader shell

Browse a real collection end-to-end with correct navigation. Time box: ≈ 1–2 weeks.

## Exit criteria

Open `~/Workspaces/uwiki`, follow ten links in a row, go back ten times, never see a tab.

## Escape hatch

If porting the renderer takes more than ~3 days, temporarily depend on a simpler renderer (plain pulldown-cmark → styled lines) and port features incrementally. Navigation matters more than table polish.

## Tasks

### Workspace & core

- [x] Cargo workspace: `wiki-reader-core`, `wiki-reader-render`, `wiki-reader` (scaffold)
- [ ] `FsProvider` + collection walk
- [ ] Parser (frontmatter split, pulldown-cmark walk, links, headings)
- [ ] Index (pages, headings, edges, search stub)

### Navigation

- [ ] `NavTree` build (titles, folding, order) with fixture snapshots (T1, [ADR-0008](../decisions/0008-side-nav-as-site-nav.md))
- [ ] `navigate()` + history + nav sync (N1–N4) with invariant tests
- [ ] Link resolution (L2–L3)

### TUI shell

- [x] Placeholder layout: header, side nav, viewer, status bar
- [ ] Real layout: breadcrumbs, ◫, ✕; sticky prev/next footer; full-width status bar
- [ ] Focus & cursor model (K1–K4, [ADR-0007](../decisions/0007-input-focus-model.md))
- [ ] Links by Tab/Enter and click (L1–L3); hit map
- [ ] Search overlay (S1)
- [ ] Rendered/raw toggle (V1–V2)
- [ ] Live reload (V3)

### Rendering

- [ ] Renderer port into `wiki-reader-render` + link spans (V1, [ADR-0002](../decisions/0002-build-vs-fork.md))

### Week-1 spikes

- [ ] herdr input: `Shift+arrows`, `BackTab`, `Alt+arrows`, click, middle-click, wheel, motion
- [ ] Custom side nav list vs `tui-tree-widget` (folding and titles likely mean custom)

## Related

- [Product spec](../product/spec.md) P0 requirements
- [Architecture](../architecture/overview.md)
