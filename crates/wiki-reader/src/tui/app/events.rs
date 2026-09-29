//! Event loop, mouse, and terminal lifecycle.

use std::io::{self, stdout};
use std::panic;
use std::path::Path;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;

use super::App;
use super::draw::draw;
use crate::tui::action::Action;
use crate::tui::hit::{Hit, HitMap};
use crate::tui::keymap;
use wiki_reader_core::nav::NavStop;

/// Run the TUI until quit. Restores the terminal on every exit path.
///
/// # Errors
///
/// Returns when terminal init/draw fails or the collection cannot be indexed.
pub fn run(root: &Path) -> io::Result<()> {
    // Validate before entering the terminal so empty collections don't leak raw mode.
    let mut app = App::new(root).map_err(|err| match err {
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
    let result = run_loop(&mut terminal, &mut app);
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

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        if app.quit {
            return Ok(());
        }
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let (action, next_chord) =
                        keymap::map(key, app.focus, app.input_mode, app.chord);
                    app.chord = next_chord;
                    if let Some(action) = action {
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
        }
    }
}

pub(crate) fn apply_mouse(
    app: &mut App,
    mouse: ratatui::crossterm::event::MouseEvent,
) -> Option<Action> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let hit = app.hit_map.hit_at(mouse.column, mouse.row)?.clone();
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
                Hit::Link(_) | Hit::ViewerLine(_) | Hit::FocusViewer | Hit::Prev | Hit::Next => {
                    app.update(Action::FocusViewer);
                }
                _ => {}
            }
            Some(HitMap::action_for(&hit))
        }
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
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
                    )
            });
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
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
