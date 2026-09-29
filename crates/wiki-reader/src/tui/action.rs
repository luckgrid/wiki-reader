//! Actions produced by keys or mouse hits.

use wiki_reader_core::nav::NodeId;
use wiki_reader_core::provider::PageKey;

/// One discrete UI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Quit the app.
    Quit,
    /// Toggle side nav visibility / overlay.
    ToggleNav,
    /// Navigate to previous page in tree order.
    PrevPage,
    /// Navigate to next page in tree order.
    NextPage,
    /// Browser back.
    Back,
    /// Browser forward.
    Forward,
    /// Open a page (breadcrumb / nav item).
    GoToPage(PageKey),
    /// Toggle a nav group.
    ToggleGroup(NodeId),
    /// Activate the search row (stub until P1-10).
    OpenSearch,
    /// Focus the side nav pane.
    FocusNav,
    /// Focus the viewer pane.
    FocusViewer,
    /// Cycle pane focus.
    CycleFocus,
    /// Set viewer cursor to a source line.
    SetCursorLine(u32),
    /// No-op / ignored.
    None,
}
