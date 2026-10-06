//! P3-12a Mermaid-to-terminal-image spike (kept for manual checks).
//!
//! Run from the repository root:
//!
//! ```text
//! cargo run -p wiki-reader-tui --example mermaid-image -- fixtures/mermaid/common-types.md
//! cargo run -p wiki-reader-tui --example mermaid-image -- --render-only --output-dir /tmp/mermaid fixtures/mermaid/common-types.md
//! ```

use std::env;
use std::fs;
use std::io::{IsTerminal, stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::DynamicImage;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::{Constraint, Layout, Size};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::picker::Picker;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::{Image, Resize};
use wiki_reader_render::{DiagramPalette, render_mermaid};

const DEFAULT_QUERY_TIMEOUT_MS: u64 = 250;

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
    paths: Vec<PathBuf>,
    output_dir: Option<PathBuf>,
    render_only: bool,
    timeout: Duration,
}

struct DiagramSource {
    label: String,
    source: String,
}

struct RenderedDiagram {
    label: String,
    image: DynamicImage,
    elapsed: Duration,
}

fn args() -> Args {
    let mut paths = Vec::new();
    let mut output_dir = None;
    let mut render_only = false;
    let mut raw_args = env::args_os().skip(1);
    while let Some(arg) = raw_args.next() {
        if arg == "--render-only" {
            render_only = true;
        } else if arg == "--output-dir" {
            output_dir = Some(PathBuf::from(
                raw_args
                    .next()
                    .expect("--output-dir requires a directory path"),
            ));
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    if paths.is_empty() {
        paths.extend([
            PathBuf::from("fixtures/elements/README.md"),
            PathBuf::from("fixtures/mermaid/common-types.md"),
            PathBuf::from("wiki/architecture/rendering.md"),
        ]);
    }
    let timeout_ms = env::var("WIKI_READER_IMAGE_QUERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_QUERY_TIMEOUT_MS);
    Args {
        paths,
        output_dir,
        render_only,
        timeout: Duration::from_millis(timeout_ms),
    }
}

fn extract_diagrams(path: &Path) -> Result<Vec<DiagramSource>, Box<dyn std::error::Error>> {
    let input = fs::read_to_string(path)?;
    if path.extension().is_some_and(|extension| extension == "mmd") {
        return Ok(vec![DiagramSource {
            label: path.display().to_string(),
            source: input,
        }]);
    }

    let mut diagrams = Vec::new();
    let mut current = None;
    let mut start_line = 0;
    for (index, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if current.is_none()
            && (trimmed.eq_ignore_ascii_case("```mermaid")
                || trimmed.eq_ignore_ascii_case("```mmd"))
        {
            current = Some(String::new());
            start_line = index + 2;
        } else if trimmed == "```"
            && let Some(source) = current.take()
        {
            diagrams.push(DiagramSource {
                label: format!("{}:{start_line}", path.display()),
                source,
            });
        } else if let Some(source) = current.as_mut() {
            source.push_str(line);
            source.push('\n');
        }
    }
    if current.is_some() {
        return Err(format!("unclosed Mermaid fence in {}", path.display()).into());
    }
    Ok(diagrams)
}

fn render_diagram(diagram: DiagramSource) -> Result<RenderedDiagram, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let raster = render_mermaid(&diagram.source, &DiagramPalette::default())?;
    let elapsed = started.elapsed();
    Ok(RenderedDiagram {
        label: diagram.label,
        image: DynamicImage::ImageRgba8(raster.image),
        elapsed,
    })
}

fn load_cases(
    paths: &[PathBuf],
    output_dir: Option<&Path>,
) -> Result<Vec<RenderedDiagram>, Box<dyn std::error::Error>> {
    if let Some(output_dir) = output_dir {
        fs::create_dir_all(output_dir)?;
    }
    let mut rendered = Vec::new();
    let mut failures = 0;
    for path in paths {
        for diagram in extract_diagrams(path)? {
            match render_diagram(diagram) {
                Ok(result) => {
                    println!(
                        "ok: {} — {}x{}, {:?}",
                        result.label,
                        result.image.width(),
                        result.image.height(),
                        result.elapsed
                    );
                    if let Some(output_dir) = output_dir {
                        let stem = format!("diagram-{:02}", rendered.len() + 1);
                        result.image.save(output_dir.join(format!("{stem}.png")))?;
                    }
                    rendered.push(result);
                }
                Err(error) => {
                    failures += 1;
                    eprintln!("fallback: {}: {error}", path.display());
                }
            }
        }
    }
    println!("rendered={} fallback={failures}", rendered.len());
    if rendered.is_empty() {
        return Err("no Mermaid block rendered successfully".into());
    }
    Ok(rendered)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args();
    let cases = load_cases(&args.paths, args.output_dir.as_deref())?;
    if args.render_only || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Ok(());
    }

    let mut terminal = ratatui::try_init()?;
    let mut guard = TerminalGuard {
        mouse: false,
        keyboard_enhancement: false,
    };
    let query_started = Instant::now();
    let picker = Picker::from_query_stdio_with_options(QueryStdioOptions {
        timeout: args.timeout,
        ..QueryStdioOptions::default()
    })?;
    let query_elapsed = query_started.elapsed();

    execute!(stdout(), EnableMouseCapture)?;
    guard.mouse = true;
    if supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        guard.keyboard_enhancement = true;
    }

    let mut selected = 0;
    let area = terminal.size()?;
    let image_size = Size::new(area.width.saturating_sub(2), area.height.saturating_sub(5));
    let mut protocol =
        picker.new_protocol(cases[selected].image.clone(), image_size, Resize::Fit(None))?;

    loop {
        terminal.draw(|frame| {
            let [details_area, image_area, help_area] = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .areas(frame.area());
            let case = &cases[selected];
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("{}/{} {}", selected + 1, cases.len(), case.label)),
                    Line::from(format!(
                        "protocol={:?} query={query_elapsed:?} raster={}x{} render={:?}",
                        picker.protocol_type(),
                        case.image.width(),
                        case.image.height(),
                        case.elapsed,
                    )),
                ]),
                details_area,
            );
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Mermaid image");
            let inner = block.inner(image_area);
            frame.render_widget(block, image_area);
            frame.render_widget(Image::new(&protocol).allow_clipping(true), inner);
            frame.render_widget(
                Paragraph::new("n/Right: next  p/Left: previous  q/Esc/Ctrl+C: quit"),
                help_area,
            );
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
            let next = matches!(key.code, KeyCode::Char('n') | KeyCode::Right);
            let previous = matches!(key.code, KeyCode::Char('p') | KeyCode::Left);
            if next || previous {
                selected = if next {
                    (selected + 1) % cases.len()
                } else {
                    selected.checked_sub(1).unwrap_or(cases.len() - 1)
                };
                let area = terminal.size()?;
                let image_size =
                    Size::new(area.width.saturating_sub(2), area.height.saturating_sub(5));
                protocol = picker.new_protocol(
                    cases[selected].image.clone(),
                    image_size,
                    Resize::Fit(None),
                )?;
            }
        }
    }
}
