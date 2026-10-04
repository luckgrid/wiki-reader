//! Terminal graphics: startup probe (ADR-0004), background decode, and drawing image slots
//! (ADR-0017). The text placeholder always works; pictures are an upgrade.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
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
use wiki_reader_render::{
    DiagramPalette, DiagramRequest, DiagramSize, DiagramSizeCache, DiagramTextReason, ImageSlot,
    SlotSource, is_legible, mermaid_to_svg, rasterise_svg, render_mermaid, svg_natural_size,
};

use crate::tui::theme::Theme;

/// Query timeout. Answers arrive in single-digit milliseconds; the timeout only bounds a
/// terminal that never answers (see the P3-S1 spike for why unknown terminals are not probed).
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_millis(250);
/// Diagnostic override for the probe timeout, in milliseconds.
const QUERY_TIMEOUT_ENV: &str = "WIKI_READER_IMAGE_QUERY_TIMEOUT_MS";
/// Prepared pictures kept for scroll-back, in RGBA bytes. Visible pictures are never evicted, so
/// the cache can exceed this by at most one screenful.
const READY_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
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
///
/// `diagrams = "text" | "source"` must skip the probe entirely (ADR-0004 step 1) — callers pass
/// that via [`should_probe`].
#[must_use]
pub fn should_probe(mode: wiki_reader_core::config::DiagramMode) -> bool {
    !matches!(
        mode,
        wiki_reader_core::config::DiagramMode::Text | wiki_reader_core::config::DiagramMode::Source
    )
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

/// Identity of a prepared picture. File stamp / Mermaid hash keep stale cache entries out.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SlotKeySource {
    File { path: PathBuf, stamp: (u64, u64) },
    Mermaid { hash: u64, palette: DiagramPalette },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SlotKey {
    source: SlotKeySource,
    cols: u16,
    rows: u16,
}

impl SlotKey {
    fn of(slot: &ImageSlot) -> Self {
        let source = match &slot.source {
            SlotSource::File { path, stamp } => SlotKeySource::File {
                path: path.clone(),
                stamp: *stamp,
            },
            SlotSource::Mermaid { hash, palette, .. } => SlotKeySource::Mermaid {
                hash: *hash,
                palette: *palette,
            },
        };
        Self {
            source,
            cols: slot.cols,
            rows: slot.rows,
        }
    }
}

/// Decode job payload (visible slot → protocol).
struct DecodeJob {
    key: SlotKey,
    /// Mermaid fence body when `key` is Mermaid.
    mermaid_source: Option<String>,
    cell_px: (u16, u16),
    max_cols: u16,
}

enum Job {
    Decode(DecodeJob),
    /// Measure a Mermaid fence off-thread and fill the size cache (triggers a re-layout).
    Measure(DiagramRequest),
}

/// The slots on screen. The worker skips queued decode jobs that left it, so scrolling quickly
/// through a long page does not decode every picture that flew past.
type Wanted = Arc<Mutex<HashSet<SlotKey>>>;

enum Outcome {
    Ready(Box<StatefulProtocol>),
    Failed(String),
    /// Not on screen any more when the worker reached it; nothing was decoded.
    Skipped,
    /// Size cache updated; the page should re-layout so the block can become a slot.
    DiagramSized,
}

struct Done {
    key: Option<SlotKey>,
    outcome: Outcome,
}

enum Entry {
    Pending,
    Ready {
        protocol: Box<StatefulProtocol>,
        /// Prepared RGBA size, for the cache budget.
        bytes: u64,
        /// `ImageManager::tick` when last drawn (least-recently-used eviction).
        used: u64,
        /// The rectangle and crop at which the picture was last drawn with its first cell
        /// visible. The Kitty upload rides in that cell on the first render at a size and
        /// `ratatui-image` never repeats it, so the cell must not be under a popup then.
        confirmed: Option<(Rect, Clip)>,
    },
    /// Kept for the page, so a file that cannot be decoded is not retried on every scroll-in.
    Failed(String),
}

/// Scale to the slot's pixel size and pad to exactly that size. Files pad transparent; a Mermaid
/// card pads with its own background so the whole slot is one opaque rectangle.
fn scale_and_pad(
    decoded: &DynamicImage,
    cols: u16,
    rows: u16,
    font: (u16, u16),
    pad: Option<(u8, u8, u8)>,
) -> DynamicImage {
    let target_w = u32::from(cols) * u32::from(font.0);
    let target_h = u32::from(rows) * u32::from(font.1);
    let scaled = decoded.resize(target_w, target_h, FilterType::Triangle);
    let mut canvas = RgbaImage::new(target_w, target_h);
    if let Some((r, g, b)) = pad {
        canvas
            .pixels_mut()
            .for_each(|p| *p = image::Rgba([r, g, b, 255]));
    }
    image::imageops::overlay(&mut canvas, &scaled.to_rgba8(), 0, 0);
    DynamicImage::ImageRgba8(canvas)
}

/// Decode a bitmap file under the ADR-0017 allocation and pixel caps.
pub(crate) fn decode_file(path: &std::path::Path) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::open(path)
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
    Ok(decoded)
}

