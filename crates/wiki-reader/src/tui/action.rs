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
    /// Activate search overlay.
    OpenSearch,
    /// Close search overlay without navigating.
    CloseSearch,
    /// Open the help overlay (`?`).
    OpenHelp,
    /// Close the help overlay.
    CloseHelp,
    /// Move help selection.
    HelpSelectDelta(i32),
    /// Page help list (sign = direction).
    HelpPageDelta(i32),
    /// Jump help selection to top.
    HelpHome,
    /// Jump help selection to bottom.
    HelpEnd,
    /// Activate the selected help row (dispatch its action).
    HelpActivate,
    /// Type a character into the search field.
    SearchChar(char),
    /// Backspace in search field.
    SearchBackspace,
    /// Move selection in search results.
    SearchSelectDelta(i32),
    /// Toggle Pages / Text mode.
    SearchToggleMode,
    /// Activate the selected search result.
    SearchActivate,
    /// Activate search result by index (mouse).
    SearchActivateIndex(usize),
    /// Cycle to next in-page search match (`n`).
    SearchNextMatch,
    /// Cycle to previous in-page search match (`N`).
    SearchPrevMatch,
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
    /// Jump to previous heading (`Alt+Shift+↑` or `{`).
    ViewerHeadingUp,
    /// Jump to next heading (`Alt+Shift+↓` or `}`).
    ViewerHeadingDown,
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
    /// Viewer Enter — follow link or footer action.
    ViewerActivate,
    /// Focus viewer footer prev/next (`f`).
    FocusFooter,
    /// Follow a link by id (mouse).
    FollowLinkId(u32),
    /// Confirm opening a pending external URL.
    ConfirmOpen,
    /// Decline external URL prompt.
    ConfirmDecline,
    /// Toggle raw / rendered view (`r`).
    ToggleViewMode,
    /// Open current page in `$VISUAL` / `$EDITOR` at the cursor line (`e`).
    OpenInEditor,
    /// Copy current page relative path (`y`).
    CopyPagePath,
    /// Copy focused link target (`Y`).
    CopyLinkTarget,
    /// Duplicate current page into a new tab (`t`).
    NewTab,
    /// Activate next tab (`gt`).
    NextTab,
    /// Activate previous tab (`gT`).
    PrevTab,
    /// Close the active tab (`x`); refuses the last tab.
    CloseTab,
    /// Activate tab by index (tab bar click).
    SwitchTab(usize),
    /// No-op / ignored.
    #[allow(dead_code)]
    None,
}
