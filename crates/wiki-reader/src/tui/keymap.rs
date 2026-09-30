//! Table-driven keymap (ADR-0007 / P1-S1 defaults + fallbacks).
//!
//! [`BINDINGS`] is the source of truth for Normal-mode global/pane maps and the
//! help overlay (P2-20). Overlay, Confirm, and `g`-chords stay special-cased;
//! they appear in the table as display-only rows.

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
    /// Help overlay: scroll / activate / dismiss.
    Help,
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

/// Where a binding applies (help sections + map filters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingScope {
    /// Always active in Normal mode.
    Global,
    /// Side-nav pane.
    Nav,
    /// Viewer pane.
    Viewer,
    /// Search overlay (display-only in help; special-cased in map).
    Overlay,
    /// Multi-key chords (display-only in help; special-cased in map).
    Chord,
}

/// How a Normal-mode key is matched against a [`Binding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matcher {
    /// Plain char (no Ctrl/Alt; Shift ok for capitals).
    PlainChar(char),
    /// Ctrl+char.
    CtrlChar(char),
    /// Alt+char **or** Alt+code (Ghostty Option encoding).
    AltCharOrCode(char, KeyCode),
    /// Plain key code (no Ctrl/Alt).
    PlainCode(KeyCode),
    /// Shift+code (no Ctrl/Alt).
    ShiftCode(KeyCode),
    /// Shift **or** Ctrl + code (block / group jump).
    ShiftOrCtrlCode(KeyCode),
    /// Alt+Shift+code.
    AltShiftCode(KeyCode),
    /// Code with any modifiers (`PageUp`, `F6`, Backspace).
    AnyCode(KeyCode),
    /// Plain Enter.
    PlainEnter,
    /// Plain Tab.
    PlainTab,
    /// `BackTab` or Shift+Tab.
    BackTab,
    /// Plain Space.
    PlainSpace,
    /// Shift+Space.
    ShiftSpace,
}

impl Matcher {
    #[must_use]
    fn matches(self, key: KeyEvent) -> bool {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let plain = no_ctrl_alt(key);
        match self {
            Matcher::PlainChar(c) => plain && key.code == KeyCode::Char(c),
            Matcher::CtrlChar(c) => ctrl && !alt && key.code == KeyCode::Char(c),
            Matcher::AltCharOrCode(c, code) => {
                alt && (key.code == KeyCode::Char(c) || key.code == code)
            }
            Matcher::PlainCode(code) => plain && key.code == code,
            Matcher::ShiftCode(code) => shift && plain && key.code == code,
            Matcher::ShiftOrCtrlCode(code) => {
                (shift || ctrl) && key.code == code && !(alt && shift)
            }
            Matcher::AltShiftCode(code) => alt && shift && key.code == code,
            Matcher::AnyCode(code) => key.code == code,
            Matcher::PlainEnter => plain && key.code == KeyCode::Enter,
            Matcher::PlainTab => plain && key.code == KeyCode::Tab,
            Matcher::BackTab => {
                key.code == KeyCode::BackTab || (shift && plain && key.code == KeyCode::Tab)
            }
            Matcher::PlainSpace => plain && key.code == KeyCode::Char(' '),
            Matcher::ShiftSpace => shift && plain && key.code == KeyCode::Char(' '),
        }
    }

    /// A representative key for drift tests.
    #[cfg(test)]
    #[must_use]
    #[allow(clippy::match_same_arms)]
    fn sample(self) -> KeyEvent {
        match self {
            Matcher::PlainChar(c) => KeyEvent::from(KeyCode::Char(c)),
            Matcher::CtrlChar(c) => KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL),
            Matcher::AltCharOrCode(c, _) => KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT),
            Matcher::PlainCode(code)
            | Matcher::AnyCode(code)
            | Matcher::ShiftOrCtrlCode(code)
            | Matcher::ShiftCode(code)
            | Matcher::AltShiftCode(code) => {
                let mods = match self {
                    Matcher::ShiftCode(_) | Matcher::ShiftOrCtrlCode(_) => KeyModifiers::SHIFT,
                    Matcher::AltShiftCode(_) => KeyModifiers::ALT | KeyModifiers::SHIFT,
                    _ => KeyModifiers::NONE,
                };
                if mods.is_empty() {
                    KeyEvent::from(code)
                } else {
                    KeyEvent::new(code, mods)
                }
            }
            Matcher::PlainEnter => KeyEvent::from(KeyCode::Enter),
            Matcher::PlainTab => KeyEvent::from(KeyCode::Tab),
            Matcher::BackTab => KeyEvent::from(KeyCode::BackTab),
            Matcher::PlainSpace => KeyEvent::from(KeyCode::Char(' ')),
            Matcher::ShiftSpace => KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT),
        }
    }
}

