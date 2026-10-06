---
id: WR-ADR-0023
title: "ADR-0023: The lite build is a cargo feature, not a separate artifact"
summary: Images, SVG and Mermaid rasterisation sit behind a default-on `media` cargo feature; lite is `cargo install --no-default-features`, with no `-lite` release tarball.
status: accepted
updated: 2026-10-03
related: [0004-diagram-rendering, 0006-reader-first, 0017-static-local-images-only]
---

# ADR-0023: The lite build is a cargo feature, not a separate artifact

**Status:** Accepted · **Date:** 2026-10-03

**Note (2026-10-06):** the TUI package was renamed `wiki-reader-tui` for crates.io; the installed command is still `wiki-reader`. Read `-p wiki-reader` and the `… wiki-reader` install commands below as `wiki-reader-tui`. The decision is unchanged.

## Context

The image stack (`image`, `resvg`, `mermaid-rs-renderer`, `ratatui-image`, an embedded font) is the largest single step in binary size and build time; see [benchmarks](../architecture/benchmarks.md). Some readers never use a graphics terminal and only want text, source and placeholders. The text tier (`mermaid-text`) already works without any of it ([ADR-0004](0004-diagram-rendering.md)).

## Decision

- **One feature, default on.** `wiki-reader`, `wiki-reader-render` and `wiki-reader-media` each have a `media` (or `raster`) feature. The binary's `media` forwards to `wiki-reader-render/media` and enables `image` and `ratatui-image`. Lite is `--no-default-features`.
- **Distribution.** Lite is `cargo install --locked --no-default-features --git … wiki-reader`. There is **no `-lite` release tarball**. Revisit only if someone asks for a prebuilt lite binary.
- **A `cfg` boundary, not a trait.** The seam is small: render asks the media crate for pixel sizes and the TUI owns the decode worker, so a plain `cfg` split is enough. Terminal-protocol probing and drawing stay in the TUI.
- **Crates.** `wiki-reader-media` holds the Mermaid and SVG rasteriser, bitmap decode and the embedded font behind its `raster` feature. The palette and fit maths are always built, because render's types name them. Media never depends on render.
- **Lite behaviour.** Every image is a text placeholder. `diagrams = "image"` falls back to the text tier with the visible reason `lite build: no image tier`. The Images and Max image rows option groups are hidden. The image viewer is absent.
- **Guard.** `scripts/check.sh` and CI run clippy and tests for both builds and fail if `cargo tree --no-default-features -p wiki-reader -e normal` lists `image`, `resvg`, `mermaid-rs-renderer` or `ratatui-image`.

## Consequences

- Default behaviour and snapshots are unchanged; the lite build has its own, smaller set of tests.
- CI cost goes up by one clippy and one test run.
- `RenderedDoc` carries a tier-independent media inventory (images and Mermaid fences, with occurrence ids that survive relayout) so later viewer work does not lose text-tier diagrams or placeholders.
- Cargo features are additive: building the whole workspace (`cargo build --workspace`) unifies to the default. Only a `-p wiki-reader --no-default-features` build is lite.

## Related

- [ADR-0004](0004-diagram-rendering.md)
- [ADR-0017](0017-static-local-images-only.md)
