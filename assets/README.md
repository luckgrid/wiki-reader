# Screenshots

The documentation screenshots of the app browsing its own wiki. They are not test fixtures: they are large and change with the UI. Fixtures for image tests live under `fixtures/` and stay tiny.

- `wiki-reader.png`: the main layout, used by the root [README](../README.md) and the [UI spec](../wiki/product/ui-spec.md).
- `wiki-reader-help.png`, `wiki-reader-search.png`: the Help and Search popups, shown in the UI spec.
- `wiki-reader-options.png`: the options window, shown in the UI spec and the [configuration guide](../wiki/guides/configuration.md).

`wiki/assets/` holds an identical copy of every file so wiki pages can show them from inside the collection root (images outside it are not rendered, [ADR-0017](../wiki/decisions/0017-static-local-images-only.md)). Update both copies when retaking a screenshot. The files were retaken on 2026-10-04 after the Batch C and chrome work (new theme, bordered bars, footer icons, carousel keys).

The four main PNGs predate the P3-30 table-viewer expand; retake them after the V22 fix / at the Phase 3 exit decision.
