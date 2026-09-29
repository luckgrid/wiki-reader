---
id: WR-ROADMAP-P1
title: Phase 1 — Reader shell
summary: Browse a real collection end-to-end with correct navigation.
status: active
updated: 2026-09-29
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
| P1-01 | FsProvider + discovery | | done | 7bc90c9 |
| P1-02 | Frontmatter + parse | | done | a907712 |
| P1-03 | Index | | done | f2752c2 |
| P1-04 | Link resolution | L2 | done | 216be37 |
| P1-05 | NavTree + fixture snapshots | T1 | done | a2d68c5 |
| P1-06 | navigate() + history + invariant tests | N1–N4 | done | ae5fd3a |
| P1-R1 | Parse robustness | F2,F7 | done | a0656a7 |
| P1-R2 | Index robustness + determinism | F3,F5,F8,F9 | done | 4ee9957 |
| P1-R3 | NavTree correctness | F1,F5,F6,T1 | done | e85c57b |
| P1-R4 | Navigator hardening | F4,F10,N1–N4 | done | 66866be |
| P1-R5 | Docs, spikes, housekeeping | | done | 20bdf18 |
| P1-07 | Layout regions + hit map | | todo | parent of 07a–07c |
| P1-07a | Core support APIs (breadcrumb, blocks, nav focus) | | done | (pending merge) |
| P1-07b | TUI skeleton: regions, hit map, static content | | doing | |
| P1-07c | Side nav (tui-tree-widget) | T1,N4 | todo | ADR-0009 |
| P1-G | Live herdr key-log gate | | todo | Entry criterion for P1-08a; [P1-S1](spikes/p1-s1-herdr-input.md) |
| P1-08 | Focus/cursor | K1–K4 | todo | parent of 08a–08c; entry: P1-G |
| P1-08a | Pane focus (K1) + side-nav keys (K4) | K1,K4 | todo | |
| P1-08b | Viewer cursor (K2) + interim ViewerDoc | K2 | todo | |
| P1-08c | Viewer Tab cycle (K3), logic only | K3 | todo | Enter activation → P1-09 |
| P1-09 | Links, Tab/Enter/click | L1–L3 | todo | non-md / `?query` targets documented in content-model |
| P1-10 | Search overlay | S1 | todo | |
| P1-11 | Raw toggle | V2 | todo | |
| P1-12 | Live reload | V3 | todo | |
| P1-13 | Renderer port | V1 | todo | needs P1-R1 `github_slug` |
| P1-S1 | Spike, herdr input | | done | [spike note](spikes/p1-s1-herdr-input.md); doc-only; live herdr key log pending |
| P1-S2 | Spike, tree widget decision | | done | ADR-0009 |

## Related

- [Product spec](../product/spec.md) P0 requirements
- [Architecture](../architecture/overview.md)
