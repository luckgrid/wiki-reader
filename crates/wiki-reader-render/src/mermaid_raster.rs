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

/// Colours a Mermaid diagram is drawn with, so the picture matches the active theme.
///
/// Plain RGB so the renderer stays terminal-free; the TUI fills it from its theme tokens. It is
/// part of every diagram cache key, so changing preset re-measures and re-rasterises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagramPalette {
    /// Card background, also used to fill the slot's padding so the picture is opaque.
    pub bg: (u8, u8, u8),
    /// Labels and titles.
    pub text: (u8, u8, u8),
    /// Edges and axes.
    pub line: (u8, u8, u8),
    /// Node fill.
    pub node_fill: (u8, u8, u8),
    /// Node outline.
    pub node_border: (u8, u8, u8),
    /// Subgraph / cluster fill.
    pub cluster_fill: (u8, u8, u8),
    /// Subgraph / cluster outline.
    pub cluster_border: (u8, u8, u8),
    /// Sequence / state note fill (its text uses [`text`](Self::text)).
    pub note_fill: (u8, u8, u8),
    /// Sequence / state note outline.
    pub note_border: (u8, u8, u8),
}

impl Default for DiagramPalette {
    /// Dark card, matching the default (dark) theme's code background.
    fn default() -> Self {
        Self {
            bg: (30, 32, 36),
            text: (226, 229, 236),
            line: (140, 148, 164),
            node_fill: (44, 48, 58),
            node_border: (110, 120, 140),
            cluster_fill: (38, 41, 50),
            cluster_border: (80, 88, 104),
            note_fill: (58, 54, 40),
            note_border: (150, 130, 80),
        }
    }
}

/// Mid-tone categorical slices that keep light labels readable on a dark card. Mermaid's own
/// dark pie colours (`#0b0000`, `#010029`, …) vanish into a dark background.
const DARK_PIE: [&str; 12] = [
    "#3B6EA8", "#B8683A", "#4C9A6A", "#9A4C86", "#A8903A", "#4A9AA8", "#A84C54", "#6A6AB8",
    "#7A9A3A", "#B8587A", "#5A7A8A", "#8A6A4A",
];

