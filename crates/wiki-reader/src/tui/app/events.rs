//! Event loop, mouse, and terminal lifecycle.

use std::io::{self, IsTerminal, stdout};
use std::panic;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ratatui::DefaultTerminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyModifiers,
    KeyboardEnhancementFlags, MouseButton, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, enable_raw_mode, supports_keyboard_enhancement,
};

use super::App;
use super::draw::draw;
use crate::tui::action::Action;
use crate::tui::hit::{Hit, HitMap};
#[cfg(feature = "media")]
use crate::tui::images;
use crate::tui::keymap;
use wiki_reader_core::nav::{NavStop, NodeId};

/// Run the TUI until quit. Restores the terminal on every exit path.
///
/// # Errors
///
/// Returns when stdin/stdout are not a terminal, terminal init/draw fails, or the collection
/// cannot be indexed.
pub fn run(root: &Path, config: Option<&Path>) -> io::Result<()> {
    require_tty()?;
    // Validate before entering the terminal so empty collections don't leak raw mode.
    let mut app = App::new_with_config(root, config).map_err(|err| match err {
        wiki_reader_core::Error::EmptyCollection => io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("collection has no markdown pages: {}", root.display()),
        ),
        wiki_reader_core::Error::PageNotFound(key) => io::Error::new(
            io::ErrorKind::NotFound,
            format!("start page not found: {}", key.relative_path.display()),
        ),
        other => io::Error::other(other),
    })?;

    app.publisher = crate::herdr::Publisher::from_env(app.herdr_publish);
    app.search_batching = true;
    // Signals and the parent-death watchdog arm before raw mode (V17 / V22).
    let terminate = Arc::new(AtomicBool::new(false));
    register_terminate_signals(&terminate);
    spawn_parent_watchdog(Arc::clone(&terminate));
    let mut terminal = ratatui::try_init()?;
    // After `try_init`, so this hook is the outermost and decides who may restore the terminal.
    install_panic_hook();
    // Armed after try_init: Drop always restores alt-screen/raw; mouse/keys if enabled.
    let mut guard = TerminalGuard {
        mouse: false,
        keyboard_enhancement: false,
    };
    // The graphics probe reads stdin, so it runs before mouse capture, the keyboard-enhancement
    // query and the first event read (ADR-0004). Config `diagrams = text|source` skips it entirely.
    #[cfg(feature = "media")]
    if images::should_probe(app.diagram_mode_for_probe())
        && app.graphics_allowed()
        && let Some(picker) = images::detect_picker(&images::GraphicsEnv::from_process())
    {
        app.enable_graphics(picker);
    }
    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
    if supports_keyboard_enhancement().unwrap_or(false) {
        // DISAMBIGUATE only — not REPORT_ALL_KEYS_AS_ESCAPE_CODES (typing stays normal).
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        guard.keyboard_enhancement = true;
    }
    let result = run_loop(&mut terminal, &mut app, &mut guard, &terminate);
    // A terminal error (closed tty after SIGHUP) must not lose the session.
    app.flush_session(true);
    drop(guard);
    report_background_panics();
    result
}

/// Refuse a non-interactive stdio pair so we never enter raw mode on a pipe or closed tty.
fn require_tty() -> io::Result<()> {
    if !stdio_is_terminal(io::stdin().is_terminal(), io::stdout().is_terminal()) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "stdin and stdout must be a terminal (not a pipe or file)",
        ));
    }
    Ok(())
}

fn stdio_is_terminal(stdin_tty: bool, stdout_tty: bool) -> bool {
    stdin_tty && stdout_tty
}

fn register_terminate_signals(terminate: &Arc<AtomicBool>) {
    let _ = signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(terminate));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(terminate));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(terminate));
}

