# wiki-reader

A terminal wiki reader for markdown collections. It browses like a documentation site (side nav, breadcrumbs, working links, back/forward, prev/next), sized to live in a multiplexer pane next to your work, and renders images and Mermaid diagrams where the terminal supports it.

![wiki-reader browsing its own wiki: side nav, rendered page, tab, Linked from pane](https://raw.githubusercontent.com/luckgrid/wiki-reader/main/assets/wiki-reader.png)

## Install

```bash
cargo install --locked wiki-reader-tui
```

The package is called `wiki-reader-tui` (the name `wiki-reader` is taken on crates.io); the command it installs is `wiki-reader`. Lite build without the image stack (smaller, faster to compile): `cargo install --locked --no-default-features wiki-reader-tui`. Prebuilt binaries for macOS and Linux are on [GitHub Releases](https://github.com/luckgrid/wiki-reader/releases).

If you installed an earlier version from git under the old package name, run `cargo uninstall wiki-reader` first (or add `--force`).

## Use

```bash
wiki-reader path/to/markdown-folder   # q to quit, ? for help
```

Press `?` in the app for every key. Configuration is read from `~/.config/wiki-reader/config.toml` and an optional `.wiki-reader.toml` in the collection; see the [configuration guide](https://github.com/luckgrid/wiki-reader/blob/main/wiki/guides/configuration.md).

## More

- Documentation, roadmap and decisions: <https://github.com/luckgrid/wiki-reader>
- Changelog: [GitHub Releases](https://github.com/luckgrid/wiki-reader/releases)
- Security: [SECURITY.md](https://github.com/luckgrid/wiki-reader/blob/main/SECURITY.md)
- Licence: MIT OR Apache-2.0 (see `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`)
