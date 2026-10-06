---
id: WR-LIBS
title: Prior art & libraries
summary: Existing tools worth learning from and the libraries wiki-reader builds on.
status: active
updated: 2026-10-05
related: [overview, rendering, benchmarks]
nav_order: 4
---

# Prior art & libraries

> Versions move fast. Workspace pins live in the root `Cargo.toml`; check crates.io and each repo's license before adding a dependency.

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

## How heavy is it?

Size, memory and start-up numbers live in [Benchmarks](benchmarks.md).

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

Pinned in the workspace `Cargo.toml` (versions move; check crates.io before bumping).

### Core (`wiki-reader-core`)

| Need | Crate | Notes |
|------|-------|-------|
| Markdown parsing | `pulldown-cmark` | Event walk for links/headings; tables/tasklists enabled. |
| Frontmatter | own splitter + `toml` + `serde_norway` | YAML via `serde_norway` (maintained); TOML via `toml`. |
| Config write | `toml_edit` | Document-preserving single-key patches ([ADR-0018](../decisions/0018-config-write-path.md)). |
| File discovery | `ignore` | `.gitignore`-aware walking. |
| Watching | `notify` + `notify-debouncer-mini` | Markdown-path filtered; rebuild off UI thread. |
| Globs | `globset` | `exclude`, path rules. |
| URLs | `percent-encoding` | Link path decode. |
| Serde | `serde` + `serde_json` | Config and herdr JSON. |
| Errors | `thiserror` | Typed library errors. |
| Fuzzy match | hand-rolled subsequence scorer | Covers short prefixes and multi-word any-order pages. |
| Full-text search | in-memory scan | |
| XDG paths | hand-rolled from env/`HOME` | No `directories` / `dirs` crate. |

### Render (`wiki-reader-render`)

| Need | Crate | Notes |
|------|-------|-------|
| Mermaid → text | `mermaid-text` ≥ 0.56.1 | Unicode box-drawing tier. |
| Width | `unicode-width` | CJK/emoji-safe wrapping (linear wrap, V4). |
| Media | `wiki-reader-media` | Palette always; heavy `raster` behind the feature. |

### Media (`wiki-reader-media`, feature `raster`)

| Need | Crate | Notes |
|------|-------|-------|
| Raster decode | `image` | PNG, JPEG, GIF (first frame), WebP. |
| Mermaid → SVG | `mermaid-rs-renderer` | Pure Rust; off in lite. |
| SVG → RGBA | `resvg` | Embedded Noto Sans; off in lite. |

### App (`wiki-reader`)

| Need | Crate | Notes |
|------|-------|-------|
| TUI | `ratatui` 0.30.x (brings `crossterm`) | Flat side-nav rows ([ADR-0010](../decisions/0010-flat-side-nav-rows.md)); no `tui-tree-widget`. |
| Terminal images | `ratatui-image` | In the **binary** (optional `media` feature); Kitty under herdr, else off/text. |
| Raw highlighting | `syntect` (regex-fancy) | In the **binary**; off-thread worker. |
| CLI | `clap` (derive) | Incl. `--herdr-context` / `--herdr-split`. |
| Command lines | `shell-words` | Config `opener` / `editor` argv split. |
| Signals | `signal-hook` | Clean terminal restore on SIGHUP/etc. |
| Opener | hand-rolled `open` / `xdg-open` | No `open` crate. |
| Clipboard | OSC 52 (hand-rolled) | Works through herdr/SSH. |
| Async | not used ([ADR-0011](../decisions/0011-renderer-source.md)): std threads + `mpsc` | |
| Snapshot tests | `insta` + ratatui `TestBackend` | |

## External tools (runtime, optional)

- `herdr` CLI: plugin actions, sibling panes, agent states (`herdr pane list --workspace $HERDR_WORKSPACE_ID`).
- `$EDITOR` / `$VISUAL`: editing.
- `git`: context signals (Phase 4).

## Licensing

**MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`). Compatible with learning from MIT tools such as markdown-reader; no source has been ported. If code is ever ported, keep its copyright notice in the ported files and add a `THIRD_PARTY.md` in the same change.
