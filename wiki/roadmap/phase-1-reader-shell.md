---
id: WR-ROADMAP-P1
title: Phase 1 — Reader shell
summary: Browse a real collection end-to-end with correct navigation.
status: active
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

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P1-00 | Scaffold | | done | 2b9fdb6 |
| P1-01 | FsProvider + discovery | | done | c67c19f |
| P1-02 | Frontmatter + parse | | todo | |
| P1-03 | Index | | todo | |
| P1-04 | Link resolution | L2 | todo | |
| P1-05 | NavTree + fixture snapshots | T1 | todo | |
| P1-06 | navigate() + history + invariant tests | N1–N4 | todo | |
| P1-07 | Layout regions + hit map | | todo | placeholder regions done |
| P1-08 | Focus/cursor | K1–K4 | todo | |
| P1-09 | Links, Tab/Enter/click | L1–L3 | todo | |
| P1-10 | Search overlay | S1 | todo | |
| P1-11 | Raw toggle | V2 | todo | |
| P1-12 | Live reload | V3 | todo | |
| P1-13 | Renderer port | V1 | todo | |
| P1-S1 | Spike, herdr input | | todo | |
| P1-S2 | Spike, tree widget decision | | todo | |

## Related

- [Product spec](../product/spec.md) P0 requirements
- [Architecture](../architecture/overview.md)
