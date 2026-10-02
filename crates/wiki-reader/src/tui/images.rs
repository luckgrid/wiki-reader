//! Terminal graphics: startup probe (ADR-0004), background decode, and drawing image slots
//! (ADR-0017). The text placeholder always works; pictures are an upgrade.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits, RgbaImage};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{CropOptions, Resize, StatefulImage};
use wiki_reader_core::images::MAX_IMAGE_PIXELS;
use wiki_reader_render::ImageSlot;

use crate::tui::theme::Theme;

/// Query timeout. Answers arrive in single-digit milliseconds; the timeout only bounds a
/// terminal that never answers (see the P3-S1 spike for why unknown terminals are not probed).
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_millis(250);
/// Diagnostic override for the probe timeout, in milliseconds.
const QUERY_TIMEOUT_ENV: &str = "WIKI_READER_IMAGE_QUERY_TIMEOUT_MS";
/// Decode allocation cap: four bytes per pixel at the pixel cap, doubled for the decoder's copy.
const MAX_DECODE_ALLOC: u64 = MAX_IMAGE_PIXELS * 4 * 2;

// ── Detection ────────────────────────────────────────────────────────────────────────────────

/// Environment facts the probe policy reads.
#[derive(Debug, Clone, Default)]
pub struct GraphicsEnv {
    pub tmux: bool,
    pub herdr: bool,
    pub term_program: Option<String>,
    pub term: Option<String>,
    pub kitty_window: bool,
}

impl GraphicsEnv {
    #[must_use]
    pub fn from_process() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        Self {
            tmux: var("TMUX").is_some(),
            herdr: var("HERDR_ENV").as_deref() == Some("1"),
            term_program: var("TERM_PROGRAM"),
            term: var("TERM"),
            kitty_window: var("KITTY_WINDOW_ID").is_some(),
        }
    }
}

/// Which probe result is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accept {
    /// Under herdr only a confirmed Kitty answer enables images; never Sixel or iTerm2.
    KittyOnly,
    /// iTerm2 answers the Kitty query without rendering Kitty; select its own protocol.
    ForceIterm2,
    /// Known graphics terminal: trust a Kitty or iTerm2 answer, anything else is text.
    KittyOrIterm2,
}

/// ADR-0004 detection rule: `None` means do not probe and use the text tier.
///
/// A probe can leave a worker blocked on stdin when the terminal never answers, so unknown
/// terminals (including Terminal.app) and tmux are never probed.
#[must_use]
pub fn probe_plan(env: &GraphicsEnv) -> Option<Accept> {
    if env.tmux {
        return None;
    }
    if env.herdr {
        return Some(Accept::KittyOnly);
    }
    match env.term_program.as_deref() {
        Some("iTerm.app") => return Some(Accept::ForceIterm2),
        Some("ghostty" | "WezTerm") => return Some(Accept::KittyOrIterm2),
        _ => {}
    }
    (env.kitty_window || env.term.as_deref() == Some("xterm-kitty"))
        .then_some(Accept::KittyOrIterm2)
}

/// The protocol to use for a probe answer, or `None` for the text tier.
#[must_use]
pub fn accepted_protocol(
    found: ProtocolType,
    cell_px: (u16, u16),
    accept: Accept,
) -> Option<ProtocolType> {
    if cell_px.0 == 0 || cell_px.1 == 0 {
        return None;
    }
    match (accept, found) {
        (Accept::KittyOnly, ProtocolType::Kitty) => Some(ProtocolType::Kitty),
        (Accept::KittyOrIterm2, ProtocolType::Kitty | ProtocolType::Iterm2) => Some(found),
        (Accept::ForceIterm2, _) => Some(ProtocolType::Iterm2),
        _ => None,
    }
}

/// Apply the accept rule to a probe answer.
#[must_use]
pub fn accept_picker(mut picker: Picker, accept: Accept) -> Option<Picker> {
    let font = picker.font_size();
    let protocol = accepted_protocol(picker.protocol_type(), (font.width, font.height), accept)?;
    picker.set_protocol_type(protocol);
    Some(picker)
}

