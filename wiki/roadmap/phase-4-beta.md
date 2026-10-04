---
id: WR-ROADMAP-P4
title: Phase 4 — Beta (widget sidebar and agent surface)
summary: Seeds for after alpha polish — right-hand widget slot, context engine, agent CLI, and provider/theme revisits.
status: proposed
updated: 2026-10-03
related: [phase-3-alpha, dogfood-log]
nav_order: 4
---

# Phase 4 — Beta

Seeds, not a committed plan. This phase was previously filed as "v2 — Widget sidebar"; the work is unchanged, only the name and task IDs moved to match the phase structure.

## Goal

Extend the reader past a single content pane: a right-hand widget slot, a first set of widgets (context engine), and a machine-readable surface for agents, all on the same terminal-free core ([ADR-0006](../decisions/0006-reader-first.md)).

## Entry criteria

- Phase 3 exit decision recorded against [its exit criteria](phase-3-alpha.md#exit-criteria): chrome, lite build and viewer polish shipped; herdr integration confirmed on a real install or explicitly deferred here. Deferred integration requires the API spike evidence before implementation.
- The widget requirements are still unsettled; confirm them from the [dogfood log](dogfood-log.md) before detailing any row below.

## Seeds

| ID | Task | Reqs | Status | Notes |
|----|------|------|--------|-------|
| P4-01 | Right-hand widget slot with a small widget trait | | todo | was V2-01; read-only page + index; renders into a rect; registers hits. Header/footer sub-regions beyond the P3-18 footer controls remain part of this widget-slot concern (P3-02 closed) |
| P4-02 | First widgets: context engine, page metadata, backlinks graph summary | | todo | was V2-02; see [context engine](../architecture/context-engine.md) |
| P4-03 | Agent CLI (`--json`) on the same core | | todo | was V2-03; see [integrations](../architecture/integrations.md) |
| P4-04 | Revisit external providers and theme-token mapping | | todo | was V2-04; builds on the Phase 3 theme presets |
| P4-05 | Follow herdr's theme live | | todo | [ADR-0019](../decisions/0019-theme-presets.md) reads herdr's theme name once at startup. Feasible without a plugin: watch and re-read herdr's `config.toml` using the existing `notify` dependency, then re-theme when the configured name changes. [P3-S2](spikes/p3-s2-herdr-integration.md) records the seam; no theme event/API is available. Custom palette mapping and terminal-driven `auto_switch` light/dark need separate evidence; a config watcher alone cannot observe terminal appearance. Filed 2026-10-02 (local time) from dogfood; implement in Phase 4 |
| P4-06 | Mirrored keybindings when nav is on the right | | todo | dogfood 2026-10; low priority. Needs a dynamic keymap; Help, nav and View all read it. Today only the ←/→ hand-off at the nav edge mirrors (P3-11). Open design: ← opens the target and focuses the View with the cursor at the end of the row; the other directional keys flip by the same rule. Needs an ADR |
| P4-07 | Optional header ‹ › buttons | U4 | todo | Deferred from P3-04 by operator decision 2026-10-03; history actions remain available through existing keys |
| P4-08 | Link hover preview popover | W6 | todo | Deferred from P3-05 by operator decision 2026-10-03 |

## Deferred ideas feeding this phase

- Right-hand context/widget sidebar — requirements not clear yet ([vision](../product/vision.md)).
- Herdr context signals (sibling pane cwds and agent states) — see [integrations](../architecture/integrations.md).
- A side nav footer for widget actions or tabbed features (Pages / Outline), designed as a future slot in the [UI spec](../product/ui-spec.md).

## Related

- [Phase 3](phase-3-alpha.md)
- [ADR-0006](../decisions/0006-reader-first.md) — core stays terminal-free for this
- [Context engine](../architecture/context-engine.md)