/// Exit when the parent shell is gone. If the main thread is stuck inside `event::poll` (V22),
/// restore the terminal and force-exit after a short grace.
///
/// ponytail: `parent_id` polling only; no `PR_SET_PDEATHSIG`. Ceiling: 1 s detection lag; upgrade
/// to a crossterm EOF-as-hangup path if upstream lands one.
fn spawn_parent_watchdog(terminate: Arc<AtomicBool>) {
    #[cfg(unix)]
    {
        let start_ppid = std::os::unix::process::parent_id();
        let _ = std::thread::Builder::new()
            .name("parent-watch".into())
            .spawn(move || {
                let mut orphaned_at: Option<Instant> = None;
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let ppid = std::os::unix::process::parent_id();
                    if ppid == 1 || ppid != start_ppid {
                        terminate.store(true, Ordering::Relaxed);
                        let since = orphaned_at.get_or_insert_with(Instant::now);
                        // Grace for a clean loop exit via `terminate`; then hard-stop a stuck poll.
                        if since.elapsed() >= Duration::from_secs(2) {
                            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
                            let _ = execute!(stdout(), DisableMouseCapture);
                            ratatui::restore();
                            std::process::exit(1);
                        }
                    } else {
                        orphaned_at = None;
                    }
                }
            });
    }
    #[cfg(not(unix))]
    {
        let _ = terminate;
    }
}

/// RAII restore for raw mode / alt screen / mouse capture / kitty keyboard flags.
struct TerminalGuard {
    mouse: bool,
    keyboard_enhancement: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.keyboard_enhancement {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
            self.keyboard_enhancement = false;
        }
        if self.mouse {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        // Drop mouse reports still queued, or the shell prints them as junk after we exit.
        while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
            if event::read().is_err() {
                break;
            }
        }
        ratatui::restore();
    }
}

/// Panics from worker threads, kept to print after the terminal is restored.
static BACKGROUND_PANICS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Most background panic messages kept (a crash loop must not grow this without bound).
const MAX_BACKGROUND_PANICS: usize = 8;

fn record_background_panic(message: String) {
    let mut log = BACKGROUND_PANICS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if log.len() < MAX_BACKGROUND_PANICS {
        log.push(message);
    }
}

fn take_background_panics() -> Vec<String> {
    std::mem::take(
        &mut *BACKGROUND_PANICS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}

/// Print worker panics to stderr once the alternate screen is gone.
fn report_background_panics() {
    for message in take_background_panics() {
        eprintln!("{message}");
    }
}

/// Restore the terminal when the **main** thread panics. A worker panic leaves the screen alone:
/// the worker's own guard turns it into an error shown in the UI, and tearing down raw mode
/// while the main loop keeps running would corrupt the display. Its message is kept for exit.
fn install_panic_hook() {
    let main = std::thread::current().id();
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let current = std::thread::current();
        if current.id() != main {
            let name = current.name().unwrap_or("worker").to_owned();
            record_background_panic(format!("thread '{name}' panicked: {info}"));
            return;
        }
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();
        prev(info);
    }));
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
    terminate: &AtomicBool,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        if terminate.load(Ordering::Relaxed) {
            app.flush_session(true);
            app.quit = true;
        }
        if app.quit {
            app.flush_session(true);
            return Ok(());
        }
        app.poll_watcher();
        app.sync_herdr();
        app.images.poll();
        if app.images.take_diagram_relayout() {
            app.relayout_after_diagram_size();
        }
        app.flush_session(false);
        // Poll fast mid-drag so a held pointer keeps scrolling the View, and while a picture is
        // still decoding so it appears promptly.
        let busy = app.images.has_pending() || app.modal.as_ref().is_some_and(|m| m.busy());
        let idle = std::time::Duration::from_millis(if app.selecting || busy { 40 } else { 250 });
        if event::poll(idle)? {
            // Apply everything already queued before redrawing: a wheel flick or mouse move
            // sends dozens of events, and one full redraw per event lags behind the pointer.
            for _ in 0..MAX_EVENTS_PER_FRAME {
                handle_event(terminal, app, guard, &event::read()?)?;
                if app.quit || !event::poll(std::time::Duration::ZERO)? {
                    break;
                }
            }
        } else if app.selecting {
            app.drag_autoscroll();
        }
    }
}

/// Cap on events applied between redraws, so a flood can't starve the screen.
const MAX_EVENTS_PER_FRAME: usize = 256;

