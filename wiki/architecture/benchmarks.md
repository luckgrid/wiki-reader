---
id: WR-BENCH
title: Benchmarks
summary: Binary size, memory and start-up compared with other terminal markdown tools.
status: draft
updated: 2026-10-03
related: [prior-art-and-libs]
nav_order: 5
---

# Benchmarks

Installed binary, memory, start-up and crate count were measured on 2026-10-02 (local time) on macOS arm64, wiki-reader at `main` `976e3ff` (release profile, after the options window, viewers and themes) against the markdown-reader 1.34.75 Homebrew binary. These measurements were carried forward, not re-measured at `a153696`. Memory and start-up used a pseudo-terminal opened on this repo's `wiki/` folder (42 pages), three runs each; no graphics protocol answered, so Mermaid stayed on the text tier. Only the release download size was updated for `a153696` (**v0.1.1**): the published macOS arm64 tarball is 6.7 MiB. The original measurement note used the UTC date 2026-10-03.

| | wiki-reader | wiki-reader lite (P3-19) | markdown-reader | treemd 0.9.1 | glow 3.0.0 |
|---|---|---|---|---|---|
| Installed binary | 16.7 MiB | — | 13.1 MiB | not measured | not measured |
| Release download (macOS arm64 tarball) | 6.7 MiB (v0.1.1) | — | 6.0 MiB | 4.4 MiB | 6.1 MiB |
| Peak memory at start | 14.0 MiB | — | 12.3 MiB | not measured | not measured |
| Time to first output (warm) | ≈ 35 ms | — | ≈ 20 ms | not measured | not measured |
| Crates built (unique, normal deps, incl. workspace) | 229 | — | not measured | not measured | Go, n/a |

What this says: wiki-reader's binary is about 27 % larger than markdown-reader's on disk (16.7 against 13.1 MiB) and uses about 14 % more memory at start (14.0 against 12.3 MiB). Both start in well under a tenth of a second once warm; the first run after a fresh build took 0.57 s for wiki-reader, so treat start-up as indicative only. The release downloads are about 12 % apart (6.7 against 6.0 MiB).

Where the extra size comes from: the Mermaid image tier (`mermaid-rs-renderer`, `resvg`, an embedded font) was +6.7 MiB in P3-12c, the largest single step. Everything since alpha.5 (themes, nav position, the options window with `toml_edit`, both viewers) added about 0.3 MiB in total. Subtracting the image step gives roughly 10 MiB, which is below markdown-reader's 13.1 MiB. That figure is an estimate, not a build: a build without the image tier is [P3-19](../roadmap/phase-3-alpha.md) (the lite column above). markdown-reader also draws Mermaid as terminal images (Kitty, Sixel, iTerm2), so the two binaries carry comparable image stacks and the comparison is like for like on that feature.

Not measured: treemd and glow memory and start-up (not installed; only the published archive sizes were read), md-tui and Frogmouth (no comparable download; Frogmouth is Python), and memory while viewing a large Mermaid diagram or many images. Treat the table as one machine and one collection, not a benchmark.

## Related

- [Prior art & libraries](prior-art-and-libs.md)
