//! Image and diagram viewer (P3-15, P3-20..22): a [`ModalContent`] over the document's pictures.
//!
//! The viewer holds the whole media inventory and shows one item at a time; `Tab` / `Shift+Tab`
//! step through them (P3-21). A diagram cycles image / text / source with `v` (P3-22, session
//! only); a view that cannot be shown (no graphics protocol) is skipped. The window is sized to
//! the picture, capped by the shell (P3-20); `a` flips fit / actual size on top of the zoom.
//! This module only exists with the `media` feature, so the lite build never has an image view.
//!
//! Zoom decision (plan, locked): SVG and Mermaid **re-rasterise** from source at the zoomed size
//! (sharp); raster files are **scaled**. Both happen on a worker thread that lives as long as the
//! viewer (dropping the viewer drops the sender, the worker exits and frees the pictures).
//! The ADR-0017 pixel cap applies to every zoomed size, see [`zoom_dims`]. Without a graphics
//! protocol the viewer shows the diagram source instead.

// Pixel and fraction math: values are bounded by the 16 MP cap, far inside f64's exact range.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use std::fmt::Write as _;
use std::sync::mpsc::{self, Receiver, Sender};

use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui_image::StatefulImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use wiki_reader_core::images::{MAX_IMAGE_FILE_BYTES, MAX_IMAGE_PIXELS};
use wiki_reader_render::{
    DiagramTier, SlotSource, decode_file, diagram_lines, is_svg_path, mermaid_svg_bytes,
    rasterise_svg_scaled, svg_natural_size,
};

use super::modal_viewer::{ModalContent, ModalEvent};
use super::text_col::line_width;
use super::theme::Theme;

/// Zoom steps, percent of the base size (100 = the fit size, or natural pixels in actual mode).
const ZOOMS: [u32; 7] = [100, 150, 200, 300, 400, 600, 800];
/// Fit mode enlarges a small picture up to this factor (the inline slot never exceeds natural
/// size, so opening a picture always shows it larger).
// ponytail: fixed 2×; upgrade: a config knob if dogfood wants more.
const FIT_UPSCALE: f64 = 2.0;

/// Pixel size of the picture at `zoom_pct`, then shrunk if needed so `w × h` stays within
/// [`MAX_IMAGE_PIXELS`]. The base scale is **fit** (the window, at most [`FIT_UPSCALE`]×
/// natural) or **actual** (1 picture pixel = 1 terminal pixel, whatever the window).
#[must_use]
pub fn zoom_dims(nat: (u32, u32), vp: (u32, u32), zoom_pct: u32, actual: bool) -> (u32, u32) {
    let (nw, nh) = (f64::from(nat.0.max(1)), f64::from(nat.1.max(1)));
    let base = if actual {
        1.0
    } else {
        (f64::from(vp.0) / nw)
            .min(f64::from(vp.1) / nh)
            .min(FIT_UPSCALE)
    };
    let cap = (MAX_IMAGE_PIXELS as f64 / (nw * nh)).sqrt() * 0.999;
    let scale = (base * f64::from(zoom_pct) / 100.0).min(cap);
    let dim = |n: f64| (n * scale).ceil().max(1.0) as u32;
    (dim(nw), dim(nh))
}

/// What a cropped picture was cut from: window offset, zoomed width, size in cells.
type ShownId = (u32, u32, u32, u16, u16);

/// One zoom request: percent, window size in pixels, actual-size mode.
type ReqKey = (u32, (u32, u32), bool);

struct Zoomed {
    img: RgbaImage,
    key: ReqKey,
}

/// What the worker keeps between requests: the decoded bitmap, or the SVG to re-rasterise.
enum Natural {
    Bitmap(DynamicImage),
    Svg(Vec<u8>, (u32, u32)),
}

impl Natural {
    fn size(&self) -> (u32, u32) {
        match self {
            Self::Bitmap(i) => (i.width(), i.height()),
            Self::Svg(_, size) => *size,
        }
    }
}

