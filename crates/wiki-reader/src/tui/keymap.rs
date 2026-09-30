//! Table-driven keymap (ADR-0007 / P1-S1 defaults + fallbacks).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::action::Action;
use super::focus::FocusPane;

/// Whether plain-letter global bindings are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Reader: `q`/`b`/`[`/`]` etc. fire.
    #[default]
    Normal,
    /// Overlay text input (search): plain letters type into the field.
    Overlay,
    /// External URL open confirmation in the status bar.
    Confirm,
}

/// Chord / pending-key state for multi-key sequences (`gg`, `gt`, `gT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chord {
    /// No pending key.
    #[default]
    None,
    /// Saw a lone `g`; next `g` → home, `t`/`T` → next/prev tab.
    PendingG,
}

/// True when CONTROL and ALT are absent (SHIFT alone is fine for capitals).
#[must_use]
fn no_ctrl_alt(key: KeyEvent) -> bool {
    !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}

/// Overlay typing: reject pure Ctrl and pure Alt+ASCII; allow `AltGr` (Ctrl+Alt) and Option non-ASCII.
#[must_use]
fn overlay_typeable(key: KeyEvent, c: char) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match (ctrl, alt) {
        (true, false) => false,
        (false, true) => !c.is_ascii(),
        (true, true) | (false, false) => true,
    }
}

/// Map a key in the given input mode. Returns `(action, next_chord)`.
#[must_use]
#[cfg(test)]
pub fn map(
    key: KeyEvent,
    focus: FocusPane,
    mode: InputMode,
    chord: Chord,
) -> (Option<Action>, Chord) {
    map_with_overrides(key, focus, mode, chord, None)
}

/// Like [`map`] with optional action-name → chord overrides from config.
#[must_use]
pub fn map_with_overrides(
    key: KeyEvent,
    focus: FocusPane,
    mode: InputMode,
    chord: Chord,
    overrides: Option<&std::collections::BTreeMap<String, String>>,
) -> (Option<Action>, Chord) {
    // Ctrl+C always quits (raw mode has no SIGINT).
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return (Some(Action::Quit), Chord::None);
    }

    if let Some(over) = overrides
        && let Some(action) = override_action(key, over)
    {
        return (Some(action), Chord::None);
    }

    if mode == InputMode::Overlay {
        return match key.code {
            KeyCode::Esc => (Some(Action::CloseSearch), Chord::None),
            KeyCode::Enter => (Some(Action::SearchActivate), Chord::None),
            KeyCode::Tab => (Some(Action::SearchToggleMode), Chord::None),
            KeyCode::Up => (Some(Action::SearchSelectDelta(-1)), Chord::None),
            KeyCode::Down => (Some(Action::SearchSelectDelta(1)), Chord::None),
            KeyCode::Backspace => (Some(Action::SearchBackspace), Chord::None),
            KeyCode::Char(c) if overlay_typeable(key, c) => {
                (Some(Action::SearchChar(c)), Chord::None)
            }
            _ => (None, Chord::None),
        };
    }

    if mode == InputMode::Confirm {
        return match key.code {
            KeyCode::Char('y' | 'Y') if no_ctrl_alt(key) => {
                (Some(Action::ConfirmOpen), Chord::None)
            }
            KeyCode::Char('n' | 'N') if no_ctrl_alt(key) => {
                (Some(Action::ConfirmDecline), Chord::None)
            }
            KeyCode::Esc => (Some(Action::ConfirmDecline), Chord::None),
            _ => (None, Chord::None),
        };
    }

    // `g` chords: `gg` home, `gt` next tab, `gT` prev tab. Lone `g` waits.
    if chord == Chord::PendingG {
        match key.code {
            KeyCode::Char('g')
                if no_ctrl_alt(key) && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                return (Some(Action::ViewerHome), Chord::None);
            }
            KeyCode::Char('t') if no_ctrl_alt(key) => return (Some(Action::NextTab), Chord::None),
            KeyCode::Char('T') if no_ctrl_alt(key) => return (Some(Action::PrevTab), Chord::None),
            _ => {} // cancel pending `g`; map this key normally below
        }
    } else if matches!(key.code, KeyCode::Char('g'))
        && no_ctrl_alt(key)
        && !key.modifiers.contains(KeyModifiers::SHIFT)
    {
        return (None, Chord::PendingG);
    }
    let chord = Chord::None;

    if let Some(a) = map_global(key) {
        return (Some(a), chord);
    }
    (map_pane(key, focus), chord)
}

