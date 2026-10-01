---
id: WR-ADR-0017
title: "ADR-0017: Images are static, local and never fetched"
summary: Render only static image files that live inside the collection; never fetch remote images; fall back to a text placeholder.
status: accepted
updated: 2026-10-01
related: [0004-diagram-rendering, 0006-reader-first]
---

# ADR-0017: Images are static, local and never fetched

**Status:** Accepted · **Date:** 2026-10-01

## Context

Phase 3 adds image rendering ([P3-12](../roadmap/phase-3-alpha.md)): local images and, via [ADR-0004](0004-diagram-rendering.md), Mermaid diagrams as images. A reader that opens arbitrary collections must not let a markdown file make the app touch the network, read files outside the collection, or run unbounded decode work.

## Decision

- **Never fetch.** `http(s)://` and other URL images are never requested, previewed or cached. They render as the text placeholder.
- **Local files only, inside the collection.** A path resolves relative to the page, is canonicalised (symlinks resolved), and must stay under the collection root. Anything else (`..` escapes, absolute paths outside the root, `file:` URLs) renders as the placeholder.
- **Static formats only.** PNG, JPEG, WebP and the first frame of a GIF; SVG is rasterised by `resvg` with no external references and no scripting. Animation is not played.
- **Bounded.** A file size cap and a decoded-pixel cap apply before decode; decode and raster work stay off the UI thread.
- **Text fallback always works.** When the terminal has no usable graphics protocol (or the image is rejected, missing or too large), the block shows `[image: alt]` with the path and the reason, the same shape as the source tier for diagrams. The image tier is an upgrade, never a requirement.
- Mermaid image diagrams ([ADR-0004](0004-diagram-rendering.md)) follow the same rules: generated in-process from fenced source, no network, no files.

## Consequences

- ➕ Opening an untrusted collection cannot cause network traffic or reads outside it.
- ➕ One fallback shape for every failure, easy to test without a graphics terminal.
- ➖ Remote and animated images show as text. Revisit only behind an explicit opt-in config key, with a new ADR.