fn load(source: &SlotSource) -> Result<Natural, String> {
    match source {
        SlotSource::File { path, .. } => {
            // The slot was validated at layout; re-check, the file may have changed since.
            let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
            if len > MAX_IMAGE_FILE_BYTES {
                return Err("file too large".into());
            }
            if is_svg_path(path) {
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                let size = svg_natural_size(&bytes).map_err(|e| e.to_string())?;
                Ok(Natural::Svg(bytes, size))
            } else {
                decode_file(path).map(Natural::Bitmap)
            }
        }
        SlotSource::Mermaid {
            palette, source, ..
        } => {
            let bytes = mermaid_svg_bytes(source, palette).map_err(|e| e.to_string())?;
            let size = svg_natural_size(&bytes).map_err(|e| e.to_string())?;
            Ok(Natural::Svg(bytes, size))
        }
    }
}

fn zoom_to(nat: &Natural, key: ReqKey) -> Result<RgbaImage, String> {
    let (w, h) = zoom_dims(nat.size(), key.1, key.0, key.2);
    match nat {
        Natural::Bitmap(img) => Ok(img.resize_exact(w, h, FilterType::Triangle).to_rgba8()),
        Natural::Svg(bytes, (nw, _)) => {
            #[allow(clippy::cast_precision_loss)] // pixel sizes are far below f32's exact range
            let scale = w as f32 / *nw as f32;
            rasterise_svg_scaled(bytes, scale)
                .map(|r| r.image)
                .map_err(|e| e.to_string())
        }
    }
}

/// Worker loop: newest request wins, the decoded source is kept for the next zoom.
fn work(source: &SlotSource, reqs: &Receiver<ReqKey>, out: &Sender<Result<Zoomed, String>>) {
    let mut natural: Option<Result<Natural, String>> = None;
    while let Ok(mut key) = reqs.recv() {
        while let Ok(newer) = reqs.try_recv() {
            key = newer;
        }
        let nat = natural.get_or_insert_with(|| load(source));
        let result = match nat {
            Ok(nat) => zoom_to(nat, key).map(|img| Zoomed { img, key }),
            Err(e) => Err(e.clone()),
        };
        if out.send(result).is_err() {
            return;
        }
    }
}

/// Graphics half of the viewer: exists only in [`View::Image`].
struct Gfx {
    picker: Picker,
    reqs: Sender<ReqKey>,
    results: Receiver<Result<Zoomed, String>>,
    /// Index into [`ZOOMS`].
    zoom: usize,
    /// Base scale: natural pixels instead of fit-to-window.
    actual: bool,
    /// Window centre as a fraction of the zoomed picture, so zoom keeps what you look at.
    centre: (f64, f64),
    zoomed: Option<Zoomed>,
    asked: Option<ReqKey>,
    error: Option<String>,
    /// Body size in pixels, from the last draw (pan step, crop).
    vp: (u32, u32),
    /// The cropped picture on screen, and what it was cut from.
    shown: Option<(ShownId, StatefulProtocol)>,
    /// Mermaid cards pad with their own background.
    pad: Option<(u8, u8, u8)>,
}

impl Gfx {
    fn new(picker: Picker, source: SlotSource) -> Self {
        let pad = match &source {
            SlotSource::Mermaid { palette, .. } => Some(palette.bg),
            SlotSource::File { .. } => None,
        };
        let (reqs, req_rx) = mpsc::channel();
        let (out, results) = mpsc::channel();
        std::thread::spawn(move || work(&source, &req_rx, &out));
        Self {
            picker,
            reqs,
            results,
            zoom: 0,
            actual: false,
            centre: (0.5, 0.5),
            zoomed: None,
            asked: None,
            error: None,
            vp: (640, 340),
            shown: None,
            pad,
        }
    }

    /// Top-left of the window in the zoomed picture, clamped inside it.
    fn offset(&self) -> (u32, u32) {
        let Some(z) = &self.zoomed else {
            return (0, 0);
        };
        let at = |c: f64, zoomed: u32, vp: u32| {
            let max = f64::from(zoomed.saturating_sub(vp));
            (c * f64::from(zoomed) - f64::from(vp) / 2.0).clamp(0.0, max) as u32
        };
        (
            at(self.centre.0, z.img.width(), self.vp.0),
            at(self.centre.1, z.img.height(), self.vp.1),
        )
    }

