//! Live key/mouse event logger for herdr keymap verification (P1-G / P1-S1).
//!
//! ```text
//! cargo run -p wiki-reader --example keylog
//! ```
//!
//! Prints every crossterm `KeyEvent` / `MouseEvent` in raw mode with mouse
//! capture on. Tick the checklist in `wiki/roadmap/spikes/p1-s1-herdr-input.md`.

use std::io::{Write, stdout};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(out, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TermGuard;
    writeln!(
        out,
        "keylog: press keys/click/wheel (q or Ctrl+C to quit)\r"
    )?;
    out.flush()?;

    loop {
        if !event::poll(std::time::Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                writeln!(out, "Key: {key:?}\r")?;
                out.flush()?;
                let ctrl_c = key
                    .modifiers
                    .contains(ratatui::crossterm::event::KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c');
                if matches!(key.code, KeyCode::Char('q')) || ctrl_c {
                    return Ok(());
                }
            }
            Event::Key(key) => {
                writeln!(out, "Key(other): {key:?}\r")?;
                out.flush()?;
            }
            Event::Mouse(mouse) => {
                writeln!(out, "Mouse: {mouse:?}\r")?;
                out.flush()?;
            }
            Event::Resize(w, h) => {
                writeln!(out, "Resize: {w}x{h}\r")?;
                out.flush()?;
            }
            other => {
                writeln!(out, "Other: {other:?}\r")?;
                out.flush()?;
            }
        }
    }
}
