---
id: WR-ROADMAP-P2
title: Phase 2 — Wiki navigation MVP
summary: Custom nav order, backlinks, tabs, diagrams, config, and session restore.
status: active
updated: 2026-09-30
related: [phase-1-reader-shell, phase-3-alpha]
nav_order: 2
---

# Phase 2 — Wiki navigation MVP

Time box: ≈ 2 weeks.

## Exit criteria

All P0/P1 acceptance criteria in [product spec](../product/spec.md) pass; two weeks without opening a GUI markdown app for these collections.

## Tasks

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P2-01 | Custom nav order via `SUMMARY.md` / `nav_order` | P1 | done | shipped in P1-R3 |
| P2-02 | "Linked from" backlinks | B1 | done | 47185bc |
| P2-03 | Block actions in the Tab cycle | BA | done | d8c63e2 |
| P2-04 | Heading jump | J1 | done | 19faa9e |
| P2-05 | Tabs as secondary | TB | done | 81ea91f |
| P2-06 | Mermaid tiers | D1 | done | 81ea91f text tier; image deferred |
| P2-07 | Responsive side nav | R1 | done | shipped in P1-R6 |
| P2-08 | `$EDITOR` | E1 | done | d130b92 |
| P2-09 | Config incl. `nav.labels` | C1 | done | 19db10d |
| P2-10 | Session restore | M1 | done | 36e00c2 |
| P2-R31 | Config trust merge (exclude union, keys merge) | C1 | done | 81ea91f |
| P2-R32 | Session autosave + test isolation | M1 | done | 81ea91f |
| P2-R33 | Block/copy polish | BA | done | 81ea91f |

## Proposed order (after Phase 1 exit)

1. P2-R31 → P2-R32 → P2-R33
2. P2-05 Tabs
3. P2-06 Diagrams

(Batch 1–2 done: P2-02/04/08/09/10/03.)

## Related

- [Phase 1](phase-1-reader-shell.md)
- [Product spec](../product/spec.md) P1
