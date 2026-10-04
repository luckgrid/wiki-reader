# Screenshots

The documentation screenshot of the app browsing its own wiki, used by the root [README](../README.md). They are not test fixtures: they are large and change with the UI. Fixtures for image tests live under `fixtures/` and stay tiny.

`wiki/assets/` holds an identical copy of it so wiki pages can show it from inside the collection root (images outside it are not rendered, [ADR-0017](../wiki/decisions/0017-static-local-images-only.md)), plus the help and search screenshots, which only the wiki shows. Update both copies of `wiki-reader.png` when retaking it.

P3-23 refreshed the README / ui-spec ASCII diagram and documented the carousel keys. The PNG retake (`assets/wiki-reader.png`, `wiki/assets/wiki-reader.png`, and ideally help/search) still needs an operator pass in Ghostty with Screen Recording permission — the agent session could not write a display capture.
