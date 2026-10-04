# wiki-reader wiki

Entrypoint for wiki-reader documentation: product intent, architecture, decisions, roadmap, and developer guides.

## Collections

- [Product](product/README.md) — vision, spec, content model, UI.
- [Architecture](architecture/README.md) — crates, rendering, prior art, benchmarks, integrations; deferred context engine.
- [Decisions](decisions/README.md) — ADRs (immutable once accepted — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed).
- [Roadmap](roadmap/README.md) — phases, exit criteria, task trackers, and the [dogfood log](roadmap/dogfood-log.md).
- [Guides](guides/README.md) — how to develop in this workspace.

## Reading order

1. [Vision](product/vision.md) — why this exists.
2. [Product spec](product/spec.md) — stories and acceptance criteria.
3. [Architecture](architecture/overview.md) — crates and navigation core.
4. [Content model](product/content-model.md) — pages, links, nav tree.
5. [UI spec](product/ui-spec.md) — layout, focus, keys, mouse.
6. [Rendering](architecture/rendering.md) — markdown and diagrams.
7. [Prior art & libraries](architecture/prior-art-and-libs.md) — Phase 0 findings and deps.
8. [Benchmarks](architecture/benchmarks.md) — binary size, memory and start-up.
9. [Roadmap](roadmap/README.md) — what to build next.
10. [Integrations](architecture/integrations.md) — external providers, design tokens, herdr (non-blocking).
11. [Context engine](architecture/context-engine.md) — deferred to Phase 4.

## Decisions (ADRs)

The full list with statuses is the index in [decisions/README.md](decisions/README.md).

## Document standard

Every non-README document uses this header:

```yaml
---
id: …
title: …
summary: …
status: draft | proposed | accepted | active | done | deferred | superseded | historical
updated: YYYY-MM-DD
related: [slug, …]
nav_order: N   # optional; product and architecture pages use this for reading order
---
```

| Status | Meaning |
|--------|---------|
| `draft` | Being written, not reviewed |
| `proposed` | Written, awaiting a decision, or planned but not started |
| `accepted` | Reviewed and settled; for ADRs, immutable |
| `active` | Living doc, in progress now |
| `done` | Finished work item / spike / phase |
| `deferred` | Postponed |
| `superseded` | Replaced by another doc |
| `historical` | Kept for the record, no longer maintained |

`id` and `summary` are part of the product [content model](product/content-model.md). `title`, `updated`, and `related` follow common wiki frontmatter conventions. `link-check` rejects unknown `status` values under `wiki/**` (non-README).

The body starts with `# Title`, a short summary paragraph, then sections. Optional `## Open questions` and `## Related` where they add value. Decisions live under `decisions/` and are immutable once accepted — supersede, don't edit; mechanical metadata or formatting fixes that don't change decision text are allowed.
