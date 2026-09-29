//! Focused pane (nav vs viewer).

/// Which pane is focused (for the status label and keymap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPane {
    /// Side nav.
    Nav,
    /// Viewer.
    #[default]
    Viewer,
}

impl FocusPane {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Nav => "NAV",
            Self::Viewer => "VIEWER",
        }
    }
}
