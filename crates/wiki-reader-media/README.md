# wiki-reader-media

Image decoding and Mermaid/SVG rasterisation for wiki-reader. The heavy half sits behind the default `raster` feature; without it only the diagram palette and fit maths are built (the lite build).

Part of [wiki-reader](https://github.com/luckgrid/wiki-reader), a terminal wiki reader for markdown collections. This crate is the media layer of the application, published so that the `wiki-reader-tui` binary can be installed from crates.io. **It is an internal crate: its API is not stable and may change in any release.** If you want the reader, install the binary instead:

```bash
cargo install --locked wiki-reader-tui
```

## Links

- Repository and documentation: <https://github.com/luckgrid/wiki-reader>
- Licence: MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`)
