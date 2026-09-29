//! Typed errors returned by the core. The UI shows them in the footer.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use std::path::PathBuf;

use thiserror::Error;

use crate::provider::PageKey;

/// Core library error.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Walk/filter failure from the `ignore` crate.
    #[error(transparent)]
    Walk(#[from] ignore::Error),

    /// Collection root is missing or not a directory.
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Requested path escapes the collection root.
    #[error("path outside collection: {0}")]
    PathOutsideRoot(PathBuf),

    /// Collection has no markdown pages.
    #[error("collection has no pages")]
    EmptyCollection,

    /// Requested start page is not in the index.
    #[error("page not found: {}", .0.relative_path.display())]
    PageNotFound(PageKey),
}
