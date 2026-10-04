//! TUI: app state, layout, regions, hit map, keymap, theme.
//!
//! See [UI spec](../../../wiki/product/ui-spec.md).

pub mod action;
pub mod app;
pub mod clipboard;
pub mod code_viewer;
pub mod editor;
pub mod focus;
pub mod help_ui;
pub mod highlight;
pub mod hit;
#[cfg(feature = "media")]
pub mod image_viewer;
#[cfg(feature = "media")]
pub mod images;
#[cfg(not(feature = "media"))]
#[path = "images_lite.rs"]
pub mod images;
pub mod keymap;
pub mod layout;
pub mod modal_viewer;
pub mod opener;
pub mod options_ui;
pub mod page_doc;
pub mod regions;
pub mod rendered_doc;
pub mod search_ui;
pub mod selection;
pub mod table_viewer;
pub mod text_col;
pub mod theme;
pub mod viewer_doc;
