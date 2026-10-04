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
    /// Options overlay: select / apply / dismiss.
    Options,
    /// Modal viewer (table, later image): every key goes to the modal.
    Modal,
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
    /// Table, image / diagram, and code viewers (display-only in help; the modal reads its own keys).
    Viewers,
}

impl BindingScope {
    /// Section title shared by the help overlay and the generated ui-spec keymap.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Nav => "Side nav",
            Self::Viewer => "View",
            Self::Chord => "Chords",
            Self::Overlay => "Search overlay",
            Self::Viewers => "Table, image and code viewers",
        }
    }
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
    /// Ctrl + code, without Shift/Alt (new-tab combos). Cmd is not used: terminals and
    /// macOS keep it (Ghostty maps Cmd+Enter to full screen).
    CtrlCode(KeyCode),
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
            Matcher::CtrlCode(code) => ctrl && !shift && !alt && key.code == code,
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
            | Matcher::CtrlCode(code)
            | Matcher::AltShiftCode(code) => {
                let mods = match self {
                    Matcher::ShiftCode(_) | Matcher::ShiftOrCtrlCode(_) => KeyModifiers::SHIFT,
                    Matcher::CtrlCode(_) => KeyModifiers::CONTROL,
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

/// Chrome icon that also triggers `action`, shown next to its key in help.
#[must_use]
pub fn binding_icon(action: &Action) -> Option<&'static str> {
    match action {
        Action::ToggleNav => Some("◫"),
        Action::OpenOptions => Some("⚙"),
        Action::OpenHelp => Some("?"),
        Action::Quit => Some("✕"),
        _ => None,
    }
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
        keys: "Backspace",
        scope: BindingScope::Global,
        action: Some(Action::Back),
        help: "Back",
        matcher: Some(Matcher::AnyCode(KeyCode::Backspace)),
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
        help: "Copy file path",
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
        keys: ",",
        scope: BindingScope::Global,
        action: Some(Action::OpenOptions),
        help: "Options (toggle)",
        matcher: Some(Matcher::PlainChar(',')),
    },
    Binding {
        keys: "c",
        scope: BindingScope::Global,
        action: Some(Action::OpenOptions),
        help: "Options (toggle)",
        matcher: Some(Matcher::PlainChar('c')),
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
        help: "Focus view",
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
        keys: "Ctrl+→",
        scope: BindingScope::Nav,
        action: Some(Action::NewTabFocusView),
        help: "Open page in a new tab and focus view",
        matcher: Some(Matcher::CtrlCode(KeyCode::Right)),
    },
    Binding {
        keys: "→",
        scope: BindingScope::Nav,
        action: Some(Action::NavExpand),
        help: "Expand / open into view",
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
        keys: "Ctrl+Enter",
        scope: BindingScope::Nav,
        action: Some(Action::NewTab),
        help: "Open page in a new tab",
        matcher: Some(Matcher::CtrlCode(KeyCode::Enter)),
    },
    Binding {
        keys: "Shift+Enter",
        scope: BindingScope::Nav,
        action: Some(Action::NewTab),
        help: "Open page in a new tab",
        matcher: Some(Matcher::ShiftCode(KeyCode::Enter)),
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
        keys: "{",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingUp),
        help: "Previous heading",
        matcher: Some(Matcher::PlainChar('{')),
    },
    Binding {
        keys: "Alt+Shift+↓",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerHeadingDown),
        help: "Next heading",
        matcher: Some(Matcher::AltShiftCode(KeyCode::Down)),
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
        keys: "←",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerLeft),
        help: "Cursor left (at edge toward nav: focus side nav)",
        matcher: Some(Matcher::PlainCode(KeyCode::Left)),
    },
    Binding {
        keys: "→",
        scope: BindingScope::Viewer,
        action: Some(Action::ViewerRight),
        help: "Cursor right (at edge toward nav: focus side nav)",
        matcher: Some(Matcher::PlainCode(KeyCode::Right)),
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
        keys: "Ctrl+Enter",
        scope: BindingScope::Viewer,
        action: Some(Action::NewTab),
        help: "Open focused link in a new tab",
        matcher: Some(Matcher::CtrlCode(KeyCode::Enter)),
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
    Binding {
        keys: "Home / End",
        scope: BindingScope::Overlay,
        action: None,
        help: "Jump to first / last result",
        matcher: None,
    },
    Binding {
        keys: "PgUp / PgDn",
        scope: BindingScope::Overlay,
        action: None,
        help: "Page results",
        matcher: None,
    },
    Binding {
        keys: "↑↓←→ / hjkl",
        scope: BindingScope::Viewers,
        action: None,
        help: "Move the table cell cursor, pan a picture, or scroll code",
        matcher: None,
    },
    Binding {
        keys: "/",
        scope: BindingScope::Viewers,
        action: None,
        help: "Table: filter rows",
        matcher: None,
    },
    Binding {
        keys: "s",
        scope: BindingScope::Viewers,
        action: None,
        help: "Table: sort by the current column",
        matcher: None,
    },
    Binding {
        keys: "y / Y",
        scope: BindingScope::Viewers,
        action: None,
        help: "Table: copy cell / row; code: copy the block (y)",
        matcher: None,
    },
    Binding {
        keys: "Home / End / 0 / $",
        scope: BindingScope::Viewers,
        action: None,
        help: "Code: jump to the start / end of the line",
        matcher: None,
    },
    Binding {
        keys: "+ / -",
        scope: BindingScope::Viewers,
        action: None,
        help: "Picture: zoom in / out",
        matcher: None,
    },
    Binding {
        keys: "0",
        scope: BindingScope::Viewers,
        action: None,
        help: "Picture: fit the window, reset zoom",
        matcher: None,
    },
    Binding {
        keys: "a",
        scope: BindingScope::Viewers,
        action: None,
        help: "Picture: toggle fit / actual size",
        matcher: None,
    },
    Binding {
        keys: "Tab / Shift+Tab",
        scope: BindingScope::Viewers,
        action: None,
        help: "Picture: next / previous image or diagram",
        matcher: None,
    },
    Binding {
        keys: "v",
        scope: BindingScope::Viewers,
        action: None,
        help: "Diagram: cycle image / text / source",
        matcher: None,
    },
    Binding {
        keys: "Esc / q",
        scope: BindingScope::Viewers,
        action: None,
        help: "Close the viewer",
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

    // Before overrides: typed filter text must not trigger remapped actions.
    if mode == InputMode::Modal {
        return (Some(Action::ModalKey(key)), Chord::None);
    }

    // Overrides remap reader keys only. Overlays (search text, Help, Options, the open-link
    // confirm) own their keys, so `quit = "q"` must not fire while typing a query.
    if mode == InputMode::Normal
        && let Some(over) = overrides
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

    if mode == InputMode::Options {
        return (
            match key.code {
                KeyCode::Esc => Some(Action::CloseOptions),
                // The open keys also close it, so `,` / `c` toggle the window.
                KeyCode::Char(',' | 'c') if no_ctrl_alt(key) => Some(Action::CloseOptions),
                KeyCode::Up | KeyCode::Char('k') => Some(Action::OptionsUp),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::OptionsDown),
                KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => Some(Action::OptionsApply),
                _ => None,
            },
            Chord::None,
        );
    }

    if mode == InputMode::Overlay {
        return match key.code {
            KeyCode::Esc => (Some(Action::CloseSearch), Chord::None),
            KeyCode::Enter => (Some(Action::SearchActivate), Chord::None),
            KeyCode::Tab => (Some(Action::SearchToggleMode), Chord::None),
            KeyCode::Up => (Some(Action::SearchSelectDelta(-1)), Chord::None),
            KeyCode::Down => (Some(Action::SearchSelectDelta(1)), Chord::None),
            KeyCode::Home => (Some(Action::SearchJump(true)), Chord::None),
            KeyCode::End => (Some(Action::SearchJump(false)), Chord::None),
            KeyCode::PageUp => (Some(Action::SearchPageDelta(-1)), Chord::None),
            KeyCode::PageDown => (Some(Action::SearchPageDelta(1)), Chord::None),
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
        let mut chars = chord.chars();
        if chars.next() == Some(c) && chars.next().is_none() {
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
        "open_options" | "open-options" | "options" => Action::OpenOptions,
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

/// One help line: adjacent bindings that do the same thing share it, keys joined by `" / "`.
#[derive(Debug, Clone)]
pub struct HelpEntry {
    pub scope: BindingScope,
    pub keys: String,
    pub action: Option<Action>,
    pub help: &'static str,
}

/// [`BINDINGS`] as help lines: a binding folds into the previous line when scope, action
/// and help text all match. Keep alternates adjacent in the table for this to apply.
#[must_use]
pub fn help_entries(overrides: &std::collections::BTreeMap<String, String>) -> Vec<HelpEntry> {
    let mut out: Vec<HelpEntry> = Vec::new();
    for b in BINDINGS {
        let keys = effective_keys_label(b, overrides);
        if let Some(prev) = out.last_mut()
            && prev.scope == b.scope
            && prev.help == b.help
            && b.action.is_some()
            && prev.action == b.action
        {
            if !prev.keys.split(" / ").any(|k| k == keys) {
                prev.keys = format!("{} / {keys}", prev.keys);
            }
            continue;
        }
        out.push(HelpEntry {
            scope: b.scope,
            keys,
            action: b.action.clone(),
            help: b.help,
        });
    }
    out
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
                BindingScope::Overlay | BindingScope::Chord | BindingScope::Viewers => continue,
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
            Action::OpenOptions,
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
            Action::ViewerLeft,
            Action::ViewerRight,
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
            Action::NewTabFocusView,
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
    fn overrides_apply_only_in_normal_mode() {
        let mut over = std::collections::BTreeMap::new();
        over.insert("quit".into(), "q".into());
        over.insert("open_search".into(), "y".into());
        let go = |mode, c| {
            map_with_overrides(
                key(KeyCode::Char(c)),
                FocusPane::Viewer,
                mode,
                Chord::None,
                Some(&over),
            )
            .0
        };
        // Typing into the search box is typing, not quitting or re-opening search.
        assert_eq!(go(InputMode::Overlay, 'q'), Some(Action::SearchChar('q')));
        assert_eq!(go(InputMode::Overlay, 'y'), Some(Action::SearchChar('y')));
        // The open-link confirm keeps `y` for yes.
        assert_ne!(go(InputMode::Confirm, 'y'), Some(Action::OpenSearch));
        // The reader still honours them.
        assert_eq!(go(InputMode::Normal, 'y'), Some(Action::OpenSearch));
    }

    #[test]
    fn a_non_ascii_override_matches_by_character_not_byte_length() {
        let mut over = std::collections::BTreeMap::new();
        over.insert("quit".into(), "é".into());
        let (got, _) = map_with_overrides(
            key(KeyCode::Char('é')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&over),
        );
        assert_eq!(got, Some(Action::Quit));
        // Two characters are not a single-key override.
        over.insert("quit".into(), "éé".into());
        let (none, _) = map_with_overrides(
            key(KeyCode::Char('é')),
            FocusPane::Viewer,
            InputMode::Normal,
            Chord::None,
            Some(&over),
        );
        assert_ne!(none, Some(Action::Quit));
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
    fn keymap_options_mode() {
        let m = |code, mode| map(key(code), FocusPane::Viewer, mode, Chord::None).0;
        assert_eq!(
            m(KeyCode::Char(','), InputMode::Normal),
            Some(Action::OpenOptions)
        );
        assert_eq!(
            m(KeyCode::Esc, InputMode::Options),
            Some(Action::CloseOptions)
        );
        assert_eq!(
            m(KeyCode::Char('j'), InputMode::Options),
            Some(Action::OptionsDown)
        );
        assert_eq!(m(KeyCode::Up, InputMode::Options), Some(Action::OptionsUp));
        for code in [KeyCode::Enter, KeyCode::Right, KeyCode::Char(' ')] {
            assert_eq!(m(code, InputMode::Options), Some(Action::OptionsApply));
        }
        // `,` and `c` toggle: they open from Normal and close from Options.
        for c in [',', 'c'] {
            assert_eq!(
                m(KeyCode::Char(c), InputMode::Normal),
                Some(Action::OpenOptions),
                "open {c}"
            );
            assert_eq!(
                m(KeyCode::Char(c), InputMode::Options),
                Some(Action::CloseOptions),
                "close {c}"
            );
        }
        // Ctrl+C stays quit even inside the window.
        assert_eq!(
            map(
                key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL),
                FocusPane::Viewer,
                InputMode::Options,
                Chord::None
            )
            .0,
            Some(Action::Quit)
        );
        assert_eq!(m(KeyCode::Char('q'), InputMode::Options), None);
    }

    #[test]
    fn keymap_enter_tab_require_no_ctrl_alt() {
        for focus in [FocusPane::Viewer, FocusPane::Nav] {
            let (plain_enter, _) = map(key(KeyCode::Enter), focus, InputMode::Normal, Chord::None);
            assert!(plain_enter.is_some(), "plain Enter focus={focus:?}");
            let (ctrl_enter, _) = map(
                key_mod(KeyCode::Enter, KeyModifiers::CONTROL),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            assert_eq!(
                ctrl_enter,
                Some(Action::NewTab),
                "Ctrl+Enter opens a new tab focus={focus:?}"
            );
            let (alt_enter, _) = map(
                key_mod(KeyCode::Enter, KeyModifiers::ALT),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            assert_eq!(alt_enter, None, "Alt+Enter focus={focus:?}");
            for mods in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
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
    fn keymap_ctrl_new_tab_precedes_plain_enter_and_any_right() {
        for (code, focus) in [
            (KeyCode::Enter, FocusPane::Nav),
            (KeyCode::Right, FocusPane::Nav),
            (KeyCode::Enter, FocusPane::Viewer),
        ] {
            let (ctrl, _) = map(
                key_mod(code, KeyModifiers::CONTROL),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            let want = if code == KeyCode::Right {
                Action::NewTabFocusView
            } else {
                Action::NewTab
            };
            assert_eq!(ctrl, Some(want), "Ctrl+{code:?} focus={focus:?}");
        }
        let (plain_right, _) = map(
            key(KeyCode::Right),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(plain_right, Some(Action::NavExpand));
        // Cmd is not a new-tab modifier (ADR-0016): it behaves like the plain key.
        for focus in [FocusPane::Nav, FocusPane::Viewer] {
            let (plain, _) = map(key(KeyCode::Enter), focus, InputMode::Normal, Chord::None);
            let (cmd, _) = map(
                key_mod(KeyCode::Enter, KeyModifiers::SUPER),
                focus,
                InputMode::Normal,
                Chord::None,
            );
            assert_eq!(cmd, plain, "Cmd+Enter == Enter focus={focus:?}");
            assert_ne!(cmd, Some(Action::NewTab));
        }
        let (cmd_right, _) = map(
            key_mod(KeyCode::Right, KeyModifiers::SUPER),
            FocusPane::Nav,
            InputMode::Normal,
            Chord::None,
        );
        assert_eq!(cmd_right, Some(Action::NavExpand));
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

/// Keeps the generated keymap block in `wiki/product/ui-spec.md` identical to
/// [`BINDINGS`] (the table the `?` help overlay is built from).
#[cfg(test)]
mod docs_sync {
    use super::{BindingScope, help_entries};
    use std::fmt::Write as _;
    use std::path::Path;

    const START: &str = "<!-- keymap:start -->";
    const END: &str = "<!-- keymap:end -->";

    /// Markdown tables per scope, in help-overlay order, default (non-overridden) keys.
    fn keymap_markdown() -> String {
        let mut out = String::from(
            "Generated from `BINDINGS` in `crates/wiki-reader/src/tui/keymap.rs`, the same table the `?` help overlay shows. Edit the table there, then run `UPDATE_DOCS=1 cargo test -p wiki-reader keymap_docs`.\n",
        );
        let mut last: Option<BindingScope> = None;
        for b in help_entries(&std::collections::BTreeMap::new()) {
            if last != Some(b.scope) {
                last = Some(b.scope);
                let _ = write!(
                    out,
                    "\n### {}\n\n| Key | Action |\n|-----|--------|\n",
                    b.scope.title()
                );
            }
            let icon = b
                .action
                .as_ref()
                .and_then(super::binding_icon)
                .map_or_else(String::new, |i| format!(" {i}"));
            let _ = writeln!(out, "| `{}`{icon} | {} |", b.keys, b.help);
        }
        out
    }

    #[test]
    fn help_entries_merge_alternate_keys_into_one_row() {
        let rows = help_entries(&std::collections::BTreeMap::new());
        let keys_for = |scope: BindingScope, help: &str| {
            rows.iter()
                .find(|r| r.scope == scope && r.help == help)
                .map(|r| r.keys.as_str())
        };
        let g = BindingScope::Global;
        let n = BindingScope::Nav;
        let v = BindingScope::Viewer;
        assert_eq!(keys_for(g, "Back"), Some("Alt+← / Alt+b / Backspace"));
        assert_eq!(keys_for(g, "Search"), Some("/ / Ctrl+k"));
        assert_eq!(keys_for(n, "Previous nav row"), Some("↑ / Shift+Tab"));
        assert_eq!(keys_for(n, "Next nav row"), Some("↓ / Tab"));
        assert_eq!(
            keys_for(n, "Open page in a new tab"),
            Some("Ctrl+Enter / Shift+Enter")
        );
        assert_eq!(keys_for(v, "Previous heading"), Some("Alt+Shift+↑ / {"));
        assert_eq!(keys_for(v, "Next heading"), Some("Alt+Shift+↓ / }"));
        assert_eq!(keys_for(v, "Cursor up"), Some("k / ↑"));
        assert_eq!(keys_for(v, "Page up"), Some("PgUp / Shift+Space"));
        assert_eq!(keys_for(v, "Page down"), Some("Space / PgDn"));
        assert_eq!(keys_for(v, "Bottom of page"), Some("End / G"));
        // No two rows in a scope may share an action and help text.
        for (i, a) in rows.iter().enumerate() {
            for b in &rows[i + 1..] {
                assert!(
                    a.action.is_none()
                        || a.scope != b.scope
                        || a.action != b.action
                        || a.help != b.help,
                    "unmerged duplicate rows: {} / {}",
                    a.keys,
                    b.keys
                );
            }
        }
    }

    #[test]
    fn keymap_docs_match_bindings() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wiki/product/ui-spec.md");
        let doc = std::fs::read_to_string(&path).expect("read ui-spec.md");
        let start = doc.find(START).expect("keymap:start marker in ui-spec.md");
        let end = doc.find(END).expect("keymap:end marker in ui-spec.md");
        assert!(start < end, "keymap markers out of order");
        let body_start = start + START.len();
        let want = format!("\n{}", keymap_markdown());
        if std::env::var_os("UPDATE_DOCS").is_some() {
            let updated = format!("{}{}{}", &doc[..body_start], want, &doc[end..]);
            std::fs::write(&path, updated).expect("write ui-spec.md");
            return;
        }
        assert_eq!(
            &doc[body_start..end],
            want,
            "ui-spec keymap block is out of date; run `UPDATE_DOCS=1 cargo test -p wiki-reader keymap_docs`"
        );
    }
}
