//! Mermaid → SVG → RGBA (ADR-0004 image tier). Deterministic embedded font; no external refs.

use std::fmt;

use image::RgbaImage;
use mermaid_rs_renderer::{RenderOptions, render_with_options};
use resvg::{tiny_skia, usvg};

use crate::images::fit_scale;

const FONT: &[u8] = include_bytes!("../fonts/NotoSans.ttf");
/// Private family so mermaid-rs-renderer uses fallback metrics instead of a system font.
const LAYOUT_FONT_FAMILY: &str = "WikiReaderEmbeddedNotoSans";
const RASTER_FONT_FAMILY: &str = "Noto Sans";
/// Minimum scale of natural diagram pixels into the slot before the text tier is preferred.
pub const MIN_LEGIBLE_SCALE: f64 = 0.55;

/// Why a Mermaid/SVG raster failed or was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterError {
    /// `mermaid-rs-renderer` could not layout the source.
    Parse(String),
    /// `resvg` / usvg failed.
    Raster(String),
    /// Natural size scaled into the pane is below [`MIN_LEGIBLE_SCALE`].
    TooWide,
    /// Decoded pixel count exceeds the ADR-0017 cap.
    TooManyPixels,
}

impl fmt::Display for RasterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) | Self::Raster(msg) => f.write_str(msg),
            Self::TooWide => f.write_str("diagram too wide for pane"),
            Self::TooManyPixels => f.write_str("image too large"),
        }
    }
}

impl std::error::Error for RasterError {}

/// Successful Mermaid raster.
#[derive(Debug, Clone)]
pub struct RasterImage {
    pub image: RgbaImage,
    pub px_w: u32,
    pub px_h: u32,
}

/// Usvg options shared by Mermaid and local SVG: embedded font only, no external image refs.
fn usvg_options() -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_font_data(FONT.to_vec());
    options.font_family = RASTER_FONT_FAMILY.to_string();
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: Box::new(|_, _, _| None),
        resolve_string: Box::new(|_, _| None),
    };
    options
}

/// Rasterise SVG bytes with the embedded font and disabled external refs.
///
/// # Errors
///
/// Returns [`RasterError::Raster`] or [`RasterError::TooManyPixels`].
pub fn rasterise_svg(svg: &[u8]) -> Result<RasterImage, RasterError> {
    let options = usvg_options();
    let tree =
        usvg::Tree::from_data(svg, &options).map_err(|e| RasterError::Raster(e.to_string()))?;
    let size = tree.size().to_int_size();
    let px_w = size.width();
    let px_h = size.height();
    if u64::from(px_w) * u64::from(px_h) > wiki_reader_core::images::MAX_IMAGE_PIXELS {
        return Err(RasterError::TooManyPixels);
    }
    let mut pixmap = tiny_skia::Pixmap::new(px_w, px_h)
        .ok_or_else(|| RasterError::Raster("diagram dimensions cannot be rasterised".into()))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let image = RgbaImage::from_raw(px_w, px_h, pixmap.take())
        .ok_or_else(|| RasterError::Raster("resvg returned an invalid RGBA buffer".into()))?;
    Ok(RasterImage { image, px_w, px_h })
}

/// Natural pixel size of an SVG without building the full RGBA buffer when possible.
///
/// # Errors
///
/// Returns [`RasterError::Raster`] or [`RasterError::TooManyPixels`].
pub fn svg_natural_size(svg: &[u8]) -> Result<(u32, u32), RasterError> {
    let options = usvg_options();
    let tree =
        usvg::Tree::from_data(svg, &options).map_err(|e| RasterError::Raster(e.to_string()))?;
    let size = tree.size().to_int_size();
    let px_w = size.width();
    let px_h = size.height();
    if u64::from(px_w) * u64::from(px_h) > wiki_reader_core::images::MAX_IMAGE_PIXELS {
        return Err(RasterError::TooManyPixels);
    }
    Ok((px_w, px_h))
}

/// Layout Mermaid source to SVG with the private font family (deterministic metrics).
///
/// # Errors
///
/// Returns [`RasterError::Parse`] when the renderer rejects the source.
pub fn mermaid_to_svg(src: &str) -> Result<String, RasterError> {
    let mut options = RenderOptions::modern();
    options.theme.font_family = LAYOUT_FONT_FAMILY.to_string();
    render_with_options(src, options).map_err(|e| RasterError::Parse(e.to_string()))
}

