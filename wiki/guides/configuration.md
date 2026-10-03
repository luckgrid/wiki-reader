---
id: WR-GUIDE-CONFIG
title: Configuration
summary: Where wiki-reader reads and writes config, how files merge, every key with its default, and what the options window changes.
status: draft
updated: 2026-10-03
related: [development, releasing]
---

# Configuration

Config is optional. Every key has a default, and a bad value never stops startup: it is ignored, a diagnostic shows in the status bar, and the earlier value (or the default) stays.

## Files and merge order

Files are merged in this order, and a later file wins for a key it sets:

1. **User file:** `$XDG_CONFIG_HOME/wiki-reader/config.toml`, else `~/.config/wiki-reader/config.toml`.
2. **Collection file:** `<collection root>/.wiki-reader.toml`.
3. **`--config PATH`:** an explicit file, if given.

The collection file is untrusted, since it comes with the content you are reading. `opener`, `editor` and `keys` are ignored there (with a diagnostic) and are honoured only in the user file or `--config`. `exclude` patterns from all files are combined.

## Keys

| Key | Values | Default | Notes |
|-----|--------|---------|-------|
| `theme` | `"dark"`, `"light"`, `"herdr"` | `"dark"`, or `"herdr"` inside herdr | `dark` and `light` use the luckgrid.net colours and paint their own background; `herdr` follows the theme in herdr's config. See [UI spec: Theming](../product/ui-spec.md) and [ADR-0019](../decisions/0019-theme-presets.md). |
| `nav.position` | `"left"`, `"right"` | `"left"` | Side nav on the left or right edge. Nav width stays session-only. |
| `nav.labels` | `"title"`, `"filename"` | `"title"` | Side-nav page titles or actual filenames including extensions; folders keep on-disk names. Header/footer labels stay title-based. |
| `diagrams` | `"auto"`, `"image"`, `"text"`, `"source"` | `"auto"` | Mermaid tier; see [ADR-0004](../decisions/0004-diagram-rendering.md). tmux always uses text. |
| `images.enabled` | `true`, `false` | `true` | `false` skips the terminal graphics probe and shows text placeholders. |
| `images.max_slot_rows` | integer `1`–`60` | `30` | Tallest picture or diagram slot, in rows. |
| `copy.path` | `"relative"`, `"absolute"` | `"relative"` | What `y` ("Copy file path" in Help) copies: with the nav focused, the selected row's file or folder path; with the viewer focused, the open page's path. Relative to the collection root, or absolute. Also a row in the options window. |
| `exclude` | glob, or list of globs | none | Paths left out of the collection. |
| `opener` | command string | system opener | Opens external URLs: program, then its args, then the URL. User file or `--config` only. |
| `editor` | command string | `$EDITOR` | User file or `--config` only. |
| `keys` | table of action name → chord | none | User file or `--config` only, for example `quit = "Q"`. |

```toml
theme = "herdr"
copy.path = "absolute"

[nav]
position = "right"
labels = "filename"

[images]
max_slot_rows = 24
```

Legacy `nav.labels = "title+filename"` is read as `"title"` with one warning per config load, even if several config layers use it. Loading does not rewrite files; choose a supported value in Options or edit the config to stop the warning. See [ADR-0020](../decisions/0020-nav-label-modes.md).

## Options window

`,` or `c` (or the layout footer ⚙) opens the options window, and the same keys, or `Esc`, close it. Each setting is a group of radio rows: `↑` / `↓` move, `Enter` applies. It edits `theme`, `nav.position`, `nav.labels`, `diagrams`, `images.enabled`, `images.max_slot_rows` and `copy.path`. Each change applies at once and is saved to one key of the file, per [ADR-0018](../decisions/0018-config-write-path.md):

- **Write target:** `--config PATH` if you started with it, else the user file. The collection `.wiki-reader.toml` is never written. The file and its folder are created when missing, and comments and unknown keys are kept.
- **A collection file can win on restart.** If `.wiki-reader.toml` sets the same key, it overrides the value saved to the user file the next time you start. The status bar says so when you change such a key. Edit or remove the key in the collection file, or start with `--config` to make the saved value final.
- **Images:** turning images on after they were off at startup needs a restart, because terminal graphics are probed once.
