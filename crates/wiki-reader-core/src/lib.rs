//! Terminal-free index, navigation, and content core for wiki-reader.
//!
//! Internal crate of [wiki-reader](https://github.com/luckgrid/wiki-reader); its API is not
//! stable. See the [architecture overview](https://github.com/luckgrid/wiki-reader/blob/main/wiki/architecture/overview.md)
//! and [ADR-0006](https://github.com/luckgrid/wiki-reader/blob/main/wiki/decisions/0006-reader-first.md).

pub mod config;
pub mod error;
pub mod herdr;
pub mod images;
pub mod index;
pub mod nav;
pub mod parse;
pub mod provider;
pub mod search;
pub mod session;
pub mod status;
pub mod watch;

pub use error::Error;
pub use index::Index;
pub use status::DocStatus;

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn error_display_works() {
        let err = Error::EmptyCollection;
        assert_eq!(err.to_string(), "collection has no pages");
    }
}
