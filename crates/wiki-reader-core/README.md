# wiki-reader-core

The terminal-free core of wiki-reader: collection discovery, markdown parsing and frontmatter, the page index and link graph, navigation, search, configuration and session state.

Part of [wiki-reader](https://github.com/luckgrid/wiki-reader), a terminal wiki reader for markdown collections. This crate is the terminal-free core of the application, published so that the `wiki-reader-tui` binary can be installed from crates.io. **It is an internal crate: its API is not stable and may change in any release.** If you want the reader, install the binary instead:

```bash
cargo install --locked wiki-reader-tui
```

## Links

- Repository and documentation: <https://github.com/luckgrid/wiki-reader>
- Licence: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`)
