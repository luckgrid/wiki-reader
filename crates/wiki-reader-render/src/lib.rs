//! Markdown → `RenderedDoc` (lines, link spans, source map).
//!
//! See [rendering](../../wiki/architecture/rendering.md) and
//! [ADR-0002](../../wiki/decisions/0002-build-vs-fork.md).

/// Marker that the render crate is linked; real `RenderedDoc` arrives in Phase 1.
pub const CRATE_NAME: &str = "wiki-reader-render";

#[cfg(test)]
mod tests {
    #[test]
    fn render_crate_name() {
        assert_eq!(super::CRATE_NAME, "wiki-reader-render");
    }
}