/// One keymap / help row.
#[derive(Debug, Clone)]
pub struct Binding {
    /// Keys shown in help (overridden display may replace this).
    pub keys: &'static str,
    /// Scope / help section.
    pub scope: BindingScope,
    /// Action when matched; `None` = display-only.
    pub action: Option<Action>,
    /// One-line help text.
    pub help: &'static str,
    /// Matcher for Normal-mode maps; `None` = display-only.
    pub matcher: Option<Matcher>,
}

/// Source of truth for Normal-mode bindings and the help overlay.
pub static BINDINGS: &[Binding] = &[
    // —— Global (Alt before plain for b/f) ——
    Binding {
        keys: "Alt+← / Alt+b",
        scope: BindingScope::Global,
        action: Some(Action::Back),
        help: "Back",
        matcher: Some(Matcher::AltCharOrCode('b', KeyCode::Left)),
    },
    Binding {
        keys: "Alt+→ / Alt+f",
        scope: BindingScope::Global,
        action: Some(Action::Forward),
        help: "Forward",
        matcher: Some(Matcher::AltCharOrCode('f', KeyCode::Right)),
    },
    Binding {
        keys: "q",
        scope: BindingScope::Global,
        action: Some(Action::Quit),
        help: "Quit",
        matcher: Some(Matcher::PlainChar('q')),
    },
    Binding {
        keys: "b",
        scope: BindingScope::Global,
        action: Some(Action::ToggleNav),
        help: "Toggle side nav",
        matcher: Some(Matcher::PlainChar('b')),
    },
    Binding {
        keys: "r",
        scope: BindingScope::Global,
        action: Some(Action::ToggleViewMode),
        help: "Toggle raw / rendered",
        matcher: Some(Matcher::PlainChar('r')),
    },
    Binding {
        keys: "e",
        scope: BindingScope::Global,
        action: Some(Action::OpenInEditor),
        help: "Open in editor",
        matcher: Some(Matcher::PlainChar('e')),
    },
    Binding {
        keys: "y",
        scope: BindingScope::Global,
        action: Some(Action::CopyPagePath),
        help: "Copy page path",
        matcher: Some(Matcher::PlainChar('y')),
    },
    Binding {
        keys: "Y",
        scope: BindingScope::Global,
        action: Some(Action::CopyLinkTarget),
        help: "Copy focused link target",
        matcher: Some(Matcher::PlainChar('Y')),
    },
    Binding {
        keys: "t",
        scope: BindingScope::Global,
        action: Some(Action::NewTab),
        help: "New tab",
        matcher: Some(Matcher::PlainChar('t')),
    },
    Binding {
        keys: "x",
        scope: BindingScope::Global,
        action: Some(Action::CloseTab),
        help: "Close tab",
        matcher: Some(Matcher::PlainChar('x')),
    },
    Binding {
        keys: "/",
        scope: BindingScope::Global,
        action: Some(Action::OpenSearch),
        help: "Search",
        matcher: Some(Matcher::PlainChar('/')),
    },
    Binding {
        keys: "Ctrl+k",
        scope: BindingScope::Global,
        action: Some(Action::OpenSearch),
        help: "Search",
        matcher: Some(Matcher::CtrlChar('k')),
    },
    Binding {
        keys: "?",
        scope: BindingScope::Global,
        action: Some(Action::OpenHelp),
        help: "Help",
        matcher: Some(Matcher::PlainChar('?')),
    },
    Binding {
        keys: "n",
        scope: BindingScope::Global,
        action: Some(Action::SearchNextMatch),
        help: "Next search match",
        matcher: Some(Matcher::PlainChar('n')),
    },
    Binding {
        keys: "N",
        scope: BindingScope::Global,
        action: Some(Action::SearchPrevMatch),
        help: "Previous search match",
        matcher: Some(Matcher::PlainChar('N')),
    },
    Binding {
        keys: "[",
        scope: BindingScope::Global,
        action: Some(Action::PrevPage),
        help: "Previous page",
        matcher: Some(Matcher::PlainChar('[')),
    },
    Binding {
        keys: "]",
        scope: BindingScope::Global,
        action: Some(Action::NextPage),
        help: "Next page",
        matcher: Some(Matcher::PlainChar(']')),
    },
    Binding {
        keys: "Backspace",
        scope: BindingScope::Global,
        action: Some(Action::Back),
        help: "Back",
        matcher: Some(Matcher::AnyCode(KeyCode::Backspace)),
    },
    Binding {
        keys: "Shift+←",
        scope: BindingScope::Global,
        action: Some(Action::FocusNav),
        help: "Focus side nav",
        matcher: Some(Matcher::ShiftCode(KeyCode::Left)),
    },
    Binding {
        keys: "Shift+→",
        scope: BindingScope::Global,
        action: Some(Action::FocusViewer),
        help: "Focus viewer",
        matcher: Some(Matcher::ShiftCode(KeyCode::Right)),
    },
    Binding {
        keys: "F6",
        scope: BindingScope::Global,
        action: Some(Action::CycleFocus),
        help: "Cycle pane focus",
        matcher: Some(Matcher::AnyCode(KeyCode::F(6))),
    },
    // —— Nav ——
    Binding {
        keys: "Shift+↑ / Ctrl+↑",
        scope: BindingScope::Nav,
        action: Some(Action::NavJumpUp),
        help: "Jump to previous group / search",
        matcher: Some(Matcher::ShiftOrCtrlCode(KeyCode::Up)),
    },
    Binding {
        keys: "Shift+↓ / Ctrl+↓",
        scope: BindingScope::Nav,
        action: Some(Action::NavJumpDown),
        help: "Jump to next group",
        matcher: Some(Matcher::ShiftOrCtrlCode(KeyCode::Down)),
    },
    Binding {
        keys: "↑",
        scope: BindingScope::Nav,
        action: Some(Action::NavStepUp),
        help: "Previous nav row",
        matcher: Some(Matcher::PlainCode(KeyCode::Up)),
    },
    Binding {
        keys: "Shift+Tab",
        scope: BindingScope::Nav,
        action: Some(Action::NavStepUp),
        help: "Previous nav row",
        matcher: Some(Matcher::BackTab),
    },
    Binding {
        keys: "↓",
        scope: BindingScope::Nav,
        action: Some(Action::NavStepDown),
        help: "Next nav row",
        matcher: Some(Matcher::AnyCode(KeyCode::Down)),
    },
    Binding {
        keys: "Tab",
        scope: BindingScope::Nav,
        action: Some(Action::NavStepDown),
        help: "Next nav row",
        matcher: Some(Matcher::PlainTab),
    },
    Binding {
        keys: "→",
        scope: BindingScope::Nav,
        action: Some(Action::NavExpand),
        help: "Expand / open into viewer",
        matcher: Some(Matcher::AnyCode(KeyCode::Right)),
    },
    Binding {
        keys: "←",
        scope: BindingScope::Nav,
        action: Some(Action::NavCollapse),
        help: "Collapse / parent group",
        matcher: Some(Matcher::AnyCode(KeyCode::Left)),
    },
    Binding {
        keys: "Enter",
        scope: BindingScope::Nav,
        action: Some(Action::NavActivate),
        help: "Open page (stay in nav) / toggle group",
        matcher: Some(Matcher::PlainEnter),
    },
    // —— Viewer ——
    Binding {
        keys: "Alt+Shift+↑",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingUp),
        help: "Previous heading",
        matcher: Some(Matcher::AltShiftCode(KeyCode::Up)),
    },
    Binding {
        keys: "Alt+Shift+↓",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingDown),
        help: "Next heading",
        matcher: Some(Matcher::AltShiftCode(KeyCode::Down)),
    },
    Binding {
        keys: "{",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingUp),
        help: "Previous heading",
        matcher: Some(Matcher::PlainChar('{')),
    },
    Binding {
        keys: "}",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingDown),
        help: "Next heading",
        matcher: Some(Matcher::PlainChar('}')),
    },
    Binding {
        keys: "Shift+↑ / Ctrl+↑",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerBlockUp),
        help: "Previous block",
        matcher: Some(Matcher::ShiftOrCtrlCode(KeyCode::Up)),
    },
    Binding {
        keys: "Shift+↓ / Ctrl+↓",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerBlockDown),
        help: "Next block",
        matcher: Some(Matcher::ShiftOrCtrlCode(KeyCode::Down)),
    },
    Binding {
        keys: "k",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerUp),
        help: "Cursor up",
        matcher: Some(Matcher::PlainChar('k')),
    },
    Binding {
        keys: "↑",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerUp),
        help: "Cursor up",
        matcher: Some(Matcher::PlainCode(KeyCode::Up)),
    },
    Binding {
        keys: "j",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerDown),
        help: "Cursor down",
        matcher: Some(Matcher::PlainChar('j')),
    },
    Binding {
        keys: "↓",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerDown),
        help: "Cursor down",
        matcher: Some(Matcher::PlainCode(KeyCode::Down)),
    },
    Binding {
        keys: "PgUp",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerPageUp),
        help: "Page up",
        matcher: Some(Matcher::AnyCode(KeyCode::PageUp)),
    },
    Binding {
        keys: "Shift+Space",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerPageUp),
        help: "Page up",
        matcher: Some(Matcher::ShiftSpace),
    },
    Binding {
        keys: "Space",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerPageDown),
        help: "Page down",
        matcher: Some(Matcher::PlainSpace),
    },
    Binding {
        keys: "PgDn",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerPageDown),
        help: "Page down",
        matcher: Some(Matcher::AnyCode(KeyCode::PageDown)),
    },
    Binding {
        keys: "Home",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHome),
        help: "Top of page",
        matcher: Some(Matcher::AnyCode(KeyCode::Home)),
    },
    Binding {
        keys: "End",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerEnd),
        help: "Bottom of page",
        matcher: Some(Matcher::AnyCode(KeyCode::End)),
    },
    Binding {
        keys: "G",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerEnd),
        help: "Bottom of page",
        matcher: Some(Matcher::PlainChar('G')),
    },
    Binding {
        keys: "Shift+Tab",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerBackTab),
        help: "Previous focusable item",
        matcher: Some(Matcher::BackTab),
    },
    Binding {
        keys: "Tab",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerTab),
        help: "Next focusable item",
        matcher: Some(Matcher::PlainTab),
    },
    Binding {
        keys: "Enter",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerActivate),
        help: "Activate focused item",
        matcher: Some(Matcher::PlainEnter),
    },
    Binding {
        keys: "f",
        scope: BindingScope::Viewer,
        action: Some(Action::FocusFooter),
        help: "Focus footer prev/next",
        matcher: Some(Matcher::PlainChar('f')),
    },
    // —— Display-only: chords ——
    Binding {
        keys: "gg",
        scope: BindingScope::Chord,
        action: Some(Action::ViewerHome),
        help: "Top of page",
        matcher: None,
    },
    Binding {
        keys: "gt",
        scope: BindingScope::Chord,
        action: Some(Action::NextTab),
        help: "Next tab",
        matcher: None,
    },
    Binding {
        keys: "gT",
        scope: BindingScope::Chord,
        action: Some(Action::PrevTab),
        help: "Previous tab",
        matcher: None,
    },
    // —— Display-only: search overlay ——
    Binding {
        keys: "Esc",
        scope: BindingScope::Overlay,
        action: Some(Action::CloseSearch),
        help: "Close search",
        matcher: None,
    },
    Binding {
        keys: "Enter",
        scope: BindingScope::Overlay,
        action: Some(Action::SearchActivate),
        help: "Open selected result",
        matcher: None,
    },
    Binding {
        keys: "Tab",
        scope: BindingScope::Overlay,
        action: Some(Action::SearchToggleMode),
        help: "Toggle Files / Content",
        matcher: None,
    },
    Binding {
        keys: "↑ / ↓",
        scope: BindingScope::Overlay,
        action: None,
        help: "Move selection",
        matcher: None,
    },
];

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

    if mode == InputMode::Help {
        return match key.code {
            KeyCode::Esc => (Some(Action::CloseHelp), Chord::None),
            KeyCode::Char('?') if no_ctrl_alt(key) => (Some(Action::CloseHelp), Chord::None),
            KeyCode::Enter if no_ctrl_alt(key) => (Some(Action::HelpActivate), Chord::None),
            KeyCode::Up => (Some(Action::HelpSelectDelta(-1)), Chord::None),
            KeyCode::Down => (Some(Action::HelpSelectDelta(1)), Chord::None),
            KeyCode::PageUp => (Some(Action::HelpPageDelta(-1)), Chord::None),
            KeyCode::PageDown => (Some(Action::HelpPageDelta(1)), Chord::None),
            KeyCode::Char('g')
                if no_ctrl_alt(key) && !key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                (Some(Action::HelpHome), Chord::None)
            }
            KeyCode::Char('G') if no_ctrl_alt(key) => (Some(Action::HelpEnd), Chord::None),
            KeyCode::Home => (Some(Action::HelpHome), Chord::None),
            KeyCode::End => (Some(Action::HelpEnd), Chord::None),
            _ => (None, Chord::None),
        };
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

