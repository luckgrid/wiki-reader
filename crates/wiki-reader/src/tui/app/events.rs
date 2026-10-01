//! Event loop, mouse, and terminal lifecycle.

use std::io::{self, stdout};
use std::panic;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::DefaultTerminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};

use super::App;
use super::draw::draw;
use crate::tui::action::Action;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::keymap;
use wiki_reader_core::nav::{NavStop, NodeId};

/// Run the TUI until quit. Restores the terminal on every exit path.
///
/// # Errors
///
/// Returns when terminal init/draw fails or the collection cannot be indexed.
pub fn run(root: &Path, config: Option<&Path>) -> io::Result<()> {
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

    install_panic_hook();
    let mut terminal = ratatui::try_init()?;
    // Armed after try_init: Drop always restores alt-screen/raw; mouse if enabled.
    let mut guard = TerminalGuard { mouse: false };
    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
    let result = run_loop(&mut terminal, &mut app, &mut guard);
    drop(guard);
    result
}

/// RAII restore for raw mode / alt screen / mouse capture.
struct TerminalGuard {
    mouse: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.mouse {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        ratatui::restore();
    }
}

fn install_panic_hook() {
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();
        prev(info);
    }));
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
) -> io::Result<()> {
    let terminate = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&terminate));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminate));
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
        app.flush_session(false);
        // Poll fast mid-drag so a held pointer keeps scrolling the View.
        let idle = std::time::Duration::from_millis(if app.selecting { 40 } else { 250 });
        if event::poll(idle)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let (action, next_chord) = keymap::map_with_overrides(
                        key,
                        app.focus,
                        app.input_mode,
                        app.chord,
                        Some(&app.key_overrides),
                    );
                    app.chord = next_chord;
                    if let Some(Action::OpenInEditor) = action {
                        suspend_run_editor(terminal, app, guard)?;
                    } else if let Some(action) = action {
                        app.update(action);
                    }
                }
                Event::Mouse(mouse) => {
                    if let Some(action) = apply_mouse(app, mouse) {
                        app.update(action);
                    }
                }
                _ => {}
            }
        } else if app.selecting {
            app.drag_autoscroll();
        }
    }
}

/// Leave alt-screen/raw/mouse, run `$EDITOR`, then re-enter without stacking
/// another `try_init` panic hook (`TerminalGuard` semantics).
fn suspend_run_editor(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    guard: &mut TerminalGuard,
) -> io::Result<()> {
    if guard.mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
        guard.mouse = false;
    }
    ratatui::restore();
    app.open_in_editor();
    reenter_terminal(terminal)?;
    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
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
            if matches!(hit, Hit::NavDivider) {
                app.nav_dragging = true;
                return None;
            }
            // Shift+click opens in a new tab, like a browser (same as middle-click).
            if mouse.modifiers.contains(KeyModifiers::SHIFT)
                && matches!(
                    hit,
                    Hit::NavItem(NodeId::Page(_))
                        | Hit::Breadcrumb(_)
                        | Hit::Link(_)
                        | Hit::Prev
                        | Hit::Next
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
            app.middle_click_hit(hit);
            None
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            // Popups own the wheel: it never falls through to the panes behind.
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
            if app.search.is_some() {
                app.update(Action::SearchSelectDelta(if up { -1 } else { 1 }));
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
