# Screenshots

The documentation screenshot of the app browsing its own wiki, used by the root [README](../README.md). They are not test fixtures: they are large and change with the UI. Fixtures for image tests live under `fixtures/` and stay tiny.

`wiki/assets/` holds an identical copy of it so wiki pages can show it from inside the collection root (images outside it are not rendered, [ADR-0017](../wiki/decisions/0017-static-local-images-only.md)), plus the help and search screenshots, which only the wiki shows. Update both copies of `wiki-reader.png` when retaking it. A full retake (new theme, footer icons, tab/footer borders) waits for [P3-23](../wiki/roadmap/phase-3-alpha.md) so screenshots are not taken twice.
