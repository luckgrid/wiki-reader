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
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::execute;

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    writeln!(
        out,
        "keylog: press keys/click/wheel (q or Esc to quit)\r"
    )?;
    out.flush()?;

    let result = (|| -> std::io::Result<()> {
        loop {
            if !event::poll(std::time::Duration::from_millis(250))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    writeln!(out, "Key: {key:?}\r")?;
                    out.flush()?;
                    if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
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
    })();

    let _ = execute!(out, DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();
    result
}