/// Match a single-character override like `"quit" = "Q"` (plain char, no mods).
fn override_action(
    key: KeyEvent,
    overrides: &std::collections::BTreeMap<String, String>,
) -> Option<Action> {
    if !key.modifiers.is_empty()
        && key.modifiers != KeyModifiers::SHIFT
        && key.modifiers != KeyModifiers::NONE
    {
        // Allow SHIFT for uppercase letters only via Char already uppercased.
    }
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    for (name, chord) in overrides {
        let chord = chord.trim();
        if chord.len() == 1 && chord.starts_with(c) {
            return action_by_name(name);
        }
    }
    None
}

fn action_by_name(name: &str) -> Option<Action> {
    Some(match name {
        "quit" => Action::Quit,
        "toggle_nav" | "toggle-nav" => Action::ToggleNav,
        "open_in_editor" | "open-in-editor" | "editor" => Action::OpenInEditor,
        "open_search" | "open-search" | "search" => Action::OpenSearch,
        "prev_page" | "prev-page" => Action::PrevPage,
        "next_page" | "next-page" => Action::NextPage,
        "back" => Action::Back,
        "forward" => Action::Forward,
        "toggle_view" | "toggle-view" | "raw" => Action::ToggleViewMode,
        _ => return None,
    })
}

/// Global bindings (always, before pane-local). Esc does **not** quit.
#[must_use]
pub fn map_global(key: KeyEvent) -> Option<Action> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let plain = no_ctrl_alt(key);

    match key.code {
        // Ghostty on macOS sends Option+←/→ as Alt+b / Alt+f (readline), not arrows.
        KeyCode::Char('b') | KeyCode::Left if alt => Some(Action::Back),
        KeyCode::Char('f') | KeyCode::Right if alt => Some(Action::Forward),
        KeyCode::Char('q') if plain => Some(Action::Quit),
        KeyCode::Char('b') if plain => Some(Action::ToggleNav),
        KeyCode::Char('r') if plain => Some(Action::ToggleViewMode),
        KeyCode::Char('e') if plain => Some(Action::OpenInEditor),
        KeyCode::Char('y') if plain => Some(Action::CopyPagePath),
        KeyCode::Char('Y') if plain => Some(Action::CopyLinkTarget),
        KeyCode::Char('t') if plain => Some(Action::NewTab),
        KeyCode::Char('x') if plain => Some(Action::CloseTab),
        KeyCode::Char('/') if plain => Some(Action::OpenSearch),
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::OpenSearch)
        }
        KeyCode::Char('n') if plain => Some(Action::SearchNextMatch),
        KeyCode::Char('N') if plain => Some(Action::SearchPrevMatch),
        KeyCode::Char('[') if plain => Some(Action::PrevPage),
        KeyCode::Char(']') if plain => Some(Action::NextPage),
        KeyCode::Backspace => Some(Action::Back),
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
    let plain = no_ctrl_alt(key);

    match focus {
        FocusPane::Nav => match key.code {
            KeyCode::Up if shift || ctrl => Some(Action::NavJumpUp),
            KeyCode::Down if shift || ctrl => Some(Action::NavJumpDown),
            KeyCode::Up | KeyCode::BackTab => Some(Action::NavStepUp),
            KeyCode::Tab if shift && plain => Some(Action::NavStepUp),
            KeyCode::Down => Some(Action::NavStepDown),
            KeyCode::Tab if plain => Some(Action::NavStepDown),
            KeyCode::Right => Some(Action::NavExpand),
            KeyCode::Left => Some(Action::NavCollapse),
            KeyCode::Enter if plain => Some(Action::NavActivate),
            _ => None,
        },
        FocusPane::Viewer => {
            let alt = key.modifiers.contains(KeyModifiers::ALT);
            match key.code {
                // Alt+Shift+↑/↓ (primary); {/} fallback when Alt+Shift is unreliable
                KeyCode::Up if alt && shift => Some(Action::ViewerHeadingUp),
                KeyCode::Down if alt && shift => Some(Action::ViewerHeadingDown),
                KeyCode::Char('{') if plain => Some(Action::ViewerHeadingUp),
                KeyCode::Char('}') if plain => Some(Action::ViewerHeadingDown),
                KeyCode::Up if shift || ctrl => Some(Action::ViewerBlockUp),
                KeyCode::Down if shift || ctrl => Some(Action::ViewerBlockDown),
                KeyCode::Char('k') if plain => Some(Action::ViewerUp),
                KeyCode::Up if plain => Some(Action::ViewerUp),
                KeyCode::Char('j') if plain => Some(Action::ViewerDown),
                KeyCode::Down if plain => Some(Action::ViewerDown),
                KeyCode::PageUp => Some(Action::ViewerPageUp),
                KeyCode::Char(' ') if shift => Some(Action::ViewerPageUp),
                KeyCode::Char(' ') if plain => Some(Action::ViewerPageDown),
                KeyCode::PageDown => Some(Action::ViewerPageDown),
                KeyCode::Home => Some(Action::ViewerHome),
                KeyCode::End => Some(Action::ViewerEnd),
                KeyCode::Char('G') if plain => Some(Action::ViewerEnd),
                KeyCode::BackTab => Some(Action::ViewerBackTab),
                KeyCode::Tab if shift && plain => Some(Action::ViewerBackTab),
                KeyCode::Tab if plain => Some(Action::ViewerTab),
                KeyCode::Enter if plain => Some(Action::ViewerActivate),
                _ => None,
            }
        }
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
    #[allow(clippy::too_many_lines)]
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

        // gt / gT tab chords
        let (_, c) = map(
            key(KeyCode::Char('g')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(c, Chord::PendingG);
        let (next, _) = map(
            key(KeyCode::Char('t')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::PendingG,
        );
        assert_eq!(next, Some(Action::NextTab));
        let (prev, _) = map(
            key(KeyCode::Char('T')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::PendingG,
        );
        assert_eq!(prev, Some(Action::PrevTab));
        let (new_tab, _) = map(
            key(KeyCode::Char('t')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(new_tab, Some(Action::NewTab));

        // Overlay types plain letters into the search field
        let (a, _) = map(
            key(KeyCode::Char('q')),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(a, Some(Action::SearchChar('q')));
        // Esc closes search
        let (a, _) = map(
            key(KeyCode::Esc),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(a, Some(Action::CloseSearch));
        // Ctrl+C still quits in overlay
        let (a, _) = map(
            key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(a, Some(Action::Quit));
    }

    #[test]
    fn keymap_heading_jump_alt_shift() {
        let up = key_mod(KeyCode::Up, KeyModifiers::ALT | KeyModifiers::SHIFT);
        let down = key_mod(KeyCode::Down, KeyModifiers::ALT | KeyModifiers::SHIFT);
        let (got_up, _) = map(up, FocusPane::Viewer, InputMode::Normal, Chord::None);
        let (got_down, _) = map(down, FocusPane::Viewer, InputMode::Normal, Chord::None);
        assert_eq!(got_up, Some(Action::ViewerHeadingUp));
        assert_eq!(got_down, Some(Action::ViewerHeadingDown));
        let block = key_mod(KeyCode::Up, KeyModifiers::SHIFT);
        let (got_block, _) = map(block, FocusPane::Viewer, InputMode::Normal, Chord::None);
        assert_eq!(got_block, Some(Action::ViewerBlockUp));
    }

    #[test]
    fn keymap_heading_jump_brace_fallback() {
        let (got_up, _) = map(
            key(KeyCode::Char('{')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        let (got_down, _) = map(
            key(KeyCode::Char('}')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(got_up, Some(Action::ViewerHeadingUp));
        assert_eq!(got_down, Some(Action::ViewerHeadingDown));
        // Shift+[/] still produce {/} on many terminals; modifiers must not steal the binding.
        let (got_shift, _) = map(
            key_mod(KeyCode::Char('{'), KeyModifiers::SHIFT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(got_shift, Some(Action::ViewerHeadingUp));
        // Nav focus: braces stay unbound (Alt+Shift remains the nav group jump).
        let (nav, _) = map(
            key(KeyCode::Char('{')),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(nav, None);
    }

    #[test]
    fn keymap_key_override_changes_quit() {
        let mut overs = std::collections::BTreeMap::new();
        overs.insert("quit".into(), "Q".into());
        let (got, _) = map_with_overrides(
            key(KeyCode::Char('Q')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&overs),
        );
        assert_eq!(got, Some(Action::Quit));
        // Default lowercase q still works via map_global when not overridden by that char.
        let (got_q, _) = map_with_overrides(
            key(KeyCode::Char('q')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&overs),
        );
        assert_eq!(got_q, Some(Action::Quit));
    }

    #[test]
    fn keymap_alt_b_f_back_forward_and_plain_keys_ignore_mods() {
        let (back, _) = map(
            key_mod(KeyCode::Char('b'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(back, Some(Action::Back));
        let (fwd, _) = map(
            key_mod(KeyCode::Char('f'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(fwd, Some(Action::Forward));
        let (alt_q, _) = map(
            key_mod(KeyCode::Char('q'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(alt_q, None);
        let (ctrl_b, _) = map(
            key_mod(KeyCode::Char('b'), KeyModifiers::CONTROL),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(ctrl_b, None);
        // Arrow forms still work when the terminal actually sends them.
        let (alt_left, _) = map(
            key_mod(KeyCode::Left, KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(alt_left, Some(Action::Back));
        let (alt_right, _) = map(
            key_mod(KeyCode::Right, KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(alt_right, Some(Action::Forward));
    }

    #[test]
    fn keymap_confirm_rejects_alt_ctrl_y() {
        for mods in [KeyModifiers::ALT, KeyModifiers::CONTROL] {
            let (got, _) = map(
                key_mod(KeyCode::Char('y'), mods),
                FocusPane::Viewer,
                InputMode::Confirm,
                Chord::None,
            );
            assert_eq!(got, None, "mods={mods:?}");
        }
        let (ok, _) = map(
            key(KeyCode::Char('y')),
            FocusPane::Viewer,
            InputMode::Confirm,
            Chord::None,
        );
        assert_eq!(ok, Some(Action::ConfirmOpen));
    }

    #[test]
    fn keymap_overlay_alt_b_neither_types_nor_back() {
        let (got, _) = map(
            key_mod(KeyCode::Char('b'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(got, None);
    }

    #[test]
    fn keymap_overlay_altgr_and_option_non_ascii() {
        let (altgr, _) = map(
            key_mod(
                KeyCode::Char('{'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(altgr, Some(Action::SearchChar('{')));
        let (pure_alt_ascii, _) = map(
            key_mod(KeyCode::Char('a'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(pure_alt_ascii, None);
        let (option_non_ascii, _) = map(
            key_mod(KeyCode::Char('å'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(option_non_ascii, Some(Action::SearchChar('å')));
        let (pure_ctrl, _) = map(
            key_mod(KeyCode::Char('a'), KeyModifiers::CONTROL),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(pure_ctrl, None);
    }

    #[test]
    fn keymap_enter_tab_require_no_ctrl_alt() {
        for focus in [FocusPane::Viewer, FocusPane::Nav] {
            let (plain_enter, _) = map(key(KeyCode::Enter), focus, InputMode::Normal, Chord::None);
            assert!(plain_enter.is_some(), "plain Enter focus={focus:?}");
            for mods in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
                let (enter, _) = map(
                    key_mod(KeyCode::Enter, mods),
                    focus,
                    InputMode::Normal,
                    Chord::None,
                );
                assert_eq!(enter, None, "Enter mods={mods:?} focus={focus:?}");
                let (tab, _) = map(
                    key_mod(KeyCode::Tab, mods),
                    focus,
                    InputMode::Normal,
                    Chord::None,
                );
                assert_eq!(tab, None, "Tab mods={mods:?} focus={focus:?}");
            }
        }
        let (viewer_tab, _) = map(
            key(KeyCode::Tab),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(viewer_tab, Some(Action::ViewerTab));
        let (shift_tab, _) = map(
            key_mod(KeyCode::Tab, KeyModifiers::SHIFT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(shift_tab, Some(Action::ViewerBackTab));
        let (nav_tab, _) = map(
            key(KeyCode::Tab),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(nav_tab, Some(Action::NavStepDown));
    }
}