fn handle_event(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
    ev: &Event,
) -> io::Result<()> {
    match ev {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            let (action, next_chord) = keymap::map_with_overrides(
                *key,
                app.focus,
                app.input_mode,
                app.chord,
                Some(&app.key_overrides),
            );
            app.chord = next_chord;
            if let Some(action) = action {
                app.update(action);
            }
        }
        Event::Mouse(mouse) => {
            if let Some(action) = apply_mouse(app, *mouse) {
                app.update(action);
            }
        }
        _ => {}
    }
    // Every path (key, mouse, Help row) asks for the editor the same way, so it always
    // runs with the terminal suspended.
    if app.take_editor_request() {
        suspend_run_editor(terminal, app, guard)?;
    }
    Ok(())
}

/// Leave alt-screen/raw/mouse, run `$EDITOR`, then re-enter without stacking
/// another `try_init` panic hook (`TerminalGuard` semantics).
fn suspend_run_editor(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
) -> io::Result<()> {
    if guard.keyboard_enhancement {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        guard.keyboard_enhancement = false;
    }
    if guard.mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
        guard.mouse = false;
    }
    ratatui::restore();
    app.open_in_editor();
    reenter_terminal(terminal)?;
    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
    if supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        guard.keyboard_enhancement = true;
    }
    Ok(())
}

