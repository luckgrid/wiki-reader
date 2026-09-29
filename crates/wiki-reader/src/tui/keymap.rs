//! Table-driven keymap (ADR-0007 / P1-S1 defaults + fallbacks).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::action::Action;
use super::regions::status::FocusPane;

/// Global bindings (always, before pane-local).
#[must_use]
pub fn map_global(key: KeyEvent) -> Option<Action> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
        KeyCode::Char('b') => Some(Action::ToggleNav),
        KeyCode::Char('[') => Some(Action::PrevPage),
        KeyCode::Char(']') => Some(Action::NextPage),
        KeyCode::Backspace => Some(Action::Back),
        KeyCode::Left if alt => Some(Action::Back),
        KeyCode::Right if alt => Some(Action::Forward),
        KeyCode::Left if shift => Some(Action::FocusNav),
        KeyCode::Right if shift => Some(Action::FocusViewer),
        KeyCode::F(6) => Some(Action::CycleFocus),
        _ => None,
    }
}

/// Pane-local bindings given current focus.
#[must_use]
#[allow(clippy::match_same_arms)]
pub fn map_pane(key: KeyEvent, focus: FocusPane) -> Option<Action> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match focus {
        FocusPane::Nav => match key.code {
            KeyCode::Up if shift || ctrl => Some(Action::NavJumpUp),
            KeyCode::Down if shift || ctrl => Some(Action::NavJumpDown),
            KeyCode::Up | KeyCode::BackTab => Some(Action::NavStepUp),
            KeyCode::Tab if shift => Some(Action::NavStepUp),
            KeyCode::Down | KeyCode::Tab => Some(Action::NavStepDown),
            KeyCode::Right => Some(Action::NavExpand),
            KeyCode::Left => Some(Action::NavCollapse),
            KeyCode::Enter => Some(Action::NavActivate),
            _ => None,
        },
        FocusPane::Viewer => match key.code {
            KeyCode::Up if shift || ctrl => Some(Action::ViewerBlockUp),
            KeyCode::Down if shift || ctrl => Some(Action::ViewerBlockDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::ViewerUp),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::ViewerDown),
            KeyCode::PageUp => Some(Action::ViewerPageUp),
            KeyCode::Char(' ') if shift => Some(Action::ViewerPageUp),
            KeyCode::PageDown | KeyCode::Char(' ') => Some(Action::ViewerPageDown),
            KeyCode::Home | KeyCode::Char('g') => Some(Action::ViewerHome),
            KeyCode::End | KeyCode::Char('G') => Some(Action::ViewerEnd),
            KeyCode::BackTab => Some(Action::ViewerBackTab),
            KeyCode::Tab if shift => Some(Action::ViewerBackTab),
            KeyCode::Tab => Some(Action::ViewerTab),
            _ => None,
        },
    }
}