/// Effective key label for help when overrides remap an action.
#[must_use]
pub fn effective_keys_label(
    binding: &Binding,
    overrides: &std::collections::BTreeMap<String, String>,
) -> String {
    let Some(action) = &binding.action else {
        return binding.keys.to_string();
    };
    for (name, chord) in overrides {
        if action_by_name(name).as_ref() == Some(action) {
            return chord.trim().to_string();
        }
    }
    binding.keys.to_string()
}

/// Global bindings (always, before pane-local). Esc does **not** quit.
#[must_use]
pub fn map_global(key: KeyEvent) -> Option<Action> {
    for b in BINDINGS {
        if b.scope != BindingScope::Global {
            continue;
        }
        let Some(matcher) = b.matcher else {
            continue;
        };
        if matcher.matches(key) {
            return b.action.clone();
        }
    }
    None
}

/// Pane-local bindings given current focus.
#[must_use]
pub fn map_pane(key: KeyEvent, focus: FocusPane) -> Option<Action> {
    let scope = match focus {
        FocusPane::Nav => BindingScope::Nav,
        FocusPane::Viewer => BindingScope::Viewer,
    };
    for b in BINDINGS {
        if b.scope != scope {
            continue;
        }
        let Some(matcher) = b.matcher else {
            continue;
        };
        if matcher.matches(key) {
            return b.action.clone();
        }
    }
    None
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

        // Overlay: plain letters type; Esc closes; Ctrl+C still quits.
        let (typed, _) = map(
            key(KeyCode::Char('q')),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(typed, Some(Action::SearchChar('q')));
        let (esc, _) = map(
            key(KeyCode::Esc),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(esc, Some(Action::CloseSearch));
        let (ctrl_c, _) = map(
            key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(ctrl_c, Some(Action::Quit));
    }

    #[test]
    fn keymap_bindings_round_trip() {
        for b in BINDINGS {
            let Some(matcher) = b.matcher else {
                continue;
            };
            let Some(want) = b.action.clone() else {
                continue;
            };
            let key = matcher.sample();
            let focus = match b.scope {
                BindingScope::Nav => FocusPane::Nav,
                BindingScope::Viewer | BindingScope::Global => FocusPane::Viewer,
                BindingScope::Overlay | BindingScope::Chord => continue,
            };
            let (got, _) = map_with_overrides(key, focus, InputMode::Normal, Chord::None, None);
            assert_eq!(
                got,
                Some(want),
                "binding keys={} scope={:?} sample={key:?}",
                b.keys,
                b.scope
            );
        }
    }

    #[test]
    fn keymap_no_bound_action_missing_from_table() {
        // Actions reachable from Normal map_global / map_pane / chords must appear.
        let mut in_table = std::collections::HashSet::new();
        for b in BINDINGS {
            if let Some(a) = &b.action {
                in_table.insert(format!("{a:?}"));
            }
        }
        let required = [
            Action::Quit,
            Action::ToggleNav,
            Action::PrevPage,
            Action::NextPage,
            Action::Back,
            Action::Forward,
            Action::OpenSearch,
            Action::OpenHelp,
            Action::FocusNav,
            Action::FocusViewer,
            Action::CycleFocus,
            Action::NavStepUp,
            Action::NavStepDown,
            Action::NavJumpUp,
            Action::NavJumpDown,
            Action::NavExpand,
            Action::NavCollapse,
            Action::NavActivate,
            Action::ViewerUp,
            Action::ViewerDown,
            Action::ViewerBlockUp,
            Action::ViewerBlockDown,
            Action::ViewerHeadingUp,
            Action::ViewerHeadingDown,
            Action::ViewerPageUp,
            Action::ViewerPageDown,
            Action::ViewerHome,
            Action::ViewerEnd,
            Action::ViewerTab,
            Action::ViewerBackTab,
            Action::ViewerActivate,
            Action::FocusFooter,
            Action::ToggleViewMode,
            Action::OpenInEditor,
            Action::CopyPagePath,
            Action::CopyLinkTarget,
            Action::NewTab,
            Action::CloseTab,
            Action::NextTab,
            Action::PrevTab,
            Action::SearchNextMatch,
            Action::SearchPrevMatch,
        ];
        for a in required {
            assert!(
                in_table.contains(&format!("{a:?}")),
                "Action {a:?} missing from BINDINGS"
            );
        }
    }

    #[test]
    fn keymap_key_override_changes_quit() {
        let mut over = std::collections::BTreeMap::new();
        over.insert("quit".into(), "Q".into());
        let (got, _) = map_with_overrides(
            key(KeyCode::Char('Q')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&over),
        );
        assert_eq!(got, Some(Action::Quit));
        let (old, _) = map_with_overrides(
            key(KeyCode::Char('q')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&over),
        );
        // Override steals Q; plain q still matches the table binding.
        assert_eq!(old, Some(Action::Quit));
    }

    #[test]
    fn keymap_focus_footer_on_f() {
        let (got, _) = map(
            key(KeyCode::Char('f')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(got, Some(Action::FocusFooter));
        let (alt_f, _) = map(
            key_mod(KeyCode::Char('f'), KeyModifiers::ALT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(alt_f, Some(Action::Forward));
        let (nav_f, _) = map(
            key(KeyCode::Char('f')),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(nav_f, None);
    }

    #[test]
    fn keymap_open_help_on_question() {
        let (got, _) = map(
            key(KeyCode::Char('?')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(got, Some(Action::OpenHelp));
        let (close, _) = map(
            key(KeyCode::Char('?')),
            FocusPane::Viewer,
            InputMode::Help,
            Chord::None,
        );
        assert_eq!(close, Some(Action::CloseHelp));
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
    }

    #[test]
    fn keymap_plain_letters_ignore_ctrl_alt() {
        for (c, focus, want) in [
            ('q', FocusPane::Viewer, Some(Action::Quit)),
            ('b', FocusPane::Viewer, Some(Action::ToggleNav)),
        ] {
            let (plain, _) = map(key(KeyCode::Char(c)), focus, InputMode::Normal, Chord::None);
            assert_eq!(plain, want);
            let (ctrl, _) = map(
                key_mod(KeyCode::Char(c), KeyModifiers::CONTROL),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            assert_eq!(ctrl, None, "Ctrl+{c}");
            let (alt, _) = map(
                key_mod(KeyCode::Char(c), KeyModifiers::ALT),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            // Alt+b is Back (global), not ToggleNav.
            if c == 'b' {
                assert_eq!(alt, Some(Action::Back));
            } else {
                assert_eq!(alt, None, "Alt+{c}");
            }
        }
    }

    #[test]
    fn keymap_confirm_y_n() {
        let (y, _) = map(
            key(KeyCode::Char('y')),
            FocusPane::Viewer,
            InputMode::Confirm,
            Chord::None,
        );
        assert_eq!(y, Some(Action::ConfirmOpen));
        let (n, _) = map(
            key(KeyCode::Char('n')),
            FocusPane::Viewer,
            InputMode::Confirm,
            Chord::None,
        );
        assert_eq!(n, Some(Action::ConfirmDecline));
        let (esc, _) = map(
            key(KeyCode::Esc),
            FocusPane::Viewer,
            InputMode::Confirm,
            Chord::None,
        );
        assert_eq!(esc, Some(Action::ConfirmDecline));
    }

    #[test]
    fn keymap_overlay_typing_rules() {
        let (plain, _) = map(
            key(KeyCode::Char('a')),
            FocusPane::Viewer,
            InputMode::Overlay,
            Chord::None,
        );
        assert_eq!(plain, Some(Action::SearchChar('a')));
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
    fn keymap_viewer_heading_and_block() {
        let (up, _) = map(
            key_mod(KeyCode::Up, KeyModifiers::ALT | KeyModifiers::SHIFT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(up, Some(Action::ViewerHeadingUp));
        let (block, _) = map(
            key_mod(KeyCode::Up, KeyModifiers::SHIFT),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(block, Some(Action::ViewerBlockUp));
        let (brace, _) = map(
            key(KeyCode::Char('{')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(brace, Some(Action::ViewerHeadingUp));
    }

    #[test]
    fn keymap_nav_step_and_expand() {
        let (step, _) = map(
            key(KeyCode::Down),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(step, Some(Action::NavStepDown));
        let (expand, _) = map(
            key(KeyCode::Right),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(expand, Some(Action::NavExpand));
    }
}