/// Re-enter raw mode + alt screen without calling `ratatui::try_init` (which
/// would install another panic hook on every editor round-trip).
fn reenter_terminal(terminal: &mut DefaultTerminal) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    *terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) fn apply_mouse(
    app: &mut App,
    mouse: ratatui::crossterm::event::MouseEvent,
) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let hit = app.hit_map.hit_at(mouse.column, mouse.row)?.clone();
            if matches!(hit, Hit::NavDismiss) {
                // Queued clicks can still use the old hit map until the next draw.
                if app.nav_visible {
                    app.update(Action::ToggleNav);
                }
                return None;
            }
            if matches!(hit, Hit::NavDivider) {
                app.nav_dragging = true;
                return None;
            }
            // Shift/Ctrl+click opens in a new tab, like a browser (same as middle-click).
            // Cmd never arrives on mouse events and is not used; Ctrl+click may be stolen
            // as right-click by the host terminal on macOS (documented, not fixable here).
            let new_tab_mods = KeyModifiers::SHIFT | KeyModifiers::CONTROL;
            if mouse.modifiers.intersects(new_tab_mods)
                && matches!(
                    hit,
                    Hit::NavItem(NodeId::Page(_))
                        | Hit::Breadcrumb(_)
                        | Hit::Link(_)
                        | Hit::Prev
                        | Hit::Next
                        | Hit::SearchResult(_)
                )
            {
                app.middle_click_hit(hit);
                return None;
            }
            // Text in the View: press starts a selection. A link is followed on
            // release instead, so a drag can start on link text.
            if matches!(hit, Hit::ViewerLine(_) | Hit::Link(_)) {
                app.pending_link = match hit {
                    Hit::Link(id) => Some(id),
                    _ => None,
                };
                let pos = app.pos_at(mouse.column, mouse.row);
                return Some(Action::SelectStart(pos.line, pos.col));
            }
            // Click focuses the pane before the primary action (K1 / stale-cursor).
            match &hit {
                Hit::NavItem(_) | Hit::NavGroupToggle(_) | Hit::NavSearchRow | Hit::FocusNav => {
                    app.update(Action::FocusNav);
                    // Move nav cursor to the clicked row when applicable.
                    match &hit {
                        Hit::NavItem(id) | Hit::NavGroupToggle(id) => {
                            app.navigator.set_nav_cursor(id.clone());
                        }
                        Hit::NavSearchRow => app.navigator.set_nav_stop(NavStop::Search),
                        _ => {}
                    }
                }
                Hit::Link(_)
                | Hit::Block(_)
                | Hit::ViewerLine(_)
                | Hit::FocusViewer
                | Hit::Prev
                | Hit::Next
                | Hit::Tab(_)
                | Hit::TabClose(_) => {
                    app.update(Action::FocusViewer);
                }
                _ => {}
            }
            // Footer clicks keep focus on that side after the page loads (P2-22).
            match &hit {
                Hit::Prev => {
                    app.sticky_footer = Some(crate::tui::viewer_doc::FocusTarget::FooterPrev);
                }
                Hit::Next => {
                    app.sticky_footer = Some(crate::tui::viewer_doc::FocusTarget::FooterNext);
                }
                _ => {}
            }
            // TabClose carries the index; CloseTab alone would close the active tab.
            if let Hit::TabClose(i) = hit {
                app.close_tab_at(i);
                return None;
            }
            if let Hit::HelpRow(i) = hit {
                app.help_activate(Some(i));
                return None;
            }
            if let Hit::OptionsRow(i) = hit {
                app.options_activate(i);
                return None;
            }
            Some(HitMap::action_for(&hit))
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if app.nav_dragging {
                app.resize_nav_to_column(mouse.column);
            } else if app.selecting {
                app.drag_at = Some((mouse.column, mouse.row));
                let pos = app.pos_at(mouse.column, mouse.row);
                return Some(Action::SelectExtend(pos.line, pos.col));
            }
            None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if app.nav_dragging {
                app.nav_dragging = false;
                app.note_session_change();
            } else if app.selecting {
                return Some(Action::SelectEnd);
            }
            None
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            let hit = app.hit_map.hit_at(mouse.column, mouse.row)?.clone();
            if matches!(hit, Hit::NavDismiss) {
                if app.nav_visible {
                    app.update(Action::ToggleNav);
                }
            } else {
                app.middle_click_hit(hit);
            }
            None
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            // Popups own the wheel: it never falls through to the panes behind.
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
            if app.modal.is_some() {
                use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                // Shift+wheel pans horizontally in modal viewers that support it.
                let code = if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    if up { KeyCode::Left } else { KeyCode::Right }
                } else if up {
                    KeyCode::Up
                } else {
                    KeyCode::Down
                };
                app.update(Action::ModalKey(KeyEvent::new(code, KeyModifiers::NONE)));
                return None;
            }
            if app.search.is_some() {
                app.update(Action::SearchSelectDelta(if up { -1 } else { 1 }));
                return None;
            }
            if app.options.is_some() {
                app.update(if up {
                    Action::OptionsUp
                } else {
                    Action::OptionsDown
                });
                return None;
            }
            if app.help.is_some() {
                app.update(Action::HelpScroll(if up { -1 } else { 1 }));
                return None;
            }
            let over_nav = app.hit_map.entries().iter().any(|(r, h)| {
                mouse.column >= r.x
                    && mouse.column < r.x.saturating_add(r.width)
                    && mouse.row >= r.y
                    && mouse.row < r.y.saturating_add(r.height)
                    && matches!(
                        h,
                        Hit::FocusNav
                            | Hit::NavItem(_)
                            | Hit::NavGroupToggle(_)
                            | Hit::NavSearchRow
                            | Hit::NavDivider
                    )
            });
            if over_nav {
                app.nav_scroll = if up {
                    app.nav_scroll.saturating_sub(1)
                } else {
                    app.nav_scroll.saturating_add(1)
                };
                app.clamp_nav_scroll();
            } else {
                app.scroll = if up {
                    app.scroll.saturating_sub(1)
                } else {
                    app.scroll.saturating_add(1)
                };
                app.clamp_viewer_scroll();
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod panic_log_tests {
    use super::{MAX_BACKGROUND_PANICS, record_background_panic, take_background_panics};

    #[test]
    fn background_panics_are_kept_for_exit_and_bounded() {
        // Other tests never record, so the log starts empty.
        assert!(take_background_panics().is_empty());
        for i in 0..MAX_BACKGROUND_PANICS + 5 {
            record_background_panic(format!("thread 'w' panicked: {i}"));
        }
        let kept = take_background_panics();
        assert_eq!(kept.len(), MAX_BACKGROUND_PANICS, "bounded");
        assert_eq!(kept[0], "thread 'w' panicked: 0", "oldest first");
        assert!(take_background_panics().is_empty(), "taken once");
    }
}

#[cfg(test)]
mod tty_guard_tests {
    use super::stdio_is_terminal;

    #[test]
    fn requires_both_stdin_and_stdout_to_be_terminals() {
        assert!(stdio_is_terminal(true, true));
        assert!(!stdio_is_terminal(false, true));
        assert!(!stdio_is_terminal(true, false));
        assert!(!stdio_is_terminal(false, false));
    }
}
