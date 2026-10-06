# Screenshots

The documentation screenshots of the app browsing its own wiki. They are not test fixtures: they are large and change with the UI. Fixtures for image tests live under `fixtures/` and stay tiny.

- `wiki-reader.png`: the main layout, used by the root [README](../README.md) and the [UI spec](../wiki/product/ui-spec.md).
- `wiki-reader-help.png`, `wiki-reader-search.png`: the Help and Search popups, shown in the UI spec.
- `wiki-reader-options.png`: the options window, shown in the UI spec and the [configuration guide](../wiki/guides/configuration.md).
- `wiki-reader-table-viewer.png`, `wiki-reader-table-viewer-filter.png`: the table viewer with the focused row expanded, and with a `/` filter applied. Shown in the UI spec.
- `wiki-reader-diagram-viewer.png`, `wiki-reader-diagram-viewer-zoom.png`, `wiki-reader-media-viewer.png`: the diagram viewer at fit and at 200 % zoom, and the image viewer over a picture slot. Shown in the UI spec.

`wiki/assets/` holds an identical copy of every file so wiki pages can show them from inside the collection root (images outside it are not rendered, [ADR-0017](../wiki/decisions/0017-static-local-images-only.md)). Update both copies when retaking a screenshot. The first four files were retaken on 2026-10-04 after the Batch C and chrome work (new theme, bordered bars, footer icons, carousel keys); the viewer screenshots were taken on 2026-10-06 (v0.1.10).

The four main PNGs predate the P3-30 table-viewer expand; retake them at the Phase 3 exit decision if the main layout has changed.
