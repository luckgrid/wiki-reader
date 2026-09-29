//! Table-driven keymap (ADR-0007 / P1-S1 defaults + fallbacks).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::action::Action;
use super::regions::status::FocusPane;

/// Whether plain-letter global bindings are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Reader: `q`/`b`/`[`/`]` etc. fire.
    #[default]
    Normal,
    /// Overlay text input (search): plain letters type into the field.
    Overlay,
}

/// Chord / pending-key state for multi-key sequences (`gg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chord {
    /// No pending key.
    #[default]
    None,
    /// Saw a lone `g`; next `g` → home.
    PendingG,
}

/// Map a key in the given input mode. Returns `(action, next_chord)`.
#[must_use]
pub fn map(
    key: KeyEvent,
    focus: FocusPane,
    mode: InputMode,
    chord: Chord,
) -> (Option<Action>, Chord) {
    // Ctrl+C always quits (raw mode has no SIGINT).
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return (Some(Action::Quit), Chord::None);
    }

    if mode == InputMode::Overlay {
        // Esc reserved for closing overlays (P1-10); no global plain-letter binds.
        return (None, Chord::None);
    }

    // `gg` chord (viewer home). Lone `g` waits; other keys cancel.
    if matches!(key.code, KeyCode::Char('g')) && !key.modifiers.contains(KeyModifiers::SHIFT) {
        return match chord {
            Chord::PendingG => (Some(Action::ViewerHome), Chord::None),
            Chord::None => (None, Chord::PendingG),
        };
    }
    let chord = Chord::None;

    if let Some(a) = map_global(key) {
        return (Some(a), chord);
    }
    (map_pane(key, focus), chord)
}

/// Global bindings (always, before pane-local). Esc does **not** quit.
#[must_use]
pub fn map_global(key: KeyEvent) -> Option<Action> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
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
            KeyCode::Home => Some(Action::ViewerHome),
            KeyCode::End | KeyCode::Char('G') => Some(Action::ViewerEnd),
            KeyCode::BackTab => Some(Action::ViewerBackTab),
            KeyCode::Tab if shift => Some(Action::ViewerBackTab),
            KeyCode::Tab => Some(Action::ViewerTab),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::from(code)
    }

    fn key_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn keymap_table_bindings() {
        let cases: &[(KeyEvent, FocusPane, Option<Action>)] = &[
            (
                key(KeyCode::Char('q')),
                FocusPane::Viewer,
                Some(Action::Quit),
            ),
            (
                key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
                FocusPane::Viewer,
                Some(Action::Quit),
            ),
            (key(KeyCode::Esc), FocusPane::Viewer, None), // reserved for overlays
            (
                key(KeyCode::Char('b')),
                FocusPane::Viewer,
                Some(Action::ToggleNav),
            ),
            (
                key(KeyCode::Char('[')),
                FocusPane::Viewer,
                Some(Action::PrevPage),
            ),
            (
                key(KeyCode::Char(']')),
                FocusPane::Viewer,
                Some(Action::NextPage),
            ),
            (
                key(KeyCode::F(6)),
                FocusPane::Viewer,
                Some(Action::CycleFocus),
            ),
            (
                key_mod(KeyCode::Left, KeyModifiers::SHIFT),
                FocusPane::Viewer,
                Some(Action::FocusNav),
            ),
            (
                key_mod(KeyCode::Up, KeyModifiers::CONTROL),
                FocusPane::Nav,
                Some(Action::NavJumpUp),
            ),
            (
                key_mod(KeyCode::Down, KeyModifiers::CONTROL),
                FocusPane::Viewer,
                Some(Action::ViewerBlockDown),
            ),
            (
                key(KeyCode::Char('G')),
                FocusPane::Viewer,
                Some(Action::ViewerEnd),
            ),
            (
                key(KeyCode::Tab),
                FocusPane::Viewer,
                Some(Action::ViewerTab),
            ),
        ];
        for (k, focus, expected) in cases {
            let (got, _) = map(*k, *focus, InputMode::Normal, Chord::None);
            assert_eq!(got, *expected, "key={k:?} focus={focus:?}");
        }

        // gg chord
        let (a1, c1) = map(
            key(KeyCode::Char('g')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(a1, None);
        assert_eq!(c1, Chord::PendingG);
        let (a2, c2) = map(
            key(KeyCode::Char('g')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::PendingG,
        );
        assert_eq!(a2, Some(Action::ViewerHome));
        assert_eq!(c2, Chord::None);

        // Overlay swallows plain letters
        let (a, _) = map(
            key(KeyCode::Char('q')),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(a, None);
        // Ctrl+C still quits in overlay
        let (a, _) = map(
            key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(a, Some(Action::Quit));
    }
}
