//! Typed errors returned by the core. The UI shows them in the footer.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use std::path::PathBuf;

use thiserror::Error;

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

    /// Placeholder for messages that do not yet have a typed variant.
    #[error("{0}")]
    Message(String),
}
