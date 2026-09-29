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

P1-08 exit: pane focus, viewer cursor, and Tab-cycle highlighting land via P1-08a–c + R6–R9. Open from the herdr key log (P1-S1 checklist): live verification under herdr still pending — keymap ships F6 / Ctrl+↑↓ fallbacks until that gate is ticked.

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
| P1-R6 | Geometry: header hits + nav visibility | T1,T2 | done | c97963e |
| P1-R7 | Viewport + scroll follow | T3,T4 | done | c97963e |
| P1-R8 | Focus + nav state model | T6–T9 | done | c97963e |
| P1-R9 | K3 visuals, keymap, terminal safety | T5,T12,T13 | done | c97963e |
| P1-R10 | Docs, process, test hygiene | T10–T11,T14,T16–T18 | done | 5911ea5 |
| P1-R11 | Post-merge residuals + check.sh | N1–N6 | doing | PR #20 |
| P1-R12 | Renderer to V1 | V1 | doing | PR #22 |
| P1-R13 | Cursor mapping + syntect raw | V2 | doing | PR #23 |
| P1-R14 | Search core + overlay | S1 | doing | PR #24 |
| P1-R15 | Watcher + reindex | V3 | doing | PR #25 |
| P1-R16 | Hygiene | | doing | workspace deps, hit clamp, link-chain |
| P1-07 | Layout regions + hit map | | done | 5911ea5 |
| P1-07a | Core support APIs (breadcrumb, blocks, nav focus) | | done | 5911ea5 |
| P1-07b | TUI skeleton: regions, hit map, static content | | done | 5911ea5 |
| P1-07c | Side nav (`HitMap` flat rows; ADR-0010) | T1,N4 | done | 5911ea5 |
| P1-G | Live herdr key-log gate | | done | example done; live herdr check pending (gate still open) | [P1-S1](spikes/p1-s1-herdr-input.md) |
| P1-08 | Focus/cursor | K1–K4 | done | 5911ea5 |
| P1-08a | Pane focus (K1) + side-nav keys (K4) | K1,K4 | done | 5911ea5 |
| P1-08b | Viewer cursor (K2) + interim ViewerDoc | K2 | done | 5911ea5 |
| P1-08c | Viewer Tab cycle (K3) | K3 | done | 5911ea5 |
| P1-09 | Links, Tab/Enter/click | L1–L3 | doing | PR #21 |
| P1-10 | Search overlay | S1 | doing | PR #24 (fuzzy + overlay landed) |
| P1-11 | Raw toggle | V2 | doing | PR #23 (syntect + gutter + source cursor) |
| P1-12 | Live reload | V3 | doing | PR #25 (debouncer + TUI poll) |
| P1-13 | Renderer port | V1 | doing | PR #22 (StyledLine V1) |
| P1-S1 | Spike, herdr input | | done | [spike note](spikes/p1-s1-herdr-input.md); doc-only; live herdr key log pending |
| P1-S2 | Spike, tree widget decision | | done | ADR-0009 → superseded by ADR-0010 |

## Related

- [Product spec](../product/spec.md) P0 requirements
- [Architecture](../architecture/overview.md)
