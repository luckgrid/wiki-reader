# Herdr popup plugin

Requires Herdr **0.9.0+** on Linux or macOS and a wiki-reader binary supporting
`--herdr-context` on Herdr's `PATH`. The installed v0.1.3 binary does not support
this flag; build/install the implementation revision until v0.1.4 is released.
The manifest installs no build commands, startup hooks or event hooks.

From the repository root, after reviewing `herdr-plugin.toml`:

```bash
cargo install --locked --path crates/wiki-reader
herdr plugin link "$PWD/integrations/herdr"
herdr plugin pane open --plugin wiki-reader --entrypoint reader
# Equivalent action, suitable for binding a key:
herdr plugin action invoke wiki-reader.open
```

The popup is 80% of the terminal's width and height. `q` exits wiki-reader and
closes it; `Esc` dismisses wiki-reader's own overlays rather than exiting.
Herdr 0.9.0's CLI help omits popup placement, but the manifest works on that
version (see the [spike evidence](../../wiki/roadmap/spikes/p3-s2-herdr-integration.md)).
Overlay and split graphics are not promised by this plugin.

**Shipping acceptance is currently blocked:** the implementation retest on
Herdr 0.9.0 passes collection selection and keys, but falls back to text because
the popup supplies no cell metrics. Kitty is confirmed; ratatui-image drops that
result without a measured font size. See the [retest evidence](../../wiki/roadmap/spikes/p3-s2-herdr-integration.md#p3-09-implementation-retest--2026-10-03).
A longer probe timeout did not help.

Herdr starts plugin commands in the plugin directory. `--herdr-context` reads
`HERDR_PLUGIN_CONTEXT_JSON`: an explicit collection root wins; otherwise it uses
`focused_pane_cwd`, then `workspace_cwd` when the focused cwd is absent or empty.
Missing/malformed context or a selected path that is not a directory silently
falls back to `.` (the plugin directory). Without the flag, the root still
defaults to `.` and explicit invalid roots still fail normally.

Popup processes have no `HERDR_PANE_ID`. They must not publish metadata to the
underlying focused pane, which may host an unrelated agent.

Optional binding in Herdr's config (choose a key that is free in your setup):

```toml
[[keys.command]]
key = "prefix+w"
type = "plugin_action"
command = "wiki-reader.open"
description = "open wiki reader"
```

The action invokes `"$HERDR_BIN_PATH" plugin pane open`; no socket client or
extra runtime beyond the POSIX shell is required. A missing wiki-reader binary
cannot open the reader. Inspect Herdr's plugin logs for launch failures:

```bash
herdr plugin log list --plugin wiki-reader
herdr plugin unlink wiki-reader
```

Unlinking unregisters the plugin without deleting the checkout or Herdr's
plugin config/state directories. Linking or changing keys is an operator action;
these commands are not run automatically by wiki-reader.
