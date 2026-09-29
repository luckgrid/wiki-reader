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
    /// Side-nav: step to previous visible row (↑ / Shift+Tab).
    NavStepUp,
    /// Side-nav: step to next visible row (↓ / Tab).
    NavStepDown,
    /// Side-nav: jump to previous group header or search (Shift+↑).
    NavJumpUp,
    /// Side-nav: jump to next group header (Shift+↓).
    NavJumpDown,
    /// Side-nav: expand group or enter (→).
    NavExpand,
    /// Side-nav: collapse group or go to parent (←).
    NavCollapse,
    /// Side-nav: Enter — open page or toggle group.
    NavActivate,
    /// Viewer cursor / scroll (filled in P1-08b).
    ViewerUp,
    /// Viewer down.
    ViewerDown,
    /// Viewer block jump up.
    ViewerBlockUp,
    /// Viewer block jump down.
    ViewerBlockDown,
    /// Viewer page up.
    ViewerPageUp,
    /// Viewer page down.
    ViewerPageDown,
    /// Viewer home.
    ViewerHome,
    /// Viewer end.
    ViewerEnd,
    /// Viewer Tab cycle forward (P1-08c).
    ViewerTab,
    /// Viewer Tab cycle backward.
    ViewerBackTab,
    /// No-op / ignored.
    #[allow(dead_code)]
    None,
}
