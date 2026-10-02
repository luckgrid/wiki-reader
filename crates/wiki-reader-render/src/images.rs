//! Image slots: a block-level local image reserves display rows that the TUI draws into
//! (ADR-0017). The renderer stays terminal-free; it only decides *where* and *how tall*.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wiki_reader_core::images::{ImageReject, MAX_IMAGE_PIXELS, resolve_local_image};

use crate::mermaid_raster::{self, RasterError};

/// Tallest slot, in display rows. Keeps one image from filling several screens.
pub const MAX_SLOT_ROWS: u16 = 30;

/// Where the picture bytes come from for an [`ImageSlot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotSource {
    /// Canonical local file already validated against ADR-0017.
    File {
        path: PathBuf,
        /// File size and modified nanoseconds (freshness stamp).
        stamp: (u64, u64),
    },
    /// In-process Mermaid raster (content hash + background colour for the cache key).
    Mermaid {
        hash: u64,
        bg: (u8, u8, u8),
        /// Fence body; used by the decode worker to (re)rasterise.
        source: String,
    },
}

/// Rows reserved for an image. The first row is [`ImageSlot::line`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSlot {
    /// 0-based display row of the slot's first row.
    pub line: u32,
    /// Reserved rows.
    pub rows: u16,
    /// Reserved columns (≤ the layout width).
    pub cols: u16,
    /// File or Mermaid source for this slot.
    pub source: SlotSource,
    /// Alt text, for the placeholder shown while loading or when decoding fails.
    pub alt: String,
}

/// Natural size known after an off-thread Mermaid (or SVG) measure, or a forced text tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramSize {
    /// Natural SVG/raster pixel size (legible for the requested width).
    Natural { px_w: u32, px_h: u32 },
    /// Prefer the text (or source) tier for this content/width/background.
    Text,
}

/// Shared `(content_hash, width_cols, bg) → size` map filled by the image worker.
type DiagramSizeKey = (u64, u16, (u8, u8, u8));

#[derive(Debug, Default)]
pub struct DiagramSizeCache {
    inner: Mutex<std::collections::HashMap<DiagramSizeKey, DiagramSize>>,
}

impl DiagramSizeCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, hash: u64, width: u16, bg: (u8, u8, u8)) -> Option<DiagramSize> {
        self.inner
            .lock()
            .ok()?
            .get(&(hash, width, bg))
            .copied()
    }

    pub fn insert(&self, hash: u64, width: u16, bg: (u8, u8, u8), size: DiagramSize) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert((hash, width, bg), size);
        }
    }
}

/// Mermaid fence the renderer wants sized off-thread (cache miss while image tier is selected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramRequest {
    pub hash: u64,
    pub source: String,
    pub width: u16,
    pub bg: (u8, u8, u8),
    pub cell_px: (u16, u16),
}

/// What a block-level image becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImagePlan {
    Slot {
        source: SlotSource,
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

fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
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
    let (px_w, px_h) = if is_svg(&local.path) {
        let Ok(bytes) = std::fs::read(&local.path) else {
            return ImagePlan::Placeholder(ImageReject::Unreadable);
        };
        match mermaid_raster::svg_natural_size(&bytes) {
            Ok(size) => size,
            Err(RasterError::TooManyPixels) => {
                return ImagePlan::Placeholder(ImageReject::TooManyPixels);
            }
            Err(_) => return ImagePlan::Placeholder(ImageReject::Unreadable),
        }
    } else {
        // Header only: the pixel cap is enforced before any decode.
        let Ok((px_w, px_h)) = image::image_dimensions(&local.path) else {
            return ImagePlan::Placeholder(ImageReject::Unreadable);
        };
        if u64::from(px_w) * u64::from(px_h) > MAX_IMAGE_PIXELS {
            return ImagePlan::Placeholder(ImageReject::TooManyPixels);
        }
        (px_w, px_h)
    };
    let (cols, rows) = slot_geometry(px_w, px_h, cell_px, max_cols);
    ImagePlan::Slot {
        source: SlotSource::File {
            path: local.path,
            stamp: (local.bytes, local.modified_nanos),
        },
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

/// Shared empty cache for unit tests that do not care about Mermaid sizing.
#[must_use]
pub fn empty_diagram_size_cache() -> Arc<DiagramSizeCache> {
    Arc::new(DiagramSizeCache::new())
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

    #[test]
    fn size_cache_key_includes_bg_and_width() {
        let cache = DiagramSizeCache::new();
        cache.insert(1, 80, (30, 32, 36), DiagramSize::Natural { px_w: 100, px_h: 50 });
        assert_eq!(
            cache.get(1, 80, (30, 32, 36)),
            Some(DiagramSize::Natural { px_w: 100, px_h: 50 })
        );
        assert_eq!(cache.get(1, 60, (30, 32, 36)), None, "width is part of the key");
        assert_eq!(cache.get(1, 80, (0, 0, 0)), None, "bg is part of the key");
    }
}
