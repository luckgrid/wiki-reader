# Architecture

How wiki-reader is structured: crates, navigation core, rendering, and seams for later integrations.

## Reading order

1. [Overview](overview.md) — crates, navigate(), hit map, runtime.
2. [Rendering](rendering.md) — markdown, links, diagrams.
3. [Prior art & libraries](prior-art-and-libs.md) — Phase 0 findings and crate choices.
4. [Integrations](integrations.md) — external providers, design tokens, herdr (non-blocking).
5. [Context engine](context-engine.md) — deferred to Phase 4.

## Related

- [Product](../product/README.md)
- [Decisions](../decisions/README.md) — especially [ADR-0005](../decisions/0005-navigation-model.md) and [ADR-0006](../decisions/0006-reader-first.md)
