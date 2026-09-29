# wiki-reader wiki

Entrypoint for wiki-reader documentation: product intent, architecture, decisions, roadmap, and developer guides.

## Collections

- [Product](product/README.md) — vision, spec, content model, UI.
- [Architecture](architecture/README.md) — crates, rendering, prior art, integrations; deferred context engine.
- [Decisions](decisions/README.md) — ADRs (immutable once accepted; supersede, don't edit).
- [Roadmap](roadmap/README.md) — phases, exit criteria, and task trackers.
- [Guides](guides/README.md) — how to develop in this workspace.

## Reading order

1. [Vision](product/vision.md) — why this exists.
2. [Product spec](product/spec.md) — stories and acceptance criteria.
3. [Architecture](architecture/overview.md) — crates and navigation core.
4. [Content model](product/content-model.md) — pages, links, nav tree.
5. [UI spec](product/ui-spec.md) — layout, focus, keys, mouse.
6. [Rendering](architecture/rendering.md) — markdown and diagrams.
7. [Prior art & libraries](architecture/prior-art-and-libs.md) — Phase 0 findings and deps.
8. [Roadmap](roadmap/README.md) — what to build next.
9. [Integrations](architecture/integrations.md) — uwiki, design-system, herdr (non-blocking).
10. [Context engine](architecture/context-engine.md) — deferred v2 draft.

## Decisions (ADRs)

| ADR | Status |
|-----|--------|
| [0001 Rust + ratatui](decisions/0001-rust-ratatui.md) | accepted |
| [0002 Build on existing pieces](decisions/0002-build-vs-fork.md) | accepted |
| [0003 Core + CLI first](decisions/0003-core-cli-first.md) | superseded by 0006 |
| [0004 Tiered diagram rendering](decisions/0004-diagram-rendering.md) | proposed |
| [0005 Wiki navigation model](decisions/0005-navigation-model.md) | accepted |
| [0006 Reader first; core terminal-free](decisions/0006-reader-first.md) | accepted |
| [0007 Input & focus model](decisions/0007-input-focus-model.md) | accepted |
| [0008 Side nav as site navigation](decisions/0008-side-nav-as-site-nav.md) | accepted |

## Document standard

Every non-README document uses this header:

```yaml
---
id: …
title: …
summary: …
status: draft | accepted | proposed | superseded | deferred | historical
updated: YYYY-MM-DD
related: [slug, …]
nav_order: N   # optional; product and architecture pages use this for reading order
---
```

`id` and `summary` are part of the product [content model](product/content-model.md). `title`, `updated`, and `related` follow the workstation wiki convention.

The body starts with `# Title`, a short summary paragraph, then sections. Optional `## Open questions` and `## Related` where they add value. Decisions live under `decisions/` and are immutable once accepted — supersede them; don't edit them.
