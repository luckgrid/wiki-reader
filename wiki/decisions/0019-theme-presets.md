---
id: WR-ADR-0019
title: "ADR-0019: luckgrid presets, and `herdr` follows herdr's config"
summary: dark and light take the luckgrid.net palette; the herdr preset reads herdr's selected theme from its config file, and is the default inside herdr.
status: accepted
updated: 2026-10-03
related: [0018-config-write-path, 0004-diagram-rendering]
---

# ADR-0019: luckgrid presets, and `herdr` follows herdr's config

**Status:** Accepted · **Date:** 2026-10-03

## Context

The first `dark` preset looked too much like the `herdr` preset (peach and mint on near-black), so the choice between them barely showed. The `herdr` preset was also fixed to one herdr theme (vesper), while herdr offers several, and a reader run in a herdr pane should match its host without extra setup.

herdr does not expose its theme to child programs: no environment variable, and its socket API has no theme call. The selected theme name is in `~/.config/herdr/config.toml` (`[theme] name`).

## Decision

- **`dark` and `light` are the luckgrid.net palettes** (`src/styles/theme.css` of that site, OKLCH converted to sRGB): lime accent on dark, cyan-blue on light, and the site's syntax hues for links, code and alerts. Text colours that must read on white are the site's hues darkened to ≥ 4.5:1. Surfaces stay the terminal's own colours.
- **`herdr` follows herdr's theme.** At startup wiki-reader reads `[theme] name` from herdr's config (`$XDG_CONFIG_HOME/herdr/config.toml`, else `~/.config/herdr/config.toml`) and uses the matching palette: `vesper`, `catppuccin`, `catppuccin-latte`, `tokyo-night`, `tokyo-night-day`, `gruvbox`, `gruvbox-light`, `one-dark`, `kanagawa`, and `terminal` (the host's ANSI colours). An unknown or missing name falls back to vesper, and an unknown name says so in the status bar. With `auto_switch = true`, `name` is used if present, else `dark_name`.
- **Inside herdr (`HERDR_ENV=1`) the default theme is `herdr`**, but only when no config file sets `theme`. Choosing a theme in the options window writes it, so that choice then wins everywhere.
- **Palettes are derived, not hand-painted.** One small table per preset; every token is derived from it, and a test checks each preset for ≥ 4.5:1 text contrast.

## Consequences

- ➕ `dark` and `herdr` look different, and `herdr` matches the host with no setup.
- ➕ No plugin and no dependency: a read-only look at one TOML key.
- ➖ The herdr palettes are built from each theme's published colours, not from herdr's own values, so they can differ slightly from what herdr draws. Light variants are darkened where needed to read on white.
- ➖ The theme is read once at startup. Changing it in herdr needs a wiki-reader restart. Following it live, honouring `[theme.custom]` overrides and `auto_switch` light/dark need a herdr plugin (or an API herdr does not have yet): Phase 4, [P4-05](../roadmap/phase-4-beta.md).
