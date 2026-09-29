//! Link resolution, `NavTree` build (titles, folding, order), prev/next.
//!
//! See [content model](../../../../wiki/product/content-model.md) and
//! [ADR-0008](../../../../wiki/decisions/0008-side-nav-as-site-nav.md).

mod resolve;
mod tree;

pub use resolve::{ResolveOutcome, Target, resolve};
pub use tree::{NavItem, NavTree, NodeId, humanize_filename, page_label};
