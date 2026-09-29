//! Table-driven keymap (ADR-0007 / P1-S1 defaults + fallbacks).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::action::Action;

/// Map a key press to an [`Action`]. Pane-specific keys are filtered by the caller.
#[must_use]
pub fn map_key(key: KeyEvent) -> Action {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('b') => Action::ToggleNav,
        KeyCode::Char('[') => Action::PrevPage,
        KeyCode::Char(']') => Action::NextPage,
        KeyCode::Backspace => Action::Back,
        KeyCode::Left if alt => Action::Back,
        KeyCode::Right if alt => Action::Forward,
        KeyCode::Left if shift => Action::FocusNav,
        KeyCode::Right if shift => Action::FocusViewer,
        KeyCode::F(6) => Action::CycleFocus,
        _ => Action::None,
    }
}

/// Global bindings always applied (before pane-local handling).
#[must_use]
pub fn map_global(key: KeyEvent) -> Option<Action> {
    let action = map_key(key);
    match action {
        Action::Quit
        | Action::ToggleNav
        | Action::PrevPage
        | Action::NextPage
        | Action::Back
        | Action::Forward
        | Action::FocusNav
        | Action::FocusViewer
        | Action::CycleFocus => Some(action),
        _ => None,
    }
}
