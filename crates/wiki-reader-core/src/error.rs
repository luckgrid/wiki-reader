//! Typed errors returned by the core. The UI shows them in the footer.
//!
//! See [architecture overview](../../../wiki/architecture/overview.md).

use thiserror::Error;

/// Core library error.
#[derive(Debug, Error)]
pub enum Error {
    /// Placeholder until Phase 1 wires real failure modes.
    #[error("{0}")]
    Message(String),
}
