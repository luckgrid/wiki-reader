//! Image slots: a block-level local image reserves display rows that the TUI draws into
//! (ADR-0017). The renderer stays terminal-free; it only decides *where* and *how tall*.

use std::path::{Path, PathBuf};

use wiki_reader_core::images::{ImageReject, MAX_IMAGE_PIXELS, resolve_local_image};

/// Tallest slot, in display rows. Keeps one image from filling several screens.
pub const MAX_SLOT_ROWS: u16 = 30;

/// Rows reserved for an image. The first row is [`ImageSlot::line`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSlot {
    /// 0-based display row of the slot's first row.
    pub line: u32,
    /// Reserved rows.
    pub rows: u16,
    /// Reserved columns (≤ the layout width).
    pub cols: u16,
    /// Canonical file, already validated against ADR-0017.
    pub path: PathBuf,
    /// Alt text, for the placeholder shown while loading or when decoding fails.
    pub alt: String,
    /// File size and modified seconds, so an edited file is not served from a stale cache.
    pub stamp: (u64, u64),
}

/// What a block-level image becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImagePlan {
    Slot {
        path: PathBuf,
        stamp: (u64, u64),
        cols: u16,
        rows: u16,
    },
    Placeholder(ImageReject),
}

/// Columns × rows for an image of `px_w × px_h` pixels: natural size, shrunk to fit
/// `max_cols` and [`MAX_SLOT_ROWS`] while keeping the aspect ratio. Never upscales.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to u16 range
pub fn slot_geometry(px_w: u32, px_h: u32, cell_px: (u16, u16), max_cols: u16) -> (u16, u16) {
    let cell_w = f64::from(cell_px.0.max(1));
    let cell_h = f64::from(cell_px.1.max(1));
    let w = f64::from(px_w.max(1));
    let h = f64::from(px_h.max(1));
    let max_cols = max_cols.max(1);
    let scale = (f64::from(max_cols) * cell_w / w)
        .min(f64::from(MAX_SLOT_ROWS) * cell_h / h)
        .min(1.0);
    let cols = (w * scale / cell_w).ceil().clamp(1.0, f64::from(max_cols));
    let rows = (h * scale / cell_h)
        .ceil()
        .clamp(1.0, f64::from(MAX_SLOT_ROWS));
    (cols as u16, rows as u16)
}

/// Decide how a block-level image `dest` renders from the page at `page_rel`.
///
/// `cell_px` is `Some` only when the terminal has a usable graphics protocol.
pub(crate) fn plan_image(
    root: Option<&Path>,
    page_rel: &Path,
    dest: &str,
    cell_px: Option<(u16, u16)>,
    max_cols: u16,
) -> ImagePlan {
    let Some(root) = root else {
        return ImagePlan::Placeholder(ImageReject::NoRoot);
    };
    let local = match resolve_local_image(root, page_rel, dest) {
        Ok(local) => local,
        Err(reason) => return ImagePlan::Placeholder(reason),
    };
    let Some(cell_px) = cell_px else {
        return ImagePlan::Placeholder(ImageReject::NoGraphics);
    };
    // Header only: the pixel cap is enforced before any decode.
    let Ok((px_w, px_h)) = image::image_dimensions(&local.path) else {
        return ImagePlan::Placeholder(ImageReject::Unreadable);
    };
    if u64::from(px_w) * u64::from(px_h) > MAX_IMAGE_PIXELS {
        return ImagePlan::Placeholder(ImageReject::TooManyPixels);
    }
    let (cols, rows) = slot_geometry(px_w, px_h, cell_px, max_cols);
    ImagePlan::Slot {
        path: local.path,
        stamp: (local.bytes, local.modified_secs),
        cols,
        rows,
    }
}

/// Text shown instead of a picture: `[image: alt] path — reason`.
pub(crate) fn placeholder_text(alt: &str, dest: &str, reason: &ImageReject) -> String {
    let alt = alt.trim();
    let label = if alt.is_empty() {
        "[image]".to_owned()
    } else {
        format!("[image: {alt}]")
    };
    format!("{label} {dest} — {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_image_keeps_its_natural_cell_size() {
        // 80×34 px at 8×17 cells is exactly 10×2.
        assert_eq!(slot_geometry(80, 34, (8, 17), 80), (10, 2));
    }

    #[test]
    fn wide_image_shrinks_to_the_pane_and_keeps_aspect() {
        // 1600×800 px into 80 cols (640 px): scale 0.4 → 640×320 px = 80×19 cells.
        assert_eq!(slot_geometry(1600, 800, (8, 17), 80), (80, 19));
    }

    #[test]
    fn tall_image_is_capped_at_the_row_limit() {
        let (cols, rows) = slot_geometry(400, 4000, (8, 17), 80);
        assert_eq!(rows, MAX_SLOT_ROWS);
        // 30 rows × 17 px = 510 px tall ⇒ scale 0.1275 ⇒ ≈51 px wide ⇒ 7 columns.
        assert_eq!(cols, 7);
    }

    #[test]
    fn never_upscales_and_never_returns_zero() {
        assert_eq!(slot_geometry(1, 1, (8, 17), 80), (1, 1));
        let (cols, rows) = slot_geometry(16, 16, (8, 17), 80);
        assert_eq!((cols, rows), (2, 1));
    }

    #[test]
    fn unknown_cell_size_does_not_divide_by_zero() {
        let (cols, rows) = slot_geometry(100, 100, (0, 0), 20);
        assert!(cols >= 1 && rows >= 1);
    }

    #[test]
    fn placeholder_names_alt_path_and_reason() {
        assert_eq!(
            placeholder_text("Architecture", "img/a.png", &ImageReject::NoGraphics),
            "[image: Architecture] img/a.png — no graphics protocol"
        );
        assert_eq!(
            placeholder_text("  ", "https://x/y.png", &ImageReject::Url),
            "[image] https://x/y.png — URL images are never fetched"
        );
    }

    #[test]
    fn missing_root_is_a_placeholder() {
        assert_eq!(
            plan_image(None, Path::new("p.md"), "a.png", Some((8, 17)), 80),
            ImagePlan::Placeholder(ImageReject::NoRoot)
        );
    }
}