pub(crate) fn is_svg_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// Decode / rasterise `key` and pad to the slot's pixel size.
fn prepare(
    key: &SlotKey,
    font: (u16, u16),
    mermaid_source: Option<&str>,
    cell_px: (u16, u16),
    max_cols: u16,
) -> Result<DynamicImage, String> {
    let decoded = match &key.source {
        SlotKeySource::File { path, .. } => {
            if is_svg_path(path) {
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                let raster = rasterise_svg(&bytes).map_err(|e| e.to_string())?;
                DynamicImage::ImageRgba8(raster.image)
            } else {
                decode_file(path)?
            }
        }
        SlotKeySource::Mermaid { palette, .. } => {
            let src = mermaid_source.ok_or_else(|| "missing mermaid source".to_owned())?;
            let raster = render_mermaid(src, palette).map_err(|e| e.to_string())?;
            if !is_legible(raster.px_w, raster.px_h, cell_px, max_cols, key.rows) {
                return Err("diagram too wide for pane".into());
            }
            DynamicImage::ImageRgba8(raster.image)
        }
    };
    let pad = match &key.source {
        SlotKeySource::Mermaid { palette, .. } => Some(palette.bg),
        SlotKeySource::File { .. } => None,
    };
    Ok(scale_and_pad(&decoded, key.cols, key.rows, font, pad))
}

/// SVG layout + natural size only — no RGBA. Rasterisation happens in [`prepare`].
fn run_measure(req: &DiagramRequest, sizes: &DiagramSizeCache) -> Outcome {
    match mermaid_to_svg(&req.source, &req.palette).and_then(|svg| svg_natural_size(svg.as_bytes()))
    {
        Ok((px_w, px_h)) => {
            sizes.insert(req.hash, req.palette, DiagramSize::Natural { px_w, px_h });
            Outcome::DiagramSized
        }
        Err(err) => {
            sizes.insert(
                req.hash,
                req.palette,
                DiagramSize::Text(DiagramTextReason::Failed(err.to_string())),
            );
            Outcome::DiagramSized
        }
    }
}

/// Run one job: skip decode if unwanted, otherwise decode and build the protocol.
fn run_job(
    job: &Job,
    wanted: &Wanted,
    font: (u16, u16),
    picker: &Picker,
    sizes: &DiagramSizeCache,
) -> Outcome {
    match job {
        Job::Measure(req) => run_measure(req, sizes),
        Job::Decode(DecodeJob {
            key,
            mermaid_source,
            cell_px,
            max_cols,
        }) => {
            if !wanted
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contains(key)
            {
                return Outcome::Skipped;
            }
            match prepare(key, font, mermaid_source.as_deref(), *cell_px, *max_cols) {
                Ok(img) => Outcome::Ready(Box::new(picker.new_resize_protocol(img))),
                Err(reason) => Outcome::Failed(reason),
            }
        }
    }
}

/// Block until the next job arrives, preferring Decode over Measure.
///
/// `deferred` holds a Measure that was already received but yielded to a Decode.
fn recv_prefer_decode(
    decode_rx: &Receiver<DecodeJob>,
    measure_rx: &Receiver<DiagramRequest>,
    deferred: &mut Option<DiagramRequest>,
) -> Option<Job> {
    loop {
        if let Ok(job) = decode_rx.try_recv() {
            return Some(Job::Decode(job));
        }
        if let Some(req) = deferred.take() {
            return Some(Job::Measure(req));
        }
        match measure_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(req) => {
                if let Ok(job) = decode_rx.try_recv() {
                    *deferred = Some(req);
                    return Some(Job::Decode(job));
                }
                return Some(Job::Measure(req));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                match decode_rx.try_recv() {
                    Ok(job) => return Some(Job::Decode(job)),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Decodes closed; drain measures.
                        return measure_rx.recv().ok().map(Job::Measure);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return decode_rx.recv().ok().map(Job::Decode);
            }
        }
    }
}

