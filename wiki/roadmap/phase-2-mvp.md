---
id: WR-ROADMAP-P2
title: Phase 2 — Wiki navigation MVP
summary: Custom nav order, backlinks, tabs, diagrams, config, and session restore.
status: active
updated: 2026-09-29
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
| P2-03 | Block actions in the Tab cycle | BA | todo | extend `FocusTarget` |
| P2-04 | Heading jump | J1 | done | 19faa9e |
| P2-05 | Tabs as secondary | TB | todo | core `Disposition` ready; TUI bar + keys |
| P2-06 | Mermaid tiers | D1 | todo | text tier first; image behind Kitty/herdr |
| P2-07 | Responsive side nav | R1 | done | shipped in P1-R6 |
| P2-08 | `$EDITOR` | E1 | doing | this PR |
| P2-09 | Config incl. `nav.labels` | C1 | todo | before session restore |
| P2-10 | Session restore | M1 | todo | `$XDG_STATE_HOME/wiki-reader/state.toml` |

## Proposed order (after Phase 1 exit)

Cheapest / most reader-visible first. Refine once the Phase 1 exit pass results are in.

1. P2-02 Linked from
2. P2-04 Heading jump
3. P2-08 Open in `$EDITOR`
4. P2-03 Block actions
5. P2-09 Config then P2-10 Session
6. P2-05 Tabs
7. P2-06 Diagrams

## Related

- [Phase 1](phase-1-reader-shell.md)
- [Product spec](../product/spec.md) P1
