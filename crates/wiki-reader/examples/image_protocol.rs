//! P3-12a terminal-image protocol spike.
//!
//! ```text
//! cargo run -p wiki-reader-tui --example image-protocol
//! cargo run -p wiki-reader-tui --example image-protocol -- --force-kitty
//! cargo run -p wiki-reader-tui --example image-protocol -- --force-iterm2
//! WIKI_READER_IMAGE_QUERY_TIMEOUT_MS=500 cargo run -p wiki-reader-tui --example image-protocol
//!
//! `j`/`k` simulate a tall image becoming top-clipped, `o` overlays `Clear` plus a popup, and
//! `n` replaces the image. This deliberately exercises `StatefulImage` with `Resize::Crop`.
//! ```
//!
//! The picker query deliberately runs after `ratatui::try_init` enters the alternate screen,
//! but before mouse capture, kitty keyboard enhancement, or any event read.

use std::env;
use std::io::stdout;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{CropOptions, Resize, StatefulImage};

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
    force_protocol: Option<ProtocolType>,
    timeout: Duration,
}

struct SpikeApp {
    protocol: StatefulProtocol,
    image: image::DynamicImage,
    alternate_image: image::DynamicImage,
    alternate: bool,
    crop_top: bool,
    overlay_open: bool,
    top_clip: u16,
    encoding_status: String,
    details: Vec<String>,
    image_title: String,
}

impl SpikeApp {
    fn draw(&mut self, frame: &mut Frame<'_>) {
        let [details_area, image_area, help_area] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let mut details = self
            .details
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect::<Vec<_>>();
        details.push(Line::from(format!(
            "top_clip={} crop_top={} overlay={} alternate={}",
            self.top_clip, self.crop_top, self.overlay_open, self.alternate
        )));
        details.push(Line::from(format!("encoding={}", self.encoding_status)));
        frame.render_widget(Paragraph::new(details), details_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.image_title.as_str());
        let inner = block.inner(image_area);
        frame.render_widget(block, image_area);
        let visible = Rect {
            height: inner.height.saturating_sub(self.top_clip),
            ..inner
        };
        let resize = if self.crop_top {
            Resize::Crop(Some(CropOptions {
                clip_top: true,
                clip_left: false,
            }))
        } else {
            Resize::Fit(None)
        };
        frame.render_stateful_widget(
            StatefulImage::new().resize(resize),
            visible,
            &mut self.protocol,
        );
        if self.overlay_open {
            let popup = centered_rect(60, 7, frame.area());
            frame.render_widget(Clear, popup);
            frame.render_widget(
                Paragraph::new("Popup over image\n\nPress o to close")
                    .block(Block::bordered().title("Clear + block")),
                popup,
            );
        }
        frame.render_widget(
            Paragraph::new("j/k: top clip  c: Fit/Crop  o: overlay  n: swap  q/Esc/Ctrl+C: quit"),
            help_area,
        );
    }

    fn handle_key(&mut self, key: KeyEvent, picker: &Picker) -> bool {
        let ctrl_c = key
            .modifiers
            .contains(ratatui::crossterm::event::KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c');
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) || ctrl_c {
            return true;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.top_clip = self.top_clip.saturating_add(1).min(10);
                self.crop_top = true;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.top_clip = self.top_clip.saturating_sub(1);
            }
            KeyCode::Char('c') => self.crop_top = !self.crop_top,
            KeyCode::Char('o') => self.overlay_open = !self.overlay_open,
            KeyCode::Char('n') => {
                self.alternate = !self.alternate;
                let next = if self.alternate {
                    self.alternate_image.clone()
                } else {
                    self.image.clone()
                };
                self.protocol = picker.new_resize_protocol(next);
            }
            _ => {}
        }
        false
    }

    fn update_encoding_status(&mut self) {
        if let Some(result) = self.protocol.last_encoding_result() {
            self.encoding_status = match result {
                Ok(()) => "ok".to_string(),
                Err(error) => error.to_string(),
            };
        }
    }
}

fn args() -> Args {
    let mut image_path = None;
    let mut force_protocol = None;
    for arg in env::args_os().skip(1) {
        if arg == "--force-kitty" {
            force_protocol = Some(ProtocolType::Kitty);
        } else if arg == "--force-iterm2" {
            force_protocol = Some(ProtocolType::Iterm2);
        } else {
            image_path = Some(PathBuf::from(arg));
        }
    }
    let timeout_ms = env::var("WIKI_READER_IMAGE_QUERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(250);
    Args {
        image_path: image_path.unwrap_or_else(|| PathBuf::from("assets/wiki-reader.png")),
        force_protocol,
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
    if let Some(protocol) = args.force_protocol {
        picker.set_protocol_type(protocol);
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
    let alternate_image = image.fliph();
    let env_value = |name: &str| env::var(name).unwrap_or_else(|_| "<unset>".into());
    let mut app = SpikeApp {
        protocol: picker.new_resize_protocol(image.clone()),
        image,
        alternate_image,
        alternate: false,
        crop_top: false,
        overlay_open: false,
        top_clip: 0,
        encoding_status: "pending".to_string(),
        details: vec![
            format!(
                "detected={detected_protocol:?} selected={:?} query={query_elapsed:?}",
                picker.protocol_type()
            ),
            format!(
                "font={:?} capabilities={:?}",
                picker.font_size(),
                picker.capabilities()
            ),
            format!(
                "TERM={} TERM_PROGRAM={} TMUX={}",
                env_value("TERM"),
                env_value("TERM_PROGRAM"),
                env_value("TMUX")
            ),
            format!(
                "HERDR_ENV={} force={:?} error={}",
                env_value("HERDR_ENV"),
                args.force_protocol,
                query_error.as_deref().unwrap_or("none")
            ),
        ],
        image_title: args.image_path.display().to_string(),
    };

    loop {
        terminal.draw(|frame| app.draw(frame))?;
        app.update_encoding_status();
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && app.handle_key(key, &picker)
        {
            return Ok(());
        }
    }
}

fn centered_rect(width_percent: u16, height: u16, area: Rect) -> Rect {
    let width = area.width.saturating_mul(width_percent).saturating_div(100);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height: height.min(area.height),
    }
}
