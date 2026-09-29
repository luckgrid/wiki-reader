//! Terminal-free index, navigation, and content core for wiki-reader.
//!
//! See [architecture overview](../../wiki/architecture/overview.md) and
//! [ADR-0006](../../wiki/decisions/0006-reader-first.md).

pub mod config;
pub mod error;
pub mod index;
pub mod nav;
pub mod parse;
pub mod provider;
pub mod watch;

pub use error::Error;

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn error_display_works() {
        let err = Error::Message("smoke".into());
        assert_eq!(err.to_string(), "smoke");
    }
}
