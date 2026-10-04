//! Media helpers (P3-19): the diagram palette and fit maths are always built; image decode and
//! Mermaid/SVG rasterisation sit behind the default `raster` feature (the lite build drops it).
//! This crate must not depend on `wiki-reader-render`.

mod palette;

pub use palette::DiagramPalette;

#[cfg(feature = "raster")]
mod raster;
#[cfg(feature = "raster")]
pub use raster::{
    RasterError, RasterImage, decode_file, image_dimensions, mermaid_svg_bytes, mermaid_to_svg,
    rasterise_svg, rasterise_svg_scaled, render_mermaid, render_mermaid_for_pane, svg_natural_size,
};

/// Minimum scale of natural diagram pixels into the slot before the text tier is preferred.
pub const MIN_LEGIBLE_SCALE: f64 = 0.55;

/// Fit scale for `px_w × px_h` into `max_cols` × `max_rows` at `cell_px`. Never upscales.
#[must_use]
pub fn fit_scale(px_w: u32, px_h: u32, cell_px: (u16, u16), max_cols: u16, max_rows: u16) -> f64 {
    let cell_w = f64::from(cell_px.0.max(1));
    let cell_h = f64::from(cell_px.1.max(1));
    let w = f64::from(px_w.max(1));
    let h = f64::from(px_h.max(1));
    let max_cols = max_cols.max(1);
    (f64::from(max_cols) * cell_w / w)
        .min(f64::from(max_rows.max(1)) * cell_h / h)
        .min(1.0)
}

/// True when fitting `px_w`×`px_h` into `max_cols` at `cell_px` keeps scale ≥ [`MIN_LEGIBLE_SCALE`].
#[must_use]
pub fn is_legible(px_w: u32, px_h: u32, cell_px: (u16, u16), max_cols: u16, max_rows: u16) -> bool {
    fit_scale(px_w, px_h, cell_px, max_cols, max_rows) >= MIN_LEGIBLE_SCALE
}

/// True for a `.svg` path (case-insensitive).
#[must_use]
pub fn is_svg_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legibility_gate_rejects_wide_natural_size() {
        // 2000 px into 80×8 = 640 px pane → scale 0.32 < 0.55.
        assert!(!is_legible(2000, 400, (8, 17), 80, 30));
        // Compact diagram stays legible.
        assert!(is_legible(400, 200, (8, 17), 80, 30));
    }
}