/// Probe the terminal if policy allows. Must run after `ratatui::try_init` and before mouse
/// capture, keyboard enhancement and the first event read (both consume stdin).
#[must_use]
pub fn detect_picker(env: &GraphicsEnv) -> Option<Picker> {
    let accept = probe_plan(env)?;
    let timeout = std::env::var(QUERY_TIMEOUT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .map_or(DEFAULT_QUERY_TIMEOUT, Duration::from_millis);
    let picker = Picker::from_query_stdio_with_options(QueryStdioOptions {
        timeout,
        ..QueryStdioOptions::default()
    })
    .ok()?;
    accept_picker(picker, accept)
}

// ── Slot geometry ────────────────────────────────────────────────────────────────────────────

/// Which edge of a slot is scrolled out of the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clip {
    None,
    /// The top rows are hidden: keep the bottom of the picture.
    Top,
    /// The bottom rows are hidden: keep the top of the picture.
    Bottom,
}

/// The on-screen rectangle for `slot` and which edge is cropped. `None` when the slot is fully
/// off screen, or hidden at both edges (taller than the viewer): the placeholder row shows.
///
/// `text_x`/`top_y` is the first text cell, `viewer_rows` the visible rows, `scroll` the first
/// visible display row.
#[must_use]
pub fn slot_visible(
    slot: &ImageSlot,
    scroll: u32,
    text_x: u16,
    top_y: u16,
    viewer_rows: u16,
) -> Option<(Rect, Clip)> {
    let top = i64::from(slot.line) - i64::from(scroll);
    let bottom = top + i64::from(slot.rows);
    let rows = i64::from(viewer_rows);
    let (clipped_top, clipped_bottom) = (top < 0, bottom > rows);
    if bottom <= 0 || top >= rows || (clipped_top && clipped_bottom) {
        return None;
    }
    let vis_top = top.max(0);
    let vis_bottom = bottom.min(rows);
    let clip = match (clipped_top, clipped_bottom) {
        (true, _) => Clip::Top,
        (_, true) => Clip::Bottom,
        _ => Clip::None,
    };
    let y = top_y.saturating_add(u16::try_from(vis_top).ok()?);
    let height = u16::try_from(vis_bottom - vis_top).ok()?;
    Some((
        Rect {
            x: text_x,
            y,
            width: slot.cols,
            height,
        },
        clip,
    ))
}

// ── Background decode ────────────────────────────────────────────────────────────────────────

/// Identity of a prepared picture. The file stamp keeps an edited image from a stale cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SlotKey {
    path: PathBuf,
    stamp: (u64, u64),
    cols: u16,
    rows: u16,
}

impl SlotKey {
    fn of(slot: &ImageSlot) -> Self {
        Self {
            path: slot.path.clone(),
            stamp: slot.stamp,
            cols: slot.cols,
            rows: slot.rows,
        }
    }
}

struct Job {
    key: SlotKey,
}

struct Done {
    key: SlotKey,
    result: Result<Box<StatefulProtocol>, String>,
}

enum Entry {
    Pending,
    Ready(Box<StatefulProtocol>),
    Failed(String),
}

/// Decode `key.path`, scale it to the slot's pixel size (preserving aspect, never above the
/// slot) and pad to exactly that size, so a later top/bottom crop keeps the scale.
fn prepare(key: &SlotKey, font: (u16, u16)) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::open(&key.path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| e.to_string())?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > MAX_IMAGE_PIXELS {
        return Err("image too large".into());
    }
    let target_w = u32::from(key.cols) * u32::from(font.0);
    let target_h = u32::from(key.rows) * u32::from(font.1);
    let scaled = decoded.resize(target_w, target_h, FilterType::Triangle);
    let mut canvas = RgbaImage::new(target_w, target_h);
    image::imageops::overlay(&mut canvas, &scaled.to_rgba8(), 0, 0);
    Ok(DynamicImage::ImageRgba8(canvas))
}

/// Owns the terminal's graphics capability, the decode worker and the prepared pictures.
pub struct ImageManager {
    picker: Option<Picker>,
    jobs: Option<Sender<Job>>,
    done: Option<Receiver<Done>>,
    entries: HashMap<SlotKey, Entry>,
}

