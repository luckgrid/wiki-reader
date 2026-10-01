---
id: WR-LIBS
title: Prior art & libraries
summary: Existing tools worth learning from and the libraries wiki-reader builds on.
status: draft
updated: 2026-09-29
related: [overview, rendering]
nav_order: 4
---

# Prior art & libraries

> Versions move fast. Pin exact versions at scaffold time, and check crates.io and each repo's license before adding a dependency.

## Prior art

| Tool | Stack | What to take | Why not just use it |
|------|-------|--------------|---------------------|
| **markdown-reader** (`markdown-tui-explorer`, leboiko) | Rust, ratatui, MIT | Repo tree + tabs, inline Mermaid (image and text), tables, frontmatter box, live reload, link checker. **Closest match; main source to borrow from.** | Two-pane layout; no index, backlinks, or context; editing-focused features we don't need. |
| **treemd** | Rust | Outline-as-tree navigation; jq-like query CLI (`-q '.h2[...]'`), a good model for our CLI ergonomics | Single-document focus |
| **md-tui** (`mdt`) | Rust | Link navigation keys, recursive file-tree discovery | No sidebars/context |
| **Frogmouth** | Python, Textual | Browser-like history, bookmarks, TOC panel UX | Different stack; no diagrams |
| **Glow** | Go, Glamour | Rendering aesthetics | Presentation only |
| **MDDock** | Rust, ratatui | TUI + MCP server over the same notes. Validates "same index for humans and agents." | Notes app, not a wiki reader |
| **herdr-image** | Bash + Kitty | Proof that Kitty graphics work in herdr side panes | Images only |
| **superfile (spf)** | Go | What you're replacing for this use | Weak markdown preview |

## Phase 0 findings: markdown-reader in daily use

Tried as-is inside herdr. **Kept:** it's markdown-only, reads far better than superfile's preview, and is a great quick reader for a directory of markdown. It stays in the toolbox for that.

**Why it doesn't fit wiki browsing** (each maps to a requirement in [product spec](../product/spec.md)):

| Observation | Root cause | wiki-reader answer |
|-------------|-----------|-----------------|
| Behaves like an editor: tree and viewer are separate panels with editor semantics | Designed for reading *and editing* repos | Reader-only; one navigation model ([ADR-0005](../decisions/0005-navigation-model.md)) |
| Picking another file doesn't replace the file in the current tab; tabs are the primary way to hold pages | Tab-centric, IDE-style model | Replace-by-default + history; tabs opt-in (N2, T1) |
| Links in documents can't be clicked or entered | Links rendered as styled text, no hit regions or focus | Link spans + hit map + Tab/Enter focus (L1–L4) |
| Search opens results in new tabs; the tab picker is unintuitive | Search is a modal feeding the tab system | Search overlay opened from anywhere or the side nav search row; results replace the view (S1) |
| Search isn't reachable by mouse | Key-only entry point | Clickable `/ Search…` row at the top of the side nav (S1) |
| No site-style orientation | Not a wiki goal for that tool | Breadcrumbs, prev/next, tree sync, Linked from (H1, F1, N4, B1) |

**Still worth borrowing:** its rendering (tables, code, frontmatter box, wrapping), Mermaid pipeline, `.gitignore`-aware discovery, live reload, and theme approach. See [ADR-0002](../decisions/0002-build-vs-fork.md).

## Libraries

### Core (`wiki-reader-core`)

| Need | Crate | Notes |
|------|-------|-------|
| Markdown parsing | `pulldown-cmark` | Same parser markdown-reader uses, which keeps the render port simple. Enable tables, tasklists, strikethrough, footnotes; check its wikilink option. Alternative: `comrak` (full AST with sourcepos, wikilinks/front-matter extensions) if the event model gets painful. |
| Frontmatter | own splitter + `toml` + a maintained YAML crate | `serde_yaml` is archived. Evaluate current maintained options at scaffold time. |
| File discovery | `ignore` | `.gitignore`-aware walking (from ripgrep) |
| Watching | `notify` + `notify-debouncer-mini` | Markdown-path filtered; rebuild off UI thread |
| Regex (IDs) | `regex` | |
| Globs | `globset` | Path rules, `applies_to` |
| Git | shell out to `git` in POC → `gix` later | Shelling out is simpler and good enough at ~2 s refresh |
| Fuzzy match | hand-rolled subsequence scorer | `nucleo` considered for Helix-class fuzzy; skipped — current scorer covers `tkn`→token and multi-word any-order pages (`projection token` → Token Projection). Revisit if lists get huge. |
| Full-text search | in-memory scan (POC) → `tantivy` only if needed | |
| Config | `serde` + `toml` (+ `figment` if layering gets complex) | |
| Paths | `directories` / `dirs` | XDG config/state/cache |

### Render (`wiki-reader-render`)

| Need | Crate | Notes |
|------|-------|-------|
| Code/raw highlighting | `syntect` (pure-Rust regex backend) | Also highlights raw markdown |
| Mermaid → text | `mermaid-text` | By markdown-reader's author; flowchart, sequence, state, class, ER, and more. Pin ≥ 0.56.1 (earlier versions had a multibyte label bug). |
| Mermaid → SVG | `mermaid-rs-renderer` | Pure Rust, no Node or Chromium |
| SVG → PNG | `resvg` + `image` | |
| Terminal images | `ratatui-image` | Kitty/Sixel/iTerm2/halfblocks. Force Kitty or off under herdr (see [rendering](rendering.md)). |
| Width | `unicode-width` | CJK/emoji-safe wrapping |

### App (`wiki-reader`)

| Need | Crate | Notes |
|------|-------|-------|
| TUI | `ratatui` 0.30.x + `crossterm` | 0.30 split into `ratatui-core`/`ratatui-widgets`. herdr itself is built on ratatui 0.30 + crossterm 0.29. |
| Tree widget | flat `HitMap` rows ([ADR-0010](../decisions/0010-flat-side-nav-rows.md)) | Custom visible-row list; `tui-tree-widget` evaluated then rejected (ADR-0009 superseded) because `TreeState` duplicated core `NavState`. |
| Scrolling | `tui-scrollview` (optional) | |
| Async | not used ([ADR-0011](../decisions/0011-renderer-source.md)): std threads + `mpsc` | Watcher debounce, index rebuild, highlight |
| CLI | `clap` (derive) | |
| Errors | `anyhow` (bin), `thiserror` (libs), `color-eyre` optional | |
| Opener | `open` crate (or shell `open`/`xdg-open`) | External links |
| Clipboard | OSC 52 (hand-rolled, ~20 lines) | Works through herdr/SSH |
| Snapshot tests | `insta` + ratatui `TestBackend` | Render a fixture page to a buffer and snapshot it |

## External tools (runtime, optional)

- `git`: context signals.
- `herdr` CLI: sibling panes, agent states (`herdr pane list --workspace $HERDR_WORKSPACE_ID` returns JSON with `cwd`/`foreground_cwd`).
- `$EDITOR`: editing.

## Licensing

Choose MIT or Apache-2.0 (dual is common in Rust). That keeps you compatible with porting MIT code from markdown-reader. No source has been ported. If code is ever ported, keep its copyright notice in the ported files and add a `THIRD_PARTY.md` in the same change.