    fn pan(&mut self, dx: i64, dy: i64) {
        let Some(z) = &self.zoomed else {
            return;
        };
        let (zw, zh) = (z.img.width(), z.img.height());
        let (ox, oy) = self.offset();
        let to = |off: u32, d: i64, zoomed: u32, vp: u32| {
            let max = i64::from(zoomed.saturating_sub(vp));
            let off = (i64::from(off) + d).clamp(0, max);
            // Store the clamped result so reversing direction responds at once.
            (off as f64 + f64::from(vp) / 2.0) / f64::from(zoomed.max(1))
        };
        self.centre = (to(ox, dx, zw, self.vp.0), to(oy, dy, zh, self.vp.1));
    }

    fn set_zoom(&mut self, zoom: usize) {
        self.zoom = zoom.min(ZOOMS.len() - 1);
    }

    /// Ask for the zoomed picture if the zoom, mode or window `vp` (pixels) changed, and take
    /// finished ones.
    fn sync(&mut self, vp: (u32, u32)) {
        let key = (ZOOMS[self.zoom], vp, self.actual);
        if self.asked != Some(key) {
            self.asked = Some(key);
            // A closed worker shows up as an error on the next read; nothing to do here.
            let _ = self.reqs.send(key);
        }
        while let Ok(result) = self.results.try_recv() {
            match result {
                Ok(z) => {
                    self.zoomed = Some(z);
                    self.error = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
}

/// One picture or diagram of the document (P3-21 inventory entry).
pub struct ViewerItem {
    pub alt: String,
    /// What to draw; `None` for a picture the layout could not load (see `note`).
    pub src: Option<SlotSource>,
    /// Shown when there is no source view: why the picture is not available.
    pub note: String,
}

/// What the viewer shows for the current item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Image,
    /// Mermaid text art at the window width.
    Text,
    /// Mermaid source, or the note for a picture with nothing to draw.
    Source,
}

impl View {
    fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Text => "text",
            Self::Source => "source",
        }
    }
}

fn source_lines(item: &ViewerItem) -> Vec<String> {
    match &item.src {
        Some(SlotSource::Mermaid { source, .. }) => source.lines().map(str::to_owned).collect(),
        Some(SlotSource::File { path, .. }) => {
            vec![format!("{}: no graphics protocol", path.display())]
        }
        None => vec![item.note.clone()],
    }
}

/// Open viewer state.
pub struct ImageViewer {
    items: Vec<ViewerItem>,
    cur: usize,
    picker: Option<Picker>,
    view: View,
    /// `Some` exactly in [`View::Image`]; dropping it stops the worker.
    gfx: Option<Gfx>,
    /// Lines of the text views (and the fallback when the picture cannot render).
    text: Vec<String>,
    /// Width the [`View::Text`] art was laid out for.
    text_w: u16,
    scroll: usize,
}

impl ImageViewer {
    /// Show `items[start]` first. `picker` is `None` without a graphics protocol, which leaves
    /// diagrams on their text / source views. `items` must not be empty.
    #[must_use]
    pub fn new(items: Vec<ViewerItem>, start: usize, picker: Option<Picker>) -> Self {
        let mut v = Self {
            cur: start.min(items.len().saturating_sub(1)),
            items,
            picker,
            view: View::Source,
            gfx: None,
            text: Vec::new(),
            text_w: 0,
            scroll: 0,
        };
        v.show(v.cur);
        v
    }

    fn item(&self) -> &ViewerItem {
        &self.items[self.cur]
    }

    fn is_diagram(&self) -> bool {
        matches!(self.item().src, Some(SlotSource::Mermaid { .. }))
    }

    /// Views this item can show here: no image without a protocol (or a source), no text art
    /// for a plain picture.
    fn views(&self) -> Vec<View> {
        let mut v = Vec::new();
        if self.picker.is_some() && self.item().src.is_some() {
            v.push(View::Image);
        }
        if self.is_diagram() {
            v.push(View::Text);
        }
        v.push(View::Source);
        v
    }

