//! P3-12a terminal-image protocol spike.
//!
//! ```text
//! cargo run -p wiki-reader --example image-protocol
//! cargo run -p wiki-reader --example image-protocol -- --force-kitty
//! WIKI_READER_IMAGE_QUERY_TIMEOUT_MS=500 cargo run -p wiki-reader --example image-protocol
//! ```
//!
//! The picker query deliberately runs after `ratatui::try_init` enters the alternate screen,
//! but before mouse capture, kitty keyboard enhancement, or any event read.

use std::env;
use std::io::stdout;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::{Constraint, Layout, Size};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::{Image, Resize};

struct TerminalGuard {
    mouse: bool,
    keyboard_enhancement: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.keyboard_enhancement {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        if self.mouse {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        ratatui::restore();
    }
}

struct Args {
    image_path: PathBuf,
    force_kitty: bool,
    timeout: Duration,
}

fn args() -> Args {
    let mut image_path = None;
    let mut force_kitty = false;
    for arg in env::args_os().skip(1) {
        if arg == "--force-kitty" {
            force_kitty = true;
        } else {
            image_path = Some(PathBuf::from(arg));
        }
    }
    let timeout_ms = env::var("WIKI_READER_IMAGE_QUERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2_000);
    Args {
        image_path: image_path.unwrap_or_else(|| PathBuf::from("assets/wiki-reader.png")),
        force_kitty,
        timeout: Duration::from_millis(timeout_ms),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args();
    let mut terminal = ratatui::try_init()?;
    let mut guard = TerminalGuard {
        mouse: false,
        keyboard_enhancement: false,
    };

    let query_started = Instant::now();
    let picker_result = Picker::from_query_stdio_with_options(QueryStdioOptions {
        timeout: args.timeout,
        ..QueryStdioOptions::default()
    });
    let query_elapsed = query_started.elapsed();
    let (mut picker, query_error) = match picker_result {
        Ok(picker) => (picker, None),
        Err(error) => (Picker::halfblocks(), Some(error.to_string())),
    };
    let detected_protocol = picker.protocol_type();
    if args.force_kitty {
        picker.set_protocol_type(ProtocolType::Kitty);
    }

    // Match the reader's setup order only after Picker has finished with stdin/stdout.
    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
    if supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        guard.keyboard_enhancement = true;
    }

    let image = image::ImageReader::open(&args.image_path)?.decode()?;
    let area = terminal.size()?;
    let image_size = Size::new(
        area.width.saturating_sub(2).min(80),
        area.height.saturating_sub(8).min(30),
    );
    let protocol = picker.new_protocol(image, image_size, Resize::Fit(None))?;

    loop {
        terminal.draw(|frame| {
            let [details_area, image_area, help_area] = Layout::vertical([
                Constraint::Length(5),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .areas(frame.area());
            let env_value = |name: &str| env::var(name).unwrap_or_else(|_| "<unset>".into());
            let details = vec![
                Line::from(format!(
                    "detected={detected_protocol:?} selected={:?} query={query_elapsed:?}",
                    picker.protocol_type()
                )),
                Line::from(format!(
                    "font={:?} capabilities={:?}",
                    picker.font_size(),
                    picker.capabilities()
                )),
                Line::from(format!(
                    "TERM={} TERM_PROGRAM={} TMUX={}",
                    env_value("TERM"),
                    env_value("TERM_PROGRAM"),
                    env_value("TMUX")
                )),
                Line::from(format!(
                    "HERDR_ENV={} force_kitty={} error={}",
                    env_value("HERDR_ENV"),
                    args.force_kitty,
                    query_error.as_deref().unwrap_or("none")
                )),
            ];
            frame.render_widget(Paragraph::new(details), details_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(args.image_path.display().to_string());
            let inner = block.inner(image_area);
            frame.render_widget(block, image_area);
            frame.render_widget(Image::new(&protocol).allow_clipping(true), inner);
            frame.render_widget(Paragraph::new("q / Esc / Ctrl+C: quit"), help_area);
        })?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            let ctrl_c = key
                .modifiers
                .contains(ratatui::crossterm::event::KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('c');
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) || ctrl_c {
                return Ok(());
            }
        }
    }
}