impl ImageManager {
    /// No graphics protocol: every image stays a text placeholder.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            picker: None,
            jobs: None,
            done: None,
            entries: HashMap::new(),
        }
    }

    /// Start the decode worker for a confirmed protocol.
    #[must_use]
    pub fn enabled(picker: Picker) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done_rx) = mpsc::channel::<Done>();
        let worker_picker = picker.clone();
        let font = picker.font_size();
        std::thread::spawn(move || {
            // One image at a time, in the order slots were first drawn.
            while let Ok(Job { key }) = job_rx.recv() {
                let result = prepare(&key, (font.width, font.height))
                    .map(|img| Box::new(worker_picker.new_resize_protocol(img)));
                if done_tx.send(Done { key, result }).is_err() {
                    break;
                }
            }
        });
        Self {
            picker: Some(picker),
            jobs: Some(job_tx),
            done: Some(done_rx),
            entries: HashMap::new(),
        }
    }

    /// Cell size in pixels when a protocol is active (the renderer sizes slots from it).
    #[must_use]
    pub fn cell_px(&self) -> Option<(u16, u16)> {
        self.picker.as_ref().map(|p| {
            let font = p.font_size();
            (font.width, font.height)
        })
    }

    /// Drain finished decodes. True when a picture arrived, so the frame needs a redraw.
    pub fn poll(&mut self) -> bool {
        let Some(done) = &self.done else {
            return false;
        };
        let mut changed = false;
        while let Ok(Done { key, result }) = done.try_recv() {
            // A key dropped by `retain_for` while decoding is not resurrected.
            if let Some(entry) = self.entries.get_mut(&key) {
                *entry = match result {
                    Ok(protocol) => Entry::Ready(protocol),
                    Err(reason) => Entry::Failed(reason),
                };
                changed = true;
            }
        }
        changed
    }

    /// True while a decode is queued or running (the loop polls faster to show it promptly).
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.entries.values().any(|e| matches!(e, Entry::Pending))
    }

    /// Keep only the pictures `slots` need; everything else is released.
    pub fn retain_for(&mut self, slots: &[ImageSlot]) {
        let keep: Vec<SlotKey> = slots.iter().map(SlotKey::of).collect();
        self.entries.retain(|key, _| keep.contains(key));
    }

    /// Queue a decode for `slot` if it has no entry yet.
    fn ensure(&mut self, slot: &ImageSlot) {
        let key = SlotKey::of(slot);
        if self.entries.contains_key(&key) {
            return;
        }
        let queued = self
            .jobs
            .as_ref()
            .is_some_and(|tx| tx.send(Job { key: key.clone() }).is_ok());
        self.entries.insert(
            key,
            if queued {
                Entry::Pending
            } else {
                Entry::Failed("image worker stopped".into())
            },
        );
    }

    /// Draw pictures for `slots` over their reserved rows. Call after the viewer text and before
    /// popups, so a popup's `Clear` covers a picture. A slot that is not ready (or cannot be
    /// shown whole or cropped at one edge) gets its `[image: alt]` placeholder on its first row.
    pub fn draw(
        &mut self,
        frame: &mut Frame<'_>,
        slots: &[ImageSlot],
        scroll: u32,
        geom: crate::tui::regions::viewer::ViewerGeom,
        theme: &Theme,
    ) {
        if self.picker.is_none() {
            return;
        }
        for slot in slots {
            self.ensure(slot);
            let visible = slot_visible(slot, scroll, geom.text_x, geom.top_y, geom.rows);
            let key = SlotKey::of(slot);
            match (self.entries.get_mut(&key), visible) {
                (Some(Entry::Ready(protocol)), Some((rect, clip))) => {
                    let rect = rect.intersection(frame.area());
                    let resize = Resize::Crop(Some(CropOptions {
                        clip_top: clip == Clip::Top,
                        clip_left: false,
                    }));
                    frame.render_stateful_widget(
                        StatefulImage::new().resize(resize),
                        rect,
                        protocol.as_mut(),
                    );
                }
                (entry, _) => {
                    let note = match entry {
                        Some(Entry::Failed(reason)) => Some(reason.as_str()),
                        _ => None,
                    };
                    draw_placeholder(frame, slot, scroll, geom, note, theme);
                }
            }
        }
    }
}

