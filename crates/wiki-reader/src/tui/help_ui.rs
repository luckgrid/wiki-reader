//! Help overlay state (P2-20).

use crate::tui::action::Action;
use crate::tui::keymap::{BINDINGS, BindingScope, binding_icon, effective_keys_label};

/// One row in the help list: a group heading or a binding.
#[derive(Debug, Clone)]
pub struct HelpRow {
    /// Group heading row (full-width divider); `keys` holds the title.
    pub heading: bool,
    /// Blank row of breathing room around a divider; never selected.
    pub spacer: bool,
    /// Key chord label (effective after overrides), or the heading title.
    pub keys: String,
    /// Header icon that also triggers the action (e.g. `◫`).
    pub icon: Option<&'static str>,
    /// Description.
    pub help: &'static str,
    /// Action to dispatch on click / Enter; `None` for display-only.
    pub action: Option<Action>,
}

impl HelpRow {
    /// Only binding rows can be selected.
    #[must_use]
    pub fn selectable(&self) -> bool {
        !self.heading && !self.spacer
    }

    fn spacer() -> Self {
        Self {
            heading: false,
            spacer: true,
            keys: String::new(),
            icon: None,
            help: "",
            action: None,
        }
    }
}

/// Blank rows above each group divider (the first group has the window's own
/// top padding instead).
const GAP_ABOVE_DIVIDER: usize = 2;
/// Blank rows between a divider and its first binding.
const GAP_BELOW_DIVIDER: usize = 1;

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
            if last_scope != Some(b.scope) {
                if last_scope.is_some() {
                    rows.extend((0..GAP_ABOVE_DIVIDER).map(|_| HelpRow::spacer()));
                }
                last_scope = Some(b.scope);
                rows.push(HelpRow {
                    heading: true,
                    spacer: false,
                    keys: b.scope.title().to_owned(),
                    icon: None,
                    help: "",
                    action: None,
                });
                rows.extend((0..GAP_BELOW_DIVIDER).map(|_| HelpRow::spacer()));
            }
            rows.push(HelpRow {
                heading: false,
                spacer: false,
                keys: effective_keys_label(b, overrides),
                icon: b.action.as_ref().and_then(binding_icon),
                help: b.help,
                action: b.action.clone(),
            });
        }
        let selected = rows.iter().position(HelpRow::selectable).unwrap_or(0);
        Self {
            rows,
            selected,
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
        let mut next = (cur + delta).rem_euclid(n);
        // Headings and spacers are not selectable: slide past them in the direction of travel.
        let dir = if delta < 0 { -1 } else { 1 };
        for _ in 0..n {
            if self.rows[usize::try_from(next).unwrap_or(0)].selectable() {
                break;
            }
            next = (next + dir).rem_euclid(n);
        }
        self.selected = usize::try_from(next).unwrap_or(0);
    }

    /// Scroll the list by `dir` rows (mouse wheel), keeping the selection on screen.
    pub fn scroll_by(&mut self, dir: i32) {
        let visible = self.list_height.max(1);
        let max = self.rows.len().saturating_sub(visible);
        self.scroll = if dir < 0 {
            self.scroll.saturating_sub(1)
        } else {
            (self.scroll + 1).min(max)
        };
        let last = (self.scroll + visible).min(self.rows.len());
        if self.selected < self.scroll {
            if let Some(i) = (self.scroll..last).find(|&i| self.rows[i].selectable()) {
                self.selected = i;
            }
        } else if self.selected >= last
            && let Some(i) = (self.scroll..last)
                .rev()
                .find(|&i| self.rows[i].selectable())
        {
            self.selected = i;
        }
    }

    /// Select the first (`home`) or last binding row.
    pub fn select_edge(&mut self, home: bool) {
        let pick = if home {
            self.rows.iter().position(HelpRow::selectable)
        } else {
            self.rows.iter().rposition(HelpRow::selectable)
        };
        if let Some(i) = pick {
            self.selected = i;
        }
    }
}