    fn show(&mut self, idx: usize) {
        self.cur = idx;
        let first = self.views()[0];
        // A diagram without a protocol opens on its source, as the inline tier left it.
        self.set_view(if first == View::Text {
            View::Source
        } else {
            first
        });
    }

    fn set_view(&mut self, view: View) {
        self.view = view;
        self.scroll = 0;
        self.text_w = 0;
        self.text = source_lines(self.item());
        self.gfx = match (view, &self.picker, &self.item().src) {
            (View::Image, Some(p), Some(src)) => Some(Gfx::new(p.clone(), src.clone())),
            _ => None,
        };
    }

    /// Step through the document's pictures, wrapping.
    fn step(&mut self, forward: bool) {
        let n = self.items.len();
        if n > 1 {
            self.show(if forward {
                (self.cur + 1) % n
            } else {
                (self.cur + n - 1) % n
            });
        }
    }

    /// Everything but item / view switching: pan and zoom the picture, or scroll the text.
    fn pan_or_scroll(&mut self, code: KeyCode) {
        let Some(g) = self.gfx.as_mut() else {
            match code {
                KeyCode::Up | KeyCode::Char('k') => self.scroll_text(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_text(1),
                KeyCode::PageUp => self.scroll_text(-10),
                KeyCode::PageDown => self.scroll_text(10),
                KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => self.scroll_text(isize::MAX),
                _ => {}
            }
            return;
        };
        // ponytail: keys only; mouse is the wheel (pans vertically). Upgrade: drag to pan.
        let (step_x, step_y) = (i64::from(g.vp.0 / 8).max(1), i64::from(g.vp.1 / 8).max(1));
        match code {
            KeyCode::Left | KeyCode::Char('h') => g.pan(-step_x, 0),
            KeyCode::Right | KeyCode::Char('l') => g.pan(step_x, 0),
            KeyCode::Up | KeyCode::Char('k') => g.pan(0, -step_y),
            KeyCode::Down | KeyCode::Char('j') => g.pan(0, step_y),
            KeyCode::PageUp => g.pan(0, -step_y * 7),
            KeyCode::PageDown => g.pan(0, step_y * 7),
            KeyCode::Home | KeyCode::Char('g') => g.centre.1 = 0.0,
            KeyCode::End | KeyCode::Char('G') => g.centre.1 = 1.0,
            KeyCode::Char('+' | '=') => g.set_zoom(g.zoom + 1),
            KeyCode::Char('-' | '_') => g.set_zoom(g.zoom.saturating_sub(1)),
            KeyCode::Char('a') => {
                g.actual = !g.actual;
                g.centre = (0.5, 0.5);
            }
            KeyCode::Char('0') => {
                g.set_zoom(0);
                g.actual = false;
                g.centre = (0.5, 0.5);
            }
            _ => {}
        }
    }

    fn cycle_view(&mut self) {
        let views = self.views();
        let at = views.iter().position(|&v| v == self.view).unwrap_or(0);
        self.set_view(views[(at + 1) % views.len()]);
    }

    /// Zoom percent.
    #[cfg(test)]
    #[must_use]
    pub fn zoom_pct(&self) -> u32 {
        self.gfx.as_ref().map_or(100, |g| ZOOMS[g.zoom])
    }

    fn draw_text(&self, frame: &mut Frame<'_>, body: Rect, theme: &Theme, note: Option<&str>) {
        let mut lines: Vec<Line> = note.map(Line::from).into_iter().collect();
        lines.extend(self.text.iter().map(|l| Line::from(l.as_str())));
        let rows = usize::from(body.height);
        let shown: Vec<Line> = lines.into_iter().skip(self.scroll).take(rows).collect();
        frame.render_widget(Paragraph::new(shown).style(theme.text()), body);
    }

    fn scroll_text(&mut self, rows: isize) {
        let last = self.text.len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add_signed(rows).min(last);
    }
}

impl ModalContent for ImageViewer {
    fn label(&self) -> &'static str {
        if self.is_diagram() {
            "DIAGRAM"
        } else {
            "IMAGE"
        }
    }