/// `[image: alt]` (and why, when decoding failed) on the first visible row of the slot.
fn draw_placeholder(
    frame: &mut Frame<'_>,
    slot: &ImageSlot,
    scroll: u32,
    geom: crate::tui::regions::viewer::ViewerGeom,
    failure: Option<&str>,
    theme: &Theme,
) {
    let top = i64::from(slot.line) - i64::from(scroll);
    let row = top.max(0);
    let rows = i64::from(geom.rows);
    if top + i64::from(slot.rows) <= 0 || row >= rows {
        return;
    }
    let Ok(dy) = u16::try_from(row) else {
        return;
    };
    let label = slot.alt.trim();
    let mut text = if label.is_empty() {
        "[image]".to_owned()
    } else {
        format!("[image: {label}]")
    };
    if let Some(reason) = failure {
        text.push_str(" — ");
        text.push_str(reason);
    }
    let x = geom.text_x;
    let width = u16::try_from(text.chars().count())
        .unwrap_or(u16::MAX)
        .min(frame.area().right().saturating_sub(x));
    if width == 0 {
        return;
    }
    let area = Rect {
        x,
        y: geom.top_y.saturating_add(dy),
        width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            text,
            theme.style_kind(wiki_reader_render::StyleKind::ImagePlaceholder),
        )),
        area.intersection(frame.area()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(line: u32, rows: u16) -> ImageSlot {
        ImageSlot {
            line,
            rows,
            cols: 20,
            path: PathBuf::from("a.png"),
            alt: "alt".into(),
            stamp: (1, 1),
        }
    }

    /// Viewer text at x=2, y=1, 10 visible rows.
    fn vis(slot: &ImageSlot, scroll: u32) -> Option<(Rect, Clip)> {
        slot_visible(slot, scroll, 2, 1, 10)
    }

    fn rect(y: u16, height: u16) -> Rect {
        Rect {
            x: 2,
            y,
            width: 20,
            height,
        }
    }

    #[test]
    fn fully_visible_slot_maps_to_screen_rows() {
        assert_eq!(vis(&slot(3, 4), 0), Some((rect(4, 4), Clip::None)));
        assert_eq!(vis(&slot(13, 4), 10), Some((rect(4, 4), Clip::None)));
    }

    #[test]
    fn slot_scrolled_off_the_top_keeps_its_bottom() {
        // Slot rows 3..7, scroll 5: rows 5..7 remain ⇒ 2 rows at the top of the viewer.
        assert_eq!(vis(&slot(3, 4), 5), Some((rect(1, 2), Clip::Top)));
        // Exactly one row left.
        assert_eq!(vis(&slot(3, 4), 6), Some((rect(1, 1), Clip::Top)));
    }

    #[test]
    fn slot_entering_from_the_bottom_keeps_its_top() {
        // Viewer shows rows 0..10; slot rows 8..12 ⇒ 2 rows at the bottom.
        assert_eq!(vis(&slot(8, 4), 0), Some((rect(9, 2), Clip::Bottom)));
    }

    #[test]
    fn slot_outside_the_viewer_is_hidden() {
        assert_eq!(vis(&slot(3, 4), 7), None, "scrolled past");
        assert_eq!(vis(&slot(10, 4), 0), None, "starts below the last row");
        assert_eq!(vis(&slot(0, 4), 4), None, "ends exactly at the top edge");
    }

    #[test]
    fn slot_taller_than_the_viewer_cropped_both_ends_is_hidden() {
        assert_eq!(vis(&slot(0, 30), 5), None);
        // Cropped at only one end is still drawn.
        assert_eq!(vis(&slot(0, 30), 0), Some((rect(1, 10), Clip::Bottom)));
    }

    fn env() -> GraphicsEnv {
        GraphicsEnv::default()
    }

    #[test]
    fn tmux_is_never_probed() {
        let tmux = GraphicsEnv {
            tmux: true,
            herdr: true,
            term_program: Some("ghostty".into()),
            ..env()
        };
        assert_eq!(probe_plan(&tmux), None);
    }

    #[test]
    fn herdr_accepts_only_kitty() {
        let herdr = GraphicsEnv {
            herdr: true,
            term_program: Some("iTerm.app".into()),
            ..env()
        };
        assert_eq!(probe_plan(&herdr), Some(Accept::KittyOnly));
    }

    #[test]
    fn iterm2_is_selected_explicitly() {
        let iterm = GraphicsEnv {
            term_program: Some("iTerm.app".into()),
            ..env()
        };
        assert_eq!(probe_plan(&iterm), Some(Accept::ForceIterm2));
    }

    #[test]
    fn known_graphics_terminals_are_probed() {
        for e in [
            GraphicsEnv {
                term_program: Some("ghostty".into()),
                ..env()
            },
            GraphicsEnv {
                term_program: Some("WezTerm".into()),
                ..env()
            },
            GraphicsEnv {
                kitty_window: true,
                ..env()
            },
            GraphicsEnv {
                term: Some("xterm-kitty".into()),
                ..env()
            },
        ] {
            assert_eq!(probe_plan(&e), Some(Accept::KittyOrIterm2), "{e:?}");
        }
    }

    #[test]
    fn unknown_terminals_are_not_probed() {
        for e in [
            env(),
            GraphicsEnv {
                term_program: Some("Apple_Terminal".into()),
                ..env()
            },
            GraphicsEnv {
                term: Some("xterm-256color".into()),
                ..env()
            },
        ] {
            assert_eq!(probe_plan(&e), None, "{e:?}");
        }
    }

    #[test]
    fn accept_rules_filter_probe_answers() {
        let cell = (8, 17);
        assert_eq!(
            accepted_protocol(ProtocolType::Kitty, cell, Accept::KittyOnly),
            Some(ProtocolType::Kitty)
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Iterm2, cell, Accept::KittyOnly),
            None,
            "never iTerm2 under herdr"
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Sixel, cell, Accept::KittyOrIterm2),
            None,
            "Sixel stays text until P3-12d"
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Halfblocks, cell, Accept::KittyOrIterm2),
            None,
            "halfblocks are worse than the text tier"
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Kitty, cell, Accept::ForceIterm2),
            Some(ProtocolType::Iterm2),
            "iTerm2's false Kitty answer is overridden"
        );
    }

    #[test]
    fn zero_cell_size_is_rejected() {
        for accept in [
            Accept::KittyOnly,
            Accept::KittyOrIterm2,
            Accept::ForceIterm2,
        ] {
            assert_eq!(
                accepted_protocol(ProtocolType::Kitty, (0, 17), accept),
                None
            );
            assert_eq!(accepted_protocol(ProtocolType::Kitty, (8, 0), accept), None);
        }
    }

    #[test]
    fn prepare_scales_to_the_slot_and_pads_to_its_exact_pixel_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("wide.png");
        RgbaImage::from_pixel(400, 100, image::Rgba([200, 30, 30, 255]))
            .save(&path)
            .expect("write png");
        let key = SlotKey {
            path,
            stamp: (0, 0),
            cols: 10,
            rows: 3,
        };
        // Slot is 80×51 px: the 4:1 picture scales to 80×20, padded to the slot size.
        let img = prepare(&key, (8, 17)).expect("prepare");
        assert_eq!((img.width(), img.height()), (80, 51));
        let rgba = img.to_rgba8();
        assert_eq!(rgba.get_pixel(0, 0).0[3], 255, "picture at the top-left");
        assert_eq!(
            rgba.get_pixel(0, 40).0[3],
            0,
            "padding below is transparent"
        );
    }

    #[test]
    fn prepare_reports_unreadable_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bad.png");
        std::fs::write(&path, b"not a png").expect("write");
        let key = SlotKey {
            path,
            stamp: (0, 0),
            cols: 4,
            rows: 2,
        };
        assert!(prepare(&key, (8, 17)).is_err());
    }
}