impl DiagramPalette {
    /// CSS hex colour (`#rrggbb`) for the SVG theme.
    fn hex((r, g, b): (u8, u8, u8)) -> String {
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// Dark when the card is darker than mid-grey; picks the base theme the palette overlays.
    fn is_dark(&self) -> bool {
        let (r, g, b) = self.bg;
        // Rec. 601 luma is enough to tell a dark card from a light one.
        (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000 < 128
    }

    /// Overlay the palette on `theme`, which is `Theme::modern()` for light cards and
    /// `Theme::dark()` for dark ones, so the fields the palette does not name (git graph, pie
    /// strokes, commit labels) are at least the right way round.
    fn apply(&self, theme: &mut mermaid_rs_renderer::Theme) {
        let text = Self::hex(self.text);
        let line = Self::hex(self.line);
        let node_fill = Self::hex(self.node_fill);
        let node_border = Self::hex(self.node_border);
        theme.background = Self::hex(self.bg);
        theme.edge_label_background = Self::hex(self.bg);
        theme.text_color.clone_from(&text);
        theme.primary_text_color.clone_from(&text);
        theme.pie_title_text_color.clone_from(&text);
        theme.pie_section_text_color.clone_from(&text);
        theme.pie_legend_text_color.clone_from(&text);
        theme.line_color.clone_from(&line);
        theme.sequence_actor_line.clone_from(&line);
        theme.primary_color.clone_from(&node_fill);
        theme.secondary_color.clone_from(&node_fill);
        theme.tertiary_color.clone_from(&node_fill);
        theme.sequence_actor_fill.clone_from(&node_fill);
        theme.sequence_activation_fill.clone_from(&node_fill);
        theme.primary_border_color.clone_from(&node_border);
        theme.sequence_actor_border.clone_from(&node_border);
        theme.sequence_activation_border.clone_from(&node_border);
        theme.cluster_background = Self::hex(self.cluster_fill);
        theme.cluster_border = Self::hex(self.cluster_border);
        theme.sequence_note_fill = Self::hex(self.note_fill);
        theme.sequence_note_border = Self::hex(self.note_border);
        theme.pie_outer_stroke_color.clone_from(&node_border);
        if self.is_dark() {
            theme.pie_colors = DARK_PIE.map(str::to_owned);
            theme.pie_stroke_color = Self::hex(self.bg);
        }
    }
}

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
    rasterise_svg_scaled(svg, 1.0)
}

/// [`rasterise_svg`] at `scale` times the natural size (sharp zoom, P3-15). The ADR-0017 pixel
/// cap applies to the *output*, so no zoom level can exceed it.
///
/// # Errors
///
/// Returns [`RasterError::Raster`] or [`RasterError::TooManyPixels`].
pub fn rasterise_svg_scaled(svg: &[u8], scale: f32) -> Result<RasterImage, RasterError> {
    let options = usvg_options();
    let tree =
        usvg::Tree::from_data(svg, &options).map_err(|e| RasterError::Raster(e.to_string()))?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or_else(|| RasterError::Raster("diagram dimensions cannot be rasterised".into()))?;
    let px_w = size.width();
    let px_h = size.height();
    if u64::from(px_w) * u64::from(px_h) > wiki_reader_core::images::MAX_IMAGE_PIXELS {
        return Err(RasterError::TooManyPixels);
    }
    let mut pixmap = tiny_skia::Pixmap::new(px_w, px_h)
        .ok_or_else(|| RasterError::Raster("diagram dimensions cannot be rasterised".into()))?;
    // The SVG size is fractional (268.77 px); stretch it onto the whole-pixel canvas so the last
    // column and row are fully covered instead of half-transparent.
    #[allow(clippy::cast_precision_loss)] // pixel sizes are far below f32's exact-integer range
    let transform = tiny_skia::Transform::from_scale(
        px_w as f32 / tree.size().width(),
        px_h as f32 / tree.size().height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
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
pub fn mermaid_to_svg(src: &str, palette: &DiagramPalette) -> Result<String, RasterError> {
    let mut options = RenderOptions::modern();
    if palette.is_dark() {
        // Same metrics as `modern()`: only colours change with the palette, never the layout.
        let font_size = options.theme.font_size;
        options.theme = mermaid_rs_renderer::Theme::dark();
        options.theme.font_size = font_size;
    }
    options.theme.font_family = LAYOUT_FONT_FAMILY.to_string();
    palette.apply(&mut options.theme);
    render_with_options(src, options).map_err(|e| RasterError::Parse(e.to_string()))
}

/// Mermaid source → SVG bytes with the private layout family rewritten to the embedded font's
/// real name, ready for [`rasterise_svg`] / [`rasterise_svg_scaled`].
///
/// # Errors
///
/// Returns [`RasterError::Parse`] when the renderer rejects the source.
pub fn mermaid_svg_bytes(src: &str, palette: &DiagramPalette) -> Result<Vec<u8>, RasterError> {
    let svg = mermaid_to_svg(src, palette)?;
    Ok(svg
        .replace(LAYOUT_FONT_FAMILY, RASTER_FONT_FAMILY)
        .into_bytes())
}

/// Rewrite the private layout family to the embedded font's real name, then rasterise.
///
/// # Errors
///
/// Propagates parse/raster errors from [`mermaid_to_svg`] / [`rasterise_svg`].
pub fn render_mermaid(src: &str, palette: &DiagramPalette) -> Result<RasterImage, RasterError> {
    rasterise_svg(&mermaid_svg_bytes(src, palette)?)
}

/// True when fitting `px_w`×`px_h` into `max_cols` at `cell_px` keeps scale ≥ [`MIN_LEGIBLE_SCALE`].
#[must_use]
pub fn is_legible(px_w: u32, px_h: u32, cell_px: (u16, u16), max_cols: u16, max_rows: u16) -> bool {
    fit_scale(px_w, px_h, cell_px, max_cols, max_rows) >= MIN_LEGIBLE_SCALE
}

/// Rasterise Mermaid and reject results that would be illegible in the pane.
///
/// # Errors
///
/// Returns [`RasterError::TooWide`] when the effective scale is below the gate, or other raster errors.
pub fn render_mermaid_for_pane(
    src: &str,
    palette: &DiagramPalette,
    cell_px: (u16, u16),
    max_cols: u16,
    max_rows: u16,
) -> Result<RasterImage, RasterError> {
    let rendered = render_mermaid(src, palette)?;
    if !is_legible(rendered.px_w, rendered.px_h, cell_px, max_cols, max_rows) {
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
        let a = render_mermaid(src, &DiagramPalette::default()).expect("first");
        let b = render_mermaid(src, &DiagramPalette::default()).expect("second");
        assert_eq!(a.px_w, b.px_w);
        assert_eq!(a.px_h, b.px_h);
        assert_eq!(a.image.as_raw(), b.image.as_raw());
    }

    #[test]
    fn legibility_gate_rejects_wide_natural_size() {
        // 2000 px into 80×8 = 640 px pane → scale 0.32 < 0.55.
        assert!(!is_legible(2000, 400, (8, 17), 80, 30));
        // Compact diagram stays legible.
        assert!(is_legible(400, 200, (8, 17), 80, 30));
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
        let err =
            render_mermaid("not a diagram {{{", &DiagramPalette::default()).expect_err("parse");
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
        let err =
            render_mermaid_for_pane(fence_body(md), &DiagramPalette::default(), (8, 17), 80, 30)
                .expect_err("wide");
        assert!(matches!(err, RasterError::TooWide), "{err:?}");
    }

    #[test]
    fn malformed_fixture_is_a_parse_error() {
        let md = include_str!("../../../fixtures/mermaid/malformed.md");
        let err = render_mermaid(fence_body(md), &DiagramPalette::default()).expect_err("parse");
        assert!(matches!(err, RasterError::Parse(_)), "{err:?}");
    }

    fn light() -> DiagramPalette {
        DiagramPalette {
            bg: (250, 250, 252),
            text: (20, 24, 32),
            line: (90, 98, 112),
            node_fill: (232, 236, 244),
            node_border: (120, 130, 150),
            cluster_fill: (240, 242, 248),
            cluster_border: (190, 196, 210),
            note_fill: (255, 247, 237),
            note_border: (253, 186, 116),
        }
    }

    #[test]
    fn svg_carries_the_palette_colours() {
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let dark = mermaid_to_svg(src, &DiagramPalette::default()).expect("dark");
        let light = mermaid_to_svg(src, &light()).expect("light");
        assert!(dark.to_uppercase().contains("#1E2024"), "dark card bg");
        assert!(light.to_uppercase().contains("#FAFAFC"), "light card bg");
        assert!(
            !light.to_uppercase().contains("#FFFFFF"),
            "no stray light-theme white"
        );
        assert_ne!(dark, light);
    }

    #[test]
    fn rendered_card_is_opaque_and_uses_the_background() {
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let img = render_mermaid(src, &DiagramPalette::default()).expect("render");
        assert!(
            img.image.pixels().all(|p| p.0[3] == 255),
            "no transparent pixels"
        );
        // The corner is background, not a node.
        assert_eq!(img.image.get_pixel(0, 0).0[..3], [30, 32, 36]);
    }

    #[test]
    fn palettes_differ_as_cache_keys() {
        assert_ne!(DiagramPalette::default(), light());
    }

    /// The body of every fenced mermaid block in `md`.
    fn fences(md: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = md;
        while let Some(at) = rest.find("```mermaid") {
            let body = rest[at + "```mermaid".len()..].trim_start_matches(['\r', '\n']);
            let end = body.find("```").expect("fence close");
            out.push(body[..end].trim_end_matches(['\r', '\n']));
            rest = &body[end + 3..];
        }
        out
    }

    fn herdr() -> DiagramPalette {
        DiagramPalette {
            bg: (24, 24, 24),
            text: (240, 240, 240),
            line: (160, 160, 160),
            node_fill: (38, 38, 38),
            node_border: (255, 199, 153),
            cluster_fill: (30, 30, 30),
            cluster_border: (80, 80, 80),
            note_fill: (52, 46, 40),
            note_border: (255, 199, 153),
        }
    }

    const SEQUENCE_NOTE: &str = "sequenceDiagram\n  participant A\n  participant B\n  A->>B: hi\n  Note over A,B: remember\n";

    #[test]
    fn note_fill_follows_the_palette_not_the_light_default() {
        let dark = mermaid_to_svg(SEQUENCE_NOTE, &DiagramPalette::default())
            .expect("dark")
            .to_uppercase();
        assert!(dark.contains("#3A3628"), "dark note fill");
        assert!(
            !dark.contains("#FFF7ED"),
            "no light note fill on a dark card"
        );
        let light_svg = mermaid_to_svg(SEQUENCE_NOTE, &light())
            .expect("light")
            .to_uppercase();
        assert!(light_svg.contains("#FFF7ED"), "light note fill");
    }

    #[test]
    fn dark_pie_and_git_graph_do_not_keep_light_defaults() {
        let pie = "pie title Status\n  \"a\" : 3\n  \"b\" : 2\n";
        let svg = mermaid_to_svg(pie, &DiagramPalette::default())
            .expect("pie")
            .to_uppercase();
        assert!(svg.contains("#3B6EA8"), "mid-tone slice colour");
        assert!(!svg.contains("#0B0000"), "mermaid's near-black dark slice");
        let git = "gitGraph\n  commit\n  branch x\n  commit\n";
        assert!(mermaid_to_svg(git, &DiagramPalette::default()).is_ok());
    }

    #[test]
    fn every_fixture_diagram_renders_under_every_palette() {
        let common = include_str!("../../../fixtures/mermaid/common-types.md");
        let themed = include_str!("../../../fixtures/mermaid/themed.md");
        let all: Vec<&str> = fences(common).into_iter().chain(fences(themed)).collect();
        assert!(all.len() >= 9, "fixtures changed shape: {}", all.len());
        for palette in [DiagramPalette::default(), light(), herdr()] {
            for src in &all {
                let img = render_mermaid(src, &palette).unwrap_or_else(|e| panic!("{e}: {src}"));
                assert!(img.px_w > 0 && img.px_h > 0);
            }
        }
    }

    #[test]
    fn layout_does_not_depend_on_the_palette() {
        // Colours change with the preset; sizes must not, or a theme switch would reflow pages.
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let size = |p: &DiagramPalette| {
            let svg = mermaid_to_svg(src, p).expect("svg");
            svg_natural_size(svg.as_bytes()).expect("size")
        };
        assert_eq!(size(&DiagramPalette::default()), size(&light()));
        assert_eq!(size(&DiagramPalette::default()), size(&herdr()));
    }
}