    fn want(&mut self, cap: (u16, u16)) -> (u16, u16) {
        if let Some(g) = self.gfx.as_mut() {
            let font = g.picker.font_size();
            let (fw, fh) = (u32::from(font.width.max(1)), u32::from(font.height.max(1)));
            g.sync((u32::from(cap.0) * fw, u32::from(cap.1) * fh));
            let cells = |px: u32, f: u32| u16::try_from(px.div_ceil(f)).unwrap_or(u16::MAX);
            return match (&g.zoomed, &g.error) {
                (Some(z), None) => (cells(z.img.width(), fw), cells(z.img.height(), fh)),
                (None, None) => (0, 1), // "rendering…"
                (_, Some(_)) => cap,
            };
        }
        if self.view == View::Text && self.text_w != cap.0 {
            self.text_w = cap.0;
            if let Some(SlotSource::Mermaid { source, .. }) = &self.item().src {
                self.text = diagram_lines(source, DiagramTier::Text, cap.0).0;
            }
        }
        let w = self.text.iter().map(|l| line_width(l)).max().unwrap_or(0);
        (w, u16::try_from(self.text.len()).unwrap_or(u16::MAX))
    }

    fn title(&self) -> String {
        let item = self.item();
        let mut parts = vec![if item.alt.is_empty() {
            self.label().to_lowercase()
        } else {
            item.alt.clone()
        }];
        if self.items.len() > 1 {
            parts.push(format!("{}/{}", self.cur + 1, self.items.len()));
        }
        match &self.gfx {
            Some(g) => {
                parts.push(format!(
                    "{} {}%",
                    if g.actual { "actual" } else { "fit" },
                    ZOOMS[g.zoom]
                ));
                if let Some(z) = &g.zoomed {
                    parts.push(format!("{}×{} px", z.img.width(), z.img.height()));
                }
            }
            None if self.is_diagram() => parts.push(format!("{} view", self.view.name())),
            None => {}
        }
        parts.join(" · ")
    }

    fn hint(&self) -> String {
        let mut h = match &self.gfx {
            Some(g) => format!(
                "←↑↓→ pan  +/- zoom  0 fit  a {}  g/G top/end",
                if g.actual { "fit" } else { "actual" }
            ),
            None => "↑↓ scroll".into(),
        };
        if self.items.len() > 1 {
            h.push_str("  Tab/⇧Tab item");
        }
        if self.views().len() > 1 {
            h.push_str("  v image/text/source");
        }
        h.push_str("  Esc close");
        if self.gfx.is_none() {
            let why = if self.picker.is_none() {
                "; no graphics protocol"
            } else {
                ""
            };
            let _ = write!(h, "  ({} view{why})", self.view.name());
        }
        h
    }

    fn busy(&self) -> bool {
        self.gfx.as_ref().is_some_and(|g| {
            g.error.is_none()
                && g.asked
                    .is_none_or(|k| g.zoomed.as_ref().map(|z| z.key) != Some(k))
        })
    }

    fn key(&mut self, key: KeyEvent) -> ModalEvent {
        match key.code {
            KeyCode::Char('q') => return ModalEvent::Close,
            KeyCode::BackTab => self.step(false),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.step(false),
            KeyCode::Tab => self.step(true),
            KeyCode::Char('v') => self.cycle_view(),
            _ => {
                self.pan_or_scroll(key.code);
                return ModalEvent::Stay;
            }
        }
        ModalEvent::Stay
    }

