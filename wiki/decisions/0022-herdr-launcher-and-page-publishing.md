---
id: WR-ADR-0022
title: "ADR-0022: herdr launcher opens an ordinary pane; page publishing is on by default"
summary: The herdr plugin opens the reader in an ordinary split pane because plugin panes cannot draw images, and wiki-reader publishes the current page to herdr's sidebar by default.
status: accepted
updated: 2026-10-03
related: [0004-diagram-rendering, 0006-reader-first, 0019-theme-presets]
---

# ADR-0022: herdr launcher opens an ordinary pane; page publishing is on by default

**Status:** Accepted · **Date:** 2026-10-03

## Context

Evidence is in the [P3-S2 spike](../roadmap/spikes/p3-s2-herdr-integration.md). On herdr 0.9.x, every pane started for a plugin command (overlay, popup, split or tab) gets no terminal cell metrics and never answers the cell-size query, so wiki-reader cannot size or draw images there. An ordinary shell pane answers (Kitty, 8×17 measured). herdr also lets a program publish display-only pane metadata (`herdr pane report-metadata`), which its sidebar shows. An ordinary herdr pane's environment already has `HERDR_BIN_PATH`, `HERDR_SOCKET_PATH` and `HERDR_PANE_ID`.

## Decision

- **Launcher.** The plugin's default action runs `wiki-reader --herdr-split`. It splits the focused pane in that pane's cwd (`herdr pane split`) and submits `exec wiki-reader` to the new shell (`herdr pane run`), so quitting closes the pane. Overlay and popup actions stay as text-only options. wiki-reader never guesses a cell size or forces a protocol, and its "no graphics" text says plugin panes report no cell size.
- **Publishing.** Inside herdr, in a pane with its own `HERDR_PANE_ID`, wiki-reader publishes the current page's title and a `page` token, **on by default**, with `[herdr] publish = false` to turn it off.
  - It spawns the herdr CLI from a worker thread; there is no socket client and no new dependency.
  - Reports are debounced (250 ms), carry a 10-minute TTL that is renewed every 4 minutes, and are cleared on exit. `--seq` is the Unix time in ms so a restarted reader is not ignored as stale.
  - It never reports agent lifecycle state, so the pane stays an ordinary pane. Plugin popups have no pane id and never publish.
  - A missing herdr binary or failing call never affects navigation; after three consecutive failures it stops for the session.

## Consequences

- Images work in the default herdr launch today, without depending on a herdr fix. If herdr later gives plugin panes cell metrics, the overlay and popup actions gain images with no wiki-reader change.
- Publishing by default writes to herdr's sidebar without being asked. It is display-only, expires by TTL and is cleared on exit; the opt-out is one config key. If wiki-reader is started from a shell in a pane that normally hosts an agent, it sets that pane's title and token while it runs.
- The launcher types a command into a shell, so wiki-reader must be on `PATH` in the user's shell as well as in herdr's plugin environment.
