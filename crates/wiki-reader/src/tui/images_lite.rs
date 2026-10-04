//! Lite build (ADR-0023): the `ImageManager` surface the app calls, with no graphics, decode
//! worker or probe. Every image stays a text placeholder; diagrams stay on the text tier.
#![allow(clippy::unused_self)] // same signatures as the real manager
// ponytail: stub kept method-for-method with `images.rs`; a trait would add a seam for no gain.

use std::sync::Arc;

use wiki_reader_render::{DiagramRequest, DiagramSizeCache, ImageSlot};

pub struct ImageManager {
    sizes: Arc<DiagramSizeCache>,
}

impl ImageManager {
    pub fn disabled() -> Self {
        Self {
            sizes: Arc::new(DiagramSizeCache::new()),
        }
    }

    pub fn cell_px(&self) -> Option<(u16, u16)> {
        None
    }

    pub fn diagram_sizes(&self) -> Arc<DiagramSizeCache> {
        Arc::clone(&self.sizes)
    }

    pub fn queue_diagram_requests(&mut self, _: &[DiagramRequest]) {}

    pub fn retain_for(&mut self, _: &[ImageSlot]) {}

    pub fn poll(&mut self) {}

    pub fn take_diagram_relayout(&mut self) -> bool {
        false
    }

    pub fn has_pending(&self) -> bool {
        false
    }
}
