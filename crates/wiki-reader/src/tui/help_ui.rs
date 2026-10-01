//! Help overlay state (P2-20).

use crate::tui::action::Action;
use crate::tui::keymap::{BINDINGS, BindingScope, effective_keys_label};

/// One row in the help list.
#[derive(Debug, Clone)]
pub struct HelpRow {
    /// Section header label (shown when scope changes).
    pub section: Option<&'static str>,
    /// Key chord label (effective after overrides).
    pub keys: String,
    /// Description.
    pub help: &'static str,
    /// Action to dispatch on click / Enter; `None` for display-only.
    pub action: Option<Action>,
}

/// Open help overlay.
#[derive(Debug, Clone)]
pub struct HelpOverlay {
    pub rows: Vec<HelpRow>,
    pub selected: usize,
    pub scroll: usize,
    /// Visible list rows from last draw (for PgUp/PgDn).
    pub list_height: usize,
}

impl HelpOverlay {
    /// Build rows from [`BINDINGS`], applying key overrides to labels.
    #[must_use]
    pub fn new(overrides: &std::collections::BTreeMap<String, String>) -> Self {
        let mut rows = Vec::new();
        let mut last_scope: Option<BindingScope> = None;
        for b in BINDINGS {
            let section = if last_scope == Some(b.scope) {
                None
            } else {
                last_scope = Some(b.scope);
                Some(b.scope.title())
            };
            rows.push(HelpRow {
                section,
                keys: effective_keys_label(b, overrides),
                help: b.help,
                action: b.action.clone(),
            });
        }
        Self {
            rows,
            selected: 0,
            scroll: 0,
            list_height: 10,
        }
    }

    pub fn select_delta(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let n = i32::try_from(self.rows.len()).unwrap_or(1);
        let cur = i32::try_from(self.selected).unwrap_or(0);
        let next = (cur + delta).rem_euclid(n);
        self.selected = usize::try_from(next).unwrap_or(0);
    }
}