    fn draw(&mut self, frame: &mut Frame<'_>, body: Rect, theme: &Theme) {
        let Some(g) = self.gfx.as_mut() else {
            self.draw_text(frame, body, theme, None);
            return;
        };
        let font = g.picker.font_size();
        let (fw, fh) = (u32::from(font.width.max(1)), u32::from(font.height.max(1)));
        g.vp = (u32::from(body.width) * fw, u32::from(body.height) * fh);
        if let Some(err) = g.error.clone() {
            // The diagram source is the next best thing to the picture.
            self.draw_text(frame, body, theme, Some(&format!("cannot render: {err}")));
            return;
        }
        let Some(z) = &g.zoomed else {
            frame.render_widget(Paragraph::new("rendering…").style(theme.muted()), body);
            return;
        };
        let (ox, oy) = g.offset();
        let (cw, ch) = (
            z.img.width().min(g.vp.0).max(1),
            z.img.height().min(g.vp.1).max(1),
        );
        let cols = u16::try_from(cw.div_ceil(fw))
            .unwrap_or(u16::MAX)
            .min(body.width);
        let rows = u16::try_from(ch.div_ceil(fh))
            .unwrap_or(u16::MAX)
            .min(body.height);
        let id = (ox, oy, z.img.width(), cols, rows);
        if g.shown.as_ref().is_none_or(|(shown, _)| *shown != id) {
            let mut canvas = RgbaImage::new(u32::from(cols) * fw, u32::from(rows) * fh);
            if let Some((r, g2, b)) = g.pad {
                canvas
                    .pixels_mut()
                    .for_each(|p| *p = image::Rgba([r, g2, b, 255]));
            }
            let window = image::imageops::crop_imm(&z.img, ox, oy, cw, ch).to_image();
            image::imageops::overlay(&mut canvas, &window, 0, 0);
            let protocol = g
                .picker
                .new_resize_protocol(DynamicImage::ImageRgba8(canvas));
            g.shown = Some((id, protocol));
        }
        if let Some((_, protocol)) = g.shown.as_mut() {
            let area = Rect {
                width: cols,
                height: rows,
                ..body
            };
            // Drawn inside the shell's panel, i.e. after its `Clear` (ADR-0004 order).
            frame.render_stateful_widget(StatefulImage::default(), area, protocol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use wiki_reader_render::DiagramPalette;

    fn press(v: &mut ImageViewer, c: KeyCode) {
        v.key(KeyEvent::new(c, KeyModifiers::NONE));
    }

    fn mermaid() -> SlotSource {
        SlotSource::Mermaid {
            hash: 1,
            palette: DiagramPalette::default(),
            source: "flowchart LR\n  A[Build] --> B[Deploy]\n".into(),
        }
    }

    fn item(id: u64, src: Option<SlotSource>) -> ViewerItem {
        ViewerItem {
            alt: format!("item{id}"),
            src,
            note: "pic.png: no preview".into(),
        }
    }

    fn one(picker: Option<Picker>) -> ImageViewer {
        ImageViewer::new(vec![item(1, Some(mermaid()))], 0, picker)
    }

    #[test]
    fn zoom_clamps_at_both_ends() {
        let mut v = one(Some(Picker::halfblocks()));
        press(&mut v, KeyCode::Char('-'));
        assert_eq!(v.zoom_pct(), 100, "cannot go below fit");
        for _ in 0..20 {
            press(&mut v, KeyCode::Char('+'));
        }
        assert_eq!(v.zoom_pct(), 800, "cannot go above the top step");
        press(&mut v, KeyCode::Char('0'));
        assert_eq!(v.zoom_pct(), 100);
    }

    #[test]
    fn pixel_cap_holds_at_every_zoom_and_mode() {
        // A picture already near the cap, in a huge window, at the top zoom.
        for nat in [(4000, 4000), (16_000, 1000), (100, 100), (1, 1)] {
            for zoom in ZOOMS {
                for actual in [false, true] {
                    let (w, h) = zoom_dims(nat, (20_000, 20_000), zoom, actual);
                    assert!(
                        u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS,
                        "{nat:?} {zoom} {actual}"
                    );
                }
            }
        }
        // Fit enlarges a small picture (2×) and shrinks a big one into the window;
        // actual size is natural pixels whatever the window.
        assert_eq!(zoom_dims((100, 50), (4000, 4000), 100, false), (200, 100));
        assert_eq!(zoom_dims((100, 50), (4000, 4000), 100, true), (100, 50));
        assert_eq!(zoom_dims((100, 50), (40, 40), 100, true), (100, 50));
        let (w, h) = zoom_dims((1000, 500), (200, 200), 100, false);
        assert!(w <= 200 && h <= 200);
        // Zoom multiplies the base of either mode.
        assert_eq!(zoom_dims((100, 50), (4000, 4000), 200, true), (200, 100));
    }

    #[test]
    fn pan_clamps_inside_the_picture() {
        let mut v = one(Some(Picker::halfblocks()));
        let g = v.gfx.as_mut().unwrap();
        g.vp = (100, 100);
        g.zoomed = Some(Zoomed {
            img: RgbaImage::new(300, 200),
            key: (200, (100, 100), false),
        });
        g.pan(10_000, 10_000);
        assert_eq!(g.offset(), (200, 100), "right/bottom edge");
        g.pan(-5, 0);
        assert_eq!(g.offset().0, 195, "reversing responds at once");
        g.pan(-10_000, -10_000);
        assert_eq!(g.offset(), (0, 0));
    }

    #[test]
    fn text_tier_shows_source_and_scrolls() {
        let mut v = one(None);
        assert!(!v.busy());
        press(&mut v, KeyCode::Down);
        assert_eq!(v.scroll, 1);
        press(&mut v, KeyCode::Up);
        press(&mut v, KeyCode::Up);
        assert_eq!(v.scroll, 0);
        assert!(v.hint().contains("source view"));
        // The window wants the source's size, not the whole screen.
        assert_eq!(v.want((90, 30)), (24, 2));
    }

    #[test]
    fn tab_steps_the_inventory_and_v_cycles_only_available_views() {
        let items = vec![
            item(1, Some(mermaid())),
            item(2, None),
            item(3, Some(mermaid())),
        ];
        let mut v = ImageViewer::new(items, 2, Some(Picker::halfblocks()));
        assert_eq!((v.cur, v.view), (2, View::Image), "opened one shows first");
        press(&mut v, KeyCode::Tab);
        assert_eq!(v.cur, 0, "wraps forward");
        v.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(v.cur, 2, "Shift+Tab wraps back");
        v.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(v.cur, 1);
        // A picture with nothing to draw has one view; `v` is inert and the note shows.
        assert_eq!((v.view, v.views().len()), (View::Source, 1));
        press(&mut v, KeyCode::Char('v'));
        assert_eq!(v.view, View::Source);
        assert!(v.gfx.is_none() && v.text == ["pic.png: no preview"]);
        // A diagram cycles image → text → source → image, and the text art is drawn.
        press(&mut v, KeyCode::Tab);
        let seen: Vec<_> = (0..4)
            .map(|_| {
                let at = v.view;
                press(&mut v, KeyCode::Char('v'));
                at
            })
            .collect();
        assert_eq!(seen, [View::Image, View::Text, View::Source, View::Image]);
        // Without a graphics protocol the image view is skipped: opens on source, then text.
        let mut v = one(None);
        assert_eq!(
            (v.view, v.views()),
            (View::Source, vec![View::Text, View::Source])
        );
        press(&mut v, KeyCode::Char('v'));
        assert_eq!(v.view, View::Text);
        v.want((80, 20));
        assert!(v.text.iter().any(|l| l.contains("Build")), "{:?}", v.text);
    }

    #[test]
    fn worker_rerasterises_svg_sharper_at_higher_zoom() {
        let mut v = one(Some(Picker::halfblocks()));
        let g = v.gfx.as_mut().unwrap();
        let wait = |g: &mut Gfx, vp| {
            g.sync(vp);
            let deadline = Instant::now() + Duration::from_secs(10);
            while g.zoomed.as_ref().map(|z| z.key) != g.asked {
                assert!(Instant::now() < deadline, "worker timed out");
                std::thread::sleep(Duration::from_millis(10));
                g.sync(vp);
            }
            g.zoomed.as_ref().unwrap().img.width()
        };
        let vp = (4000, 4000);
        let fit = wait(g, vp);
        g.set_zoom(2);
        let big = wait(g, vp);
        assert!(big > fit, "{big} vs {fit}");
        // Actual size is the natural SVG size, smaller than the enlarged fit.
        g.set_zoom(0);
        g.actual = true;
        let actual = wait(g, vp);
        assert!(actual < fit, "{actual} vs {fit}");
    }
}
