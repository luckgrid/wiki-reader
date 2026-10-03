---
id: WR-ADR-0018
title: "ADR-0018: Options window writes the operator config file"
summary: Persist options to the XDG user config (or --config when used); never write untrusted repo config; keep comments and unknown keys via toml_edit.
status: accepted
updated: 2026-10-02
related: [0006-reader-first]
---

# ADR-0018: Options window writes the operator config file

**Status:** Accepted · **Date:** 2026-10-02

## Context

[P3-13](../roadmap/phase-3-alpha.md) adds an in-app options window that edits theme, nav position, nav labels, diagrams, images and copy-path format. Today config is **read-only**: XDG → `<root>/.wiki-reader.toml` → optional `--config`. A write path must pick a file, preserve comments and unknown keys, and keep the earlier-file-wins merge rule on the next load.

## Decision

- **Write target:** the operator's file — `$XDG_CONFIG_HOME/wiki-reader/config.toml` (or `~/.config/...`) by default. If the process was started with `--config PATH`, write that path instead. **Never** write untrusted collection `.wiki-reader.toml`.
- **Library:** `toml_edit` so comments and unknown keys survive a round-trip. Plain `toml` + `serde` cannot do that.
- **Persist from the options window:** `theme`, `nav.position`, `nav.labels`, `diagrams`, `copy.path`, and image keys (`images.enabled`, `images.max_slot_rows`).
- **Session-only (unchanged):** `nav_width` (P2-14).
- **Live apply:** mutate the in-memory `Config` / app fields and redraw without restart. Theme and diagrams changes invalidate the Mermaid / `DiagramPalette` cache and force re-layout.
- **Invalid values on read:** keep the earlier-file-wins rule (P3-07 / P2-55 / P3-11). The write path only emits typed, valid values.
- **Create the file** (and parent dirs) when the write target does not exist yet.

## Consequences

- ➕ One typed struct remains the source of truth (spec U5); the popup is not a second store.
- ➕ Collection-local `.wiki-reader.toml` stays a merge input, never overwritten by the app.
- ➖ A new dependency (`toml_edit`). Acceptable: comment-preserving rewrite is the requirement.
- ➖ Settings written with `--config` go to that override file, not XDG; document that in the options UI status line when relevant.