/// Rewrite the private layout family to the embedded font's real name, then rasterise.
///
/// # Errors
///
/// Propagates parse/raster errors from [`mermaid_to_svg`] / [`rasterise_svg`].
pub fn render_mermaid(src: &str) -> Result<RasterImage, RasterError> {
    let svg = mermaid_to_svg(src)?;
    let raster_svg = svg.replace(LAYOUT_FONT_FAMILY, RASTER_FONT_FAMILY);
    rasterise_svg(raster_svg.as_bytes())
}

/// True when fitting `px_w`×`px_h` into `max_cols` at `cell_px` keeps scale ≥ [`MIN_LEGIBLE_SCALE`].
#[must_use]
pub fn is_legible(px_w: u32, px_h: u32, cell_px: (u16, u16), max_cols: u16) -> bool {
    fit_scale(px_w, px_h, cell_px, max_cols) >= MIN_LEGIBLE_SCALE
}

/// Rasterise Mermaid and reject results that would be illegible in the pane.
///
/// # Errors
///
/// Returns [`RasterError::TooWide`] when the effective scale is below the gate, or other raster errors.
pub fn render_mermaid_for_pane(
    src: &str,
    cell_px: (u16, u16),
    max_cols: u16,
) -> Result<RasterImage, RasterError> {
    let rendered = render_mermaid(src)?;
    if !is_legible(rendered.px_w, rendered.px_h, cell_px, max_cols) {
        return Err(RasterError::TooWide);
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_bytes_across_two_renders() {
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let a = render_mermaid(src).expect("first");
        let b = render_mermaid(src).expect("second");
        assert_eq!(a.px_w, b.px_w);
        assert_eq!(a.px_h, b.px_h);
        assert_eq!(a.image.as_raw(), b.image.as_raw());
    }

    #[test]
    fn legibility_gate_rejects_wide_natural_size() {
        // 2000 px into 80×8 = 640 px pane → scale 0.32 < 0.55.
        assert!(!is_legible(2000, 400, (8, 17), 80));
        // Compact diagram stays legible.
        assert!(is_legible(400, 200, (8, 17), 80));
    }

    #[test]
    fn svg_external_href_is_ignored() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20">
            <image href="https://example.com/x.png" width="40" height="20"/>
            <rect width="40" height="20" fill="#336699"/>
        </svg>"##;
        let img = rasterise_svg(svg.as_bytes()).expect("external href must not fail the parse");
        assert_eq!((img.px_w, img.px_h), (40, 20));
        // The rect fills the canvas; an unresolved image must not poison the render.
        let px = img.image.get_pixel(20, 10);
        assert_eq!(px.0[0], 0x33);
        assert_eq!(px.0[1], 0x66);
        assert_eq!(px.0[2], 0x99);
    }

    #[test]
    fn malformed_mermaid_is_a_parse_error() {
        let err = render_mermaid("not a diagram {{{").expect_err("parse");
        assert!(matches!(err, RasterError::Parse(_)), "{err:?}");
    }

    fn fence_body(md: &str) -> &str {
        let start = md.find("```mermaid").expect("fence open") + "```mermaid".len();
        let body = md[start..].trim_start_matches(['\r', '\n']);
        let end = body.find("```").expect("fence close");
        body[..end].trim_end_matches(['\r', '\n'])
    }

    #[test]
    fn wide_graph_fixture_falls_back_for_pane() {
        let md = include_str!("../../../fixtures/mermaid/wide-graph.md");
        let err = render_mermaid_for_pane(fence_body(md), (8, 17), 80).expect_err("wide");
        assert!(matches!(err, RasterError::TooWide), "{err:?}");
    }

    #[test]
    fn malformed_fixture_is_a_parse_error() {
        let md = include_str!("../../../fixtures/mermaid/malformed.md");
        let err = render_mermaid(fence_body(md)).expect_err("parse");
        assert!(matches!(err, RasterError::Parse(_)), "{err:?}");
    }
}
