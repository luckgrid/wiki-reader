//! App shell: owns [`Navigator`]; placeholder regions until P1-07+.

use std::io;
use std::path::Path;

use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use wiki_reader_core::Index;
use wiki_reader_core::nav::Navigator;
use wiki_reader_core::provider::FsProvider;

#[cfg(test)]
use ratatui::Terminal;

/// Run the TUI until quit. Restores the terminal on every exit path.
///
/// # Errors
///
/// Returns when terminal init/draw fails or the collection cannot be indexed.
pub fn run(root: &Path) -> io::Result<()> {
    let provider = FsProvider::open(root).map_err(io::Error::other)?;
    let index = Index::build(&provider).map_err(io::Error::other)?;
    let navigator = Navigator::new(index, None);
    let mut terminal = ratatui::try_init()?;
    let result = run_loop(&mut terminal, root, &navigator);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut DefaultTerminal, root: &Path, navigator: &Navigator) -> io::Result<()> {
    let root_display = root.display().to_string();
    let page = navigator
        .tab()
        .current()
        .page
        .relative_path
        .display()
        .to_string();
    loop {
        terminal.draw(|frame| draw(frame, &root_display, &page))?;
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn draw(frame: &mut Frame<'_>, root: &str, page: &str) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!("wiki-reader · {root}"))
            .block(Block::default().borders(Borders::ALL).title("Header")),
        chunks[0],
    );

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    frame.render_widget(
        Paragraph::new("side nav").block(Block::default().borders(Borders::ALL).title("Side nav")),
        mid[0],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "viewer\n\nroot: {root}\npage: {page}\n\nq / Esc to quit"
        ))
        .block(Block::default().borders(Borders::ALL).title("Viewer")),
        mid[1],
    );

    frame.render_widget(
        Paragraph::new(format!("STATUS · {root}"))
            .block(Block::default().borders(Borders::ALL).title("Status")),
        chunks[2],
    );
}

/// Render the placeholder UI into a buffer (for tests).
#[cfg(test)]
pub fn render_placeholder(root: &str, width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, root, "README.md"))
        .expect("draw");
    format!("{:?}", terminal.backend().buffer())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_shows_root_path() {
        let out = render_placeholder("/tmp/example-wiki", 80, 24);
        assert!(
            out.contains("/tmp/example-wiki"),
            "expected root path in buffer, got: {out}"
        );
    }
}
