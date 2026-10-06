---
id: WR-ROADMAP-P4
title: Phase 4 — Beta (widget sidebar and agent surface)
summary: Seeds for after alpha polish — right-hand widget slot, context engine, agent CLI, and provider/theme revisits.
status: proposed
updated: 2026-10-06
related: [phase-3-alpha, dogfood-log]
nav_order: 4
---

# Phase 4 — Beta

Seeds, not a committed plan. This phase was previously filed as "v2 — Widget sidebar"; the work is unchanged, only the name and task IDs moved to match the phase structure.

## Goal

Extend the reader past a single content pane: a right-hand widget slot, a first set of widgets (context engine), and a machine-readable surface for agents, all on the same terminal-free core ([ADR-0006](../decisions/0006-reader-first.md)).

## Entry criteria

- Phase 3 exit decision recorded (met 2026-10-06) against [its exit criteria](phase-3-alpha.md#exit-criteria): chrome, lite build and viewer polish shipped; herdr integration confirmed on a real install or explicitly deferred here. Deferred integration requires the API spike evidence before implementation.
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
| P4-09 | Entry-point link lists in the widget sidebar | | todo | README, summary and overview pages often list the whole collection or a reading order. Surface those links in the context widget or sidebar for quick preview and use on large trees. Builds on P4-01 and P4-02. Filed 2026-10-05 |
| P4-10 | Summarize tool through agent harnesses | | todo | Start with Pi. Scopes: a page, a sub-collection (a folder such as roadmap or architecture), the whole wiki; scope and efficiency tuned through testing. It can start as an agent tool or skill that only needs wiki-reader (or the crate) installed, not the TUI. TUI access later: a window to prompt, or a hotkey for a default summary. First slice: a hotkey that copies a prompt with the right file refs, to paste into an agent elsewhere; the fuller module follows research, specs and spikes. Relates to P4-03. Filed 2026-10-05 |
| P4-11 | Pi integration | | todo | Separate from P4-10: serves more than summaries — workflow and source-code context outside the wiki (the wiki as docs, task tracking, product foundations, architecture specs). Filed 2026-10-05 |
| P4-12 | Sticky viewer section header | U1 | todo | Carried from P3-01 (operator, 2026-10-03; Phase 3 exit 2026-10-06). Revisit now that the alpha chrome has settled |
| P4-13 | Relayout and reindex: reuse unchanged pages | | todo | Carried from P3-33 (V3 in the [v0.1.8 audit](audit-v0.1.8.md)): reuse unchanged pages, skip reload when the open page is unchanged. Low priority; no measured cost |
| P4-14 | Media: measure then decide | | todo | Carried from P3-35 (V6, V15, V16): harness SVG + image-plan, Kitty byte counter, LRU cap with the fallback reason kept. Measure first; operator manual pass in Ghostty and herdr |
| P4-15 | Process and tooling hygiene | | todo | Carried from P3-36: V17 remainder (reap children, herdr timeout, surface open errors), V19 `release.yml` split into a read-only build job and a minimal write job with trusted publishing (after the first crates.io release; dry run first), V21 (`check_doc` via `core::nav::resolve`), boolean-argument tidy-ups |

## Deferred ideas feeding this phase

- Right-hand context/widget sidebar — requirements not clear yet ([vision](../product/vision.md)).
- Herdr context signals (sibling pane cwds and agent states) — see [integrations](../architecture/integrations.md).
- A side nav footer for widget actions or tabbed features (Pages / Outline), designed as a future slot in the [UI spec](../product/ui-spec.md).

## Beyond Phase 4 (uWiki)

- uWiki as a backend that keeps wikis updated or evolves living docs, with wiki-reader as the fast terminal view without rebuilding a static site. High level only: uWiki has no public release and its workspace needs work first.

## Related

- [Phase 3](phase-3-alpha.md)
- [ADR-0006](../decisions/0006-reader-first.md) — core stays terminal-free for this
- [Context engine](../architecture/context-engine.md)
