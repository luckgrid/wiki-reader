# Herdr plugin

Opens wiki-reader from a [herdr](https://herdr.dev) key or command, in the focused pane's
directory. Requires herdr **0.9.0+** on Linux or macOS and a wiki-reader binary that supports
`--herdr-split` and `--herdr-context` (v0.1.4 or later; the v0.1.3 binary does not) on `PATH`.
The manifest declares no build commands, startup hooks or event hooks.

## Install

From the repository root, after reviewing `herdr-plugin.toml`:

```bash
cargo install --locked --path crates/wiki-reader
herdr plugin link "$PWD/integrations/herdr"
herdr plugin action invoke wiki-reader.open
```

To bind a key, add this to herdr's config (choose a key that is free in your setup):

```toml
[[keys.command]]
key = "prefix+w"
type = "plugin_action"
command = "wiki-reader.open"
description = "open wiki reader"
```

## Actions

| Action | Opens | Images |
|--------|-------|--------|
| `wiki-reader.open` (default) | An **ordinary split pane** to the right of the focused pane, in its directory | yes |
| `wiki-reader.open-overlay` | A zoomed plugin overlay | no (text only) |
| `wiki-reader.open-popup` | An 80% × 80% plugin popup | no (text only) |

**Why the default is a split pane.** On herdr 0.9.x, every pane started for a plugin command
(overlay, popup, split or tab) gets no terminal cell metrics and never answers the cell-size
query, so wiki-reader cannot size or draw images there; it shows text placeholders and diagrams
as text, and says so ("herdr plugin panes report no cell size, open the reader in a normal
pane"). An ordinary shell pane answers, so `open` creates one with `herdr pane split` and runs
`exec wiki-reader` in it, which means quitting the reader (`q`) closes the pane. wiki-reader never
guesses a cell size. See the
[spike evidence](../../wiki/roadmap/spikes/p3-s2-herdr-integration.md#p3-09-overlay-and-control-retest--2026-10-03).

The overlay and popup actions keep the keys, context and dismissal working, so they are fine for
a quick text read. Herdr 0.9.0's `--placement` help omits `popup`, but the manifest works on that
version. Popup commands have no `HERDR_PANE_ID` of their own, so a popup never publishes
page metadata.

## How the launcher chooses the directory

Herdr starts plugin commands in the plugin directory and passes `HERDR_PLUGIN_CONTEXT_JSON`.
`--herdr-context` (overlay and popup) and `--herdr-split` (the default action) use its
`focused_pane_cwd`, then `workspace_cwd`. An explicit collection root wins over the context.
Missing or malformed context, or a path that is not a directory, falls back to the current
directory for `--herdr-context`; `--herdr-split` then lets herdr pick the target pane and apply
its own `terminal.new_cwd` policy. `--herdr-split` cannot be combined with a root, `--config`
or `--herdr-context`.

## Troubleshooting

`wiki-reader.open` runs `wiki-reader --herdr-split` as the plugin command, then types
`exec wiki-reader` into the new shell, so wiki-reader must be on `PATH` both for herdr's plugin
environment and in your shell. If it is missing, the action fails (the new split pane may remain
open and show the shell's error). Inspect plugin command output with:

```bash
herdr plugin log list --plugin wiki-reader
```

Unlinking unregisters the plugin without deleting the checkout or herdr's plugin config and
state directories. Linking, unlinking and changing keys are operator actions; wiki-reader never
runs them for you:

```bash
herdr plugin unlink wiki-reader
```