/// Owns the terminal's graphics capability, the decode worker and the prepared pictures.
pub struct ImageManager {
    picker: Option<Picker>,
    decode_jobs: Option<Sender<DecodeJob>>,
    measure_jobs: Option<Sender<DiagramRequest>>,
    done: Option<Receiver<Done>>,
    wanted: Wanted,
    entries: HashMap<SlotKey, Entry>,
    /// Counts draws; stamps `Entry::Ready::used`.
    tick: u64,
    ready_budget: u64,
    diagram_sizes: Arc<DiagramSizeCache>,
    /// Set when a Mermaid measure finished; the app re-lays out the page.
    diagram_relayout: bool,
    /// Hashes already queued for measure this session (avoid re-queue spam).
    measuring: HashSet<(u64, DiagramPalette)>,
    /// In-flight measure jobs (for `has_pending` / fast poll).
    measure_inflight: usize,
}

impl ImageManager {
    /// No graphics protocol: every image stays a text placeholder.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            picker: None,
            decode_jobs: None,
            measure_jobs: None,
            done: None,
            wanted: Wanted::default(),
            entries: HashMap::new(),
            tick: 0,
            ready_budget: READY_BYTE_BUDGET,
            diagram_sizes: Arc::new(DiagramSizeCache::new()),
            diagram_relayout: false,
            measuring: HashSet::new(),
            measure_inflight: 0,
        }
    }

    /// Start the decode worker, sharing a Mermaid size cache with the renderer.
    #[must_use]
    pub fn enabled_with_sizes(picker: Picker, diagram_sizes: Arc<DiagramSizeCache>) -> Self {
        let (decode_tx, decode_rx) = mpsc::channel::<DecodeJob>();
        let (measure_tx, measure_rx) = mpsc::channel::<DiagramRequest>();
        let (done_tx, done_rx) = mpsc::channel::<Done>();
        let wanted = Wanted::default();
        let worker_picker = picker.clone();
        let worker_wanted = Arc::clone(&wanted);
        let worker_sizes = Arc::clone(&diagram_sizes);
        let font = picker.font_size();
        std::thread::spawn(move || {
            let mut deferred = None;
            while let Some(job) = recv_prefer_decode(&decode_rx, &measure_rx, &mut deferred) {
                let key = match &job {
                    Job::Decode(DecodeJob { key, .. }) => Some(key.clone()),
                    Job::Measure(_) => None,
                };
                let outcome = run_job(
                    &job,
                    &worker_wanted,
                    (font.width, font.height),
                    &worker_picker,
                    &worker_sizes,
                );
                if done_tx.send(Done { key, outcome }).is_err() {
                    break;
                }
            }
        });
        Self {
            picker: Some(picker),
            decode_jobs: Some(decode_tx),
            measure_jobs: Some(measure_tx),
            done: Some(done_rx),
            wanted,
            entries: HashMap::new(),
            tick: 0,
            ready_budget: READY_BYTE_BUDGET,
            diagram_sizes,
            diagram_relayout: false,
            measuring: HashSet::new(),
            measure_inflight: 0,
        }
    }

    /// Shared Mermaid natural-size cache (also passed into [`wiki_reader_render::RenderOpts`]).
    #[must_use]
    pub fn diagram_sizes(&self) -> Arc<DiagramSizeCache> {
        Arc::clone(&self.diagram_sizes)
    }

    /// The graphics picker, for modal viewers that draw their own pictures (P3-15).
    #[must_use]
    pub fn picker(&self) -> Option<Picker> {
        self.picker.clone()
    }

    /// Cell size in pixels when a protocol is active (the renderer sizes slots from it).
    #[must_use]
    pub fn cell_px(&self) -> Option<(u16, u16)> {
        self.picker.as_ref().map(|p| {
            let font = p.font_size();
            (font.width, font.height)
        })
    }

    /// Queue Mermaid size measures from the latest render. Idempotent per (hash, palette).
    pub fn queue_diagram_requests(&mut self, requests: &[DiagramRequest]) {
        let Some(tx) = &self.measure_jobs else {
            return;
        };
        for req in requests {
            let key = (req.hash, req.palette);
            if self.diagram_sizes.get(req.hash, req.palette).is_some() {
                continue;
            }
            if !self.measuring.insert(key) {
                continue;
            }
            if tx.send(req.clone()).is_ok() {
                self.measure_inflight += 1;
            } else {
                self.measuring.remove(&key);
            }
        }
    }

    /// True when a Mermaid measure finished and the page should re-layout.
    pub fn take_diagram_relayout(&mut self) -> bool {
        std::mem::take(&mut self.diagram_relayout)
    }

    /// Drain finished decodes. True when a picture arrived, so the frame needs a redraw.
    pub fn poll(&mut self) -> bool {
        let Some(done) = &self.done else {
            return false;
        };
        let font = self.cell_px().unwrap_or((0, 0));
        let mut changed = false;
        while let Ok(Done { key, outcome }) = done.try_recv() {
            match outcome {
                Outcome::DiagramSized => {
                    self.diagram_relayout = true;
                    self.measure_inflight = self.measure_inflight.saturating_sub(1);
                    changed = true;
                }
                other => {
                    let Some(key) = key else {
                        continue;
                    };
                    // A key dropped by `retain_for` while decoding is not resurrected.
                    let Some(entry) = self.entries.get_mut(&key) else {
                        continue;
                    };
                    match other {
                        Outcome::Ready(protocol) => {
                            let bytes = u64::from(key.cols)
                                * u64::from(font.0)
                                * u64::from(key.rows)
                                * u64::from(font.1)
                                * 4;
                            *entry = Entry::Ready {
                                protocol,
                                bytes,
                                used: self.tick,
                                confirmed: None,
                            };
                            changed = true;
                        }
                        Outcome::Failed(reason) => {
                            *entry = Entry::Failed(reason);
                            changed = true;
                        }
                        Outcome::Skipped => {
                            if matches!(entry, Entry::Pending) {
                                self.entries.remove(&key);
                            }
                        }
                        Outcome::DiagramSized => unreachable!("handled above"),
                    }
                }
            }
        }
        changed
    }

    /// True while a decode or measure is queued or running.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.entries.values().any(|e| matches!(e, Entry::Pending)) || self.measure_inflight > 0
    }

    /// Keep only the entries `slots` need (pictures and decode failures); release the rest.
    /// Called when a page is (re)built, so state never outlives its page.
    pub fn retain_for(&mut self, slots: &[ImageSlot]) {
        let keep: Vec<SlotKey> = slots.iter().map(SlotKey::of).collect();
        self.entries.retain(|key, _| keep.contains(key));
        // Allow re-measure after a page change if sizes were cleared externally.
        self.measuring
            .retain(|(hash, palette)| self.diagram_sizes.get(*hash, *palette).is_none());
    }

    /// Drop least-recently-drawn prepared pictures until the cache fits its budget. Pictures on
    /// screen are never evicted, and failures cost almost nothing, so they stay.
    fn evict_over_budget(&mut self, visible: &HashSet<SlotKey>) {
        let mut total: u64 = self
            .entries
            .values()
            .map(|e| match e {
                Entry::Ready { bytes, .. } => *bytes,
                _ => 0,
            })
            .sum();
        if total <= self.ready_budget {
            return;
        }
        let mut candidates: Vec<(u64, SlotKey)> = self
            .entries
            .iter()
            .filter_map(|(key, e)| match e {
                Entry::Ready { used, .. } if !visible.contains(key) => Some((*used, key.clone())),
                _ => None,
            })
            .collect();
        candidates.sort_by_key(|(used, _)| *used);
        for (_, key) in candidates {
            if total <= self.ready_budget {
                break;
            }
            if let Some(Entry::Ready { bytes, .. }) = self.entries.remove(&key) {
                total -= bytes;
            }
        }
    }

    /// Queue a decode for `slot` if it has no entry yet.
    fn ensure(&mut self, slot: &ImageSlot) {
        let key = SlotKey::of(slot);
        if self.entries.contains_key(&key) {
            return;
        }
        let mermaid_source = match &slot.source {
            SlotSource::Mermaid { source, .. } => Some(source.clone()),
            SlotSource::File { .. } => None,
        };
        let cell_px = self.cell_px().unwrap_or((8, 17));
        let queued = self.decode_jobs.as_ref().is_some_and(|tx| {
            tx.send(DecodeJob {
                key: key.clone(),
                mermaid_source,
                cell_px,
                max_cols: slot.cols,
            })
            .is_ok()
        });
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
    ///
    /// Only visible slots are queued, and the worker skips queued jobs that scrolled away, so a
    /// page with arbitrarily many image references cannot turn the per-image limits into
    /// unbounded aggregate work. Prepared pictures stay cached for scroll-back up to a byte
    /// budget (least recently drawn go first); decode failures stay for the page.
    pub fn draw(
        &mut self,
        frame: &mut Frame<'_>,
        slots: &[ImageSlot],
        scroll: u32,
        geom: crate::tui::regions::viewer::ViewerGeom,
        theme: &Theme,
        occlusion: Occlusion<'_>,
    ) {
        if self.picker.is_none() {
            return;
        }
        self.tick += 1;
        let visible_keys: HashSet<_> = slots
            .iter()
            .filter(|slot| slot_visible(slot, scroll, geom.text_x, geom.top_y, geom.rows).is_some())
            .map(SlotKey::of)
            .collect();
        self.wanted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone_from(&visible_keys);
        self.evict_over_budget(&visible_keys);

        for slot in slots {
            let visible = slot_visible(slot, scroll, geom.text_x, geom.top_y, geom.rows);
            let Some((rect, clip)) = visible else {
                // This also draws the documented fallback for a slot clipped at both edges;
                // a completely off-screen slot is a no-op.
                draw_placeholder(frame, slot, scroll, geom, None, theme);
                continue;
            };
            self.ensure(slot);
            let key = SlotKey::of(slot);
            match self.entries.get_mut(&key) {
                Some(Entry::Ready {
                    protocol,
                    used,
                    confirmed,
                    ..
                }) => {
                    let rect = rect.intersection(frame.area());
                    let covered = occlusion.covers(rect.x, rect.y);
                    let shown = Some((rect, clip));
                    if covered && *confirmed != shown {
                        // This render would carry the upload in a cell a popup is about to
                        // overwrite, and it would never be sent again. Wait for the popup to
                        // close; the placeholder stands in meanwhile.
                        draw_placeholder(frame, slot, scroll, geom, None, theme);
                        continue;
                    }
                    *used = self.tick;
                    let resize = Resize::Crop(Some(CropOptions {
                        clip_top: clip == Clip::Top,
                        clip_left: false,
                    }));
                    frame.render_stateful_widget(
                        StatefulImage::new().resize(resize),
                        rect,
                        protocol.as_mut(),
                    );
                    if !covered {
                        *confirmed = shown;
                    }
                }
                entry => {
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

/// What covers the viewer when pictures are drawn, because popups draw after them.
#[derive(Debug, Clone, Copy)]
pub enum Occlusion<'a> {
    /// Nothing is drawn over the pictures.
    None,
    /// These popup panels will be drawn over them.
    Rects(&'a [Rect]),
    /// A popup is open but its panel is not known yet (it just opened): assume it covers
    /// everything.
    All,
}

impl Occlusion<'_> {
    fn covers(&self, x: u16, y: u16) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Rects(rects) => rects.iter().any(|r| r.contains((x, y).into())),
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
            source: SlotSource::File {
                path: PathBuf::from("a.png"),
                stamp: (1, 1),
            },
            alt: "alt".into(),
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
    fn diagrams_text_and_source_skip_the_probe() {
        use wiki_reader_core::config::DiagramMode;
        assert!(!should_probe(DiagramMode::Text));
        assert!(!should_probe(DiagramMode::Source));
        assert!(should_probe(DiagramMode::Auto));
        assert!(should_probe(DiagramMode::Image));
    }

    #[test]
    fn adr_detection_table_outside_herdr() {
        assert_eq!(
            probe_plan(&GraphicsEnv {
                term_program: Some("ghostty".into()),
                ..env()
            }),
            Some(Accept::KittyOrIterm2)
        );
        assert_eq!(
            probe_plan(&GraphicsEnv {
                term_program: Some("WezTerm".into()),
                ..env()
            }),
            Some(Accept::KittyOrIterm2)
        );
        assert_eq!(
            probe_plan(&GraphicsEnv {
                kitty_window: true,
                ..env()
            }),
            Some(Accept::KittyOrIterm2)
        );
        assert_eq!(
            probe_plan(&GraphicsEnv {
                term: Some("xterm-kitty".into()),
                ..env()
            }),
            Some(Accept::KittyOrIterm2)
        );
        // Terminal.app / unknown: no probe.
        assert_eq!(
            probe_plan(&GraphicsEnv {
                term_program: Some("Apple_Terminal".into()),
                ..env()
            }),
            None
        );
        assert_eq!(probe_plan(&env()), None);
    }

    #[test]
    fn herdr_never_accepts_sixel_or_iterm2() {
        let cell = (8, 17);
        assert_eq!(
            accepted_protocol(ProtocolType::Sixel, cell, Accept::KittyOnly),
            None
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Iterm2, cell, Accept::KittyOnly),
            None
        );
        assert_eq!(
            accepted_protocol(ProtocolType::Kitty, cell, Accept::KittyOnly),
            Some(ProtocolType::Kitty)
        );
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
            "Sixel is never accepted (text tier is better)"
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

    fn key(name: &str) -> SlotKey {
        SlotKey {
            source: SlotKeySource::File {
                path: PathBuf::from(name),
                stamp: (1, 1),
            },
            cols: 4,
            rows: 2,
        }
    }

    fn ready(bytes: u64, used: u64) -> Entry {
        Entry::Ready {
            protocol: Box::new(
                Picker::halfblocks().new_resize_protocol(DynamicImage::new_rgba8(2, 2)),
            ),
            bytes,
            used,
            confirmed: None,
        }
    }

    fn manager_with(budget: u64, entries: Vec<(SlotKey, Entry)>) -> ImageManager {
        let mut manager = ImageManager::enabled_with_sizes(
            Picker::halfblocks(),
            Arc::new(DiagramSizeCache::new()),
        );
        manager.ready_budget = budget;
        manager.entries.extend(entries);
        manager
    }

    #[test]
    fn eviction_drops_least_recently_drawn_first() {
        let visible: HashSet<_> = [key("c")].into();
        let mut manager = manager_with(
            250,
            vec![
                (key("a"), ready(100, 1)),
                (key("b"), ready(100, 2)),
                (key("c"), ready(100, 3)),
                (key("failed"), Entry::Failed("bad".into())),
            ],
        );
        manager.evict_over_budget(&visible);
        assert!(
            !manager.entries.contains_key(&key("a")),
            "oldest goes first"
        );
        for kept in ["b", "c", "failed"] {
            assert!(manager.entries.contains_key(&key(kept)), "{kept}");
        }
    }

    #[test]
    fn eviction_never_drops_visible_pictures_or_failures() {
        let visible: HashSet<_> = [key("c")].into();
        let mut manager = manager_with(
            50,
            vec![
                (key("a"), ready(100, 1)),
                (key("b"), ready(100, 2)),
                (key("c"), ready(100, 3)),
                (key("failed"), Entry::Failed("bad".into())),
                (key("pending"), Entry::Pending),
            ],
        );
        manager.evict_over_budget(&visible);
        assert!(!manager.entries.contains_key(&key("a")));
        assert!(!manager.entries.contains_key(&key("b")));
        for kept in ["c", "failed", "pending"] {
            assert!(manager.entries.contains_key(&key(kept)), "{kept}");
        }
    }

    #[test]
    fn under_budget_nothing_is_evicted() {
        let mut manager = manager_with(
            1000,
            vec![(key("a"), ready(100, 1)), (key("b"), ready(100, 2))],
        );
        manager.evict_over_budget(&HashSet::new());
        assert_eq!(manager.entries.len(), 2);
    }

    #[test]
    fn worker_skips_jobs_that_left_the_screen_without_touching_the_file() {
        let wanted = Wanted::default();
        let picker = Picker::halfblocks();
        let sizes = DiagramSizeCache::new();
        let job = Job::Decode(DecodeJob {
            key: key("gone.png"),
            mermaid_source: None,
            cell_px: (10, 20),
            max_cols: 4,
        });
        // Not wanted: skipped, even though the file does not exist (it was never opened).
        assert!(matches!(
            run_job(&job, &wanted, (10, 20), &picker, &sizes),
            Outcome::Skipped
        ));
        // Wanted: it is decoded, and a missing file is a failure rather than a skip.
        wanted.lock().unwrap().insert(key("gone.png"));
        assert!(matches!(
            run_job(&job, &wanted, (10, 20), &picker, &sizes),
            Outcome::Failed(_)
        ));
    }

    #[test]
    fn mermaid_slot_padding_is_the_palette_background_not_transparent() {
        let palette = DiagramPalette::default();
        let key = SlotKey {
            source: SlotKeySource::Mermaid { hash: 1, palette },
            cols: 40,
            rows: 8,
        };
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let img = prepare(&key, (8, 17), Some(src), (8, 17), 40)
            .expect("prepare")
            .to_rgba8();
        assert_eq!((img.width(), img.height()), (320, 136));
        assert!(
            img.pixels().all(|p| p.0[3] == 255),
            "the whole slot is one opaque card"
        );
        let (r, g, b) = palette.bg;
        assert_eq!(
            img.get_pixel(319, 135).0,
            [r, g, b, 255],
            "padding uses the card bg"
        );
    }

    #[test]
    fn prepare_scales_to_the_slot_and_pads_to_its_exact_pixel_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("wide.png");
        RgbaImage::from_pixel(400, 100, image::Rgba([200, 30, 30, 255]))
            .save(&path)
            .expect("write png");
        let key = SlotKey {
            source: SlotKeySource::File {
                path,
                stamp: (0, 0),
            },
            cols: 10,
            rows: 3,
        };
        // Slot is 80×51 px: the 4:1 picture scales to 80×20, padded to the slot size.
        let img = prepare(&key, (8, 17), None, (8, 17), 10).expect("prepare");
        assert_eq!((img.width(), img.height()), (80, 51));
        let rgba_img = img.to_rgba8();
        assert_eq!(
            rgba_img.get_pixel(0, 0).0[3],
            255,
            "picture at the top-left"
        );
        assert_eq!(
            rgba_img.get_pixel(0, 40).0[3],
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
            source: SlotKeySource::File {
                path,
                stamp: (0, 0),
            },
            cols: 4,
            rows: 2,
        };
        assert!(prepare(&key, (8, 17), None, (8, 17), 4).is_err());
    }

    #[test]
    fn measure_stores_natural_size_without_raster_and_survives_width_change() {
        let sizes = DiagramSizeCache::new();
        let src = "flowchart LR\n  A[Build] --> B[Deploy]\n";
        let hash = wiki_reader_render::content_hash(src);
        let req = DiagramRequest {
            hash,
            source: src.into(),
            width: 80,
            palette: DiagramPalette::default(),
            cell_px: (8, 17),
        };
        assert!(matches!(run_measure(&req, &sizes), Outcome::DiagramSized));
        let first = sizes.get(hash, req.palette).expect("cached");
        // Same key for every width: layout re-checks legibility.
        assert_eq!(sizes.get(hash, req.palette), Some(first.clone()));
        assert!(matches!(first, DiagramSize::Natural { .. }));
    }

    #[test]
    fn measure_records_parse_failure_reason() {
        let sizes = DiagramSizeCache::new();
        let src = "not a real mermaid {{{";
        let hash = wiki_reader_render::content_hash(src);
        let req = DiagramRequest {
            hash,
            source: src.into(),
            width: 80,
            palette: DiagramPalette::default(),
            cell_px: (8, 17),
        };
        assert!(matches!(run_measure(&req, &sizes), Outcome::DiagramSized));
        match sizes.get(hash, req.palette) {
            Some(DiagramSize::Text(DiagramTextReason::Failed(msg))) => {
                assert!(!msg.is_empty(), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn decode_is_preferred_over_pending_measure() {
        let (decode_tx, decode_rx) = mpsc::channel::<DecodeJob>();
        let (measure_tx, measure_rx) = mpsc::channel::<DiagramRequest>();
        let mut deferred = None;
        measure_tx
            .send(DiagramRequest {
                hash: 1,
                source: "graph LR; A --> B".into(),
                width: 80,
                palette: DiagramPalette::default(),
                cell_px: (8, 17),
            })
            .unwrap();
        decode_tx
            .send(DecodeJob {
                key: key("vis.png"),
                mermaid_source: None,
                cell_px: (8, 17),
                max_cols: 4,
            })
            .unwrap();
        let job = recv_prefer_decode(&decode_rx, &measure_rx, &mut deferred).expect("job");
        assert!(
            matches!(job, Job::Decode(_)),
            "decode must win when both are pending"
        );
        assert!(
            deferred.is_some()
                || matches!(
                    recv_prefer_decode(&decode_rx, &measure_rx, &mut deferred),
                    Some(Job::Measure(_))
                )
        );
    }
}
