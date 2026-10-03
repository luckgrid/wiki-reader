//! Image and diagram viewer (P3-15): a [`ModalContent`] over one picture.
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

use std::sync::mpsc::{self, Receiver, Sender};

use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui_image::StatefulImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use wiki_reader_core::images::{MAX_IMAGE_FILE_BYTES, MAX_IMAGE_PIXELS};
use wiki_reader_render::{SlotSource, mermaid_svg_bytes, rasterise_svg_scaled, svg_natural_size};

use super::images::{decode_file, is_svg_path};
use super::modal_viewer::{ModalContent, ModalEvent};
use super::theme::Theme;

/// Zoom steps, percent of the fit size (100 = whole picture in the window).
const ZOOMS: [u32; 7] = [100, 150, 200, 300, 400, 600, 800];
const HINT: &str = "←↑↓→ pan  +/- zoom  0 fit  g/G top/end  Esc close";
const TEXT_HINT: &str = "↑↓ scroll  Esc close  (no graphics protocol: source view)";

/// Pixel size of the picture at `zoom_pct`: the fit size (never above natural) times the zoom,
/// then shrunk if needed so `w × h` stays within [`MAX_IMAGE_PIXELS`].
#[must_use]
pub fn zoom_dims(nat: (u32, u32), vp: (u32, u32), zoom_pct: u32) -> (u32, u32) {
    let (nw, nh) = (f64::from(nat.0.max(1)), f64::from(nat.1.max(1)));
    let fit = (f64::from(vp.0) / nw).min(f64::from(vp.1) / nh).min(1.0);
    let cap = (MAX_IMAGE_PIXELS as f64 / (nw * nh)).sqrt() * 0.999;
    let scale = (fit * f64::from(zoom_pct) / 100.0).min(cap);
    let dim = |n: f64| (n * scale).ceil().max(1.0) as u32;
    (dim(nw), dim(nh))
}

/// What a cropped picture was cut from: window offset, zoomed width, size in cells.
type ShownId = (u32, u32, u32, u16, u16);

/// One zoom request: percent and window size in pixels.
type ReqKey = (u32, (u32, u32));

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
    let (w, h) = zoom_dims(nat.size(), key.1, key.0);
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

/// Graphics half of the viewer.
struct Gfx {
    picker: Picker,
    reqs: Sender<ReqKey>,
    results: Receiver<Result<Zoomed, String>>,
    /// Index into [`ZOOMS`].
    zoom: usize,
    /// Window centre as a fraction of the zoomed picture, so zoom keeps what you look at.
    centre: (f64, f64),
    zoomed: Option<Zoomed>,
    asked: Option<ReqKey>,
    error: Option<String>,
    /// Window size in pixels, from the last draw (pan step).
    vp: (u32, u32),
    /// The cropped picture on screen, and what it was cut from.
    shown: Option<(ShownId, StatefulProtocol)>,
    /// Mermaid cards pad with their own background.
    pad: Option<(u8, u8, u8)>,
}

impl Gfx {
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
}

/// Open viewer state.
pub struct ImageViewer {
    label: &'static str,
    alt: String,
    gfx: Option<Gfx>,
    /// Source view: Mermaid text, or the reason there is nothing to show.
    text: Vec<String>,
    scroll: usize,
}

impl ImageViewer {
    /// `picker` is `None` on the text tier, which shows the source instead.
    #[must_use]
    pub fn new(source: SlotSource, alt: &str, picker: Option<Picker>) -> Self {
        let (label, text, pad) = match &source {
            SlotSource::Mermaid {
                source, palette, ..
            } => (
                "DIAGRAM",
                source.lines().map(str::to_owned).collect(),
                Some(palette.bg),
            ),
            SlotSource::File { path, .. } => (
                "IMAGE",
                vec![format!("{}: no graphics protocol", path.display())],
                None,
            ),
        };
        let gfx = picker.map(|picker| {
            let (reqs, req_rx) = mpsc::channel();
            let (out, results) = mpsc::channel();
            std::thread::spawn(move || work(&source, &req_rx, &out));
            Gfx {
                picker,
                reqs,
                results,
                zoom: 0,
                centre: (0.5, 0.5),
                zoomed: None,
                asked: None,
                error: None,
                vp: (640, 340),
                shown: None,
                pad,
            }
        });
        Self {
            label,
            alt: alt.to_owned(),
            gfx,
            text,
            scroll: 0,
        }
    }

    /// Zoom percent.
    #[cfg(test)]
    #[must_use]
    pub fn zoom_pct(&self) -> u32 {
        self.gfx.as_ref().map_or(100, |g| ZOOMS[g.zoom])
    }

    /// Ask for the zoomed picture if the window or zoom changed, and take finished ones.
    fn sync(g: &mut Gfx, vp: (u32, u32)) {
        g.vp = vp;
        let key = (ZOOMS[g.zoom], vp);
        if g.asked != Some(key) {
            g.asked = Some(key);
            // A closed worker shows up as an error on the next read; nothing to do here.
            let _ = g.reqs.send(key);
        }
        while let Ok(result) = g.results.try_recv() {
            match result {
                Ok(z) => {
                    g.zoomed = Some(z);
                    g.error = None;
                }
                Err(e) => g.error = Some(e),
            }
        }
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
        self.label
    }

    fn title(&self) -> String {
        let name = if self.alt.is_empty() {
            self.label.to_lowercase()
        } else {
            self.alt.clone()
        };
        match &self.gfx {
            Some(g) => match &g.zoomed {
                Some(z) => format!(
                    "{name} · {}% · {}×{} px",
                    ZOOMS[g.zoom],
                    z.img.width(),
                    z.img.height()
                ),
                None => format!("{name} · {}%", ZOOMS[g.zoom]),
            },
            None => name,
        }
    }

    fn hint(&self) -> String {
        (if self.gfx.is_some() { HINT } else { TEXT_HINT }).into()
    }

    fn busy(&self) -> bool {
        self.gfx.as_ref().is_some_and(|g| {
            g.error.is_none()
                && g.asked
                    .is_none_or(|k| g.zoomed.as_ref().map(|z| z.key) != Some(k))
        })
    }

    fn key(&mut self, key: KeyEvent) -> ModalEvent {
        if matches!(key.code, KeyCode::Char('q')) {
            return ModalEvent::Close;
        }
        let Some(g) = self.gfx.as_mut() else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.scroll_text(-1),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_text(1),
                KeyCode::PageUp => self.scroll_text(-10),
                KeyCode::PageDown => self.scroll_text(10),
                KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => self.scroll_text(isize::MAX),
                _ => {}
            }
            return ModalEvent::Stay;
        };
        // ponytail: keys only; mouse is the wheel (pans vertically). Upgrade: drag to pan.
        let (step_x, step_y) = (i64::from(g.vp.0 / 8).max(1), i64::from(g.vp.1 / 8).max(1));
        match key.code {
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
            KeyCode::Char('0') => {
                g.set_zoom(0);
                g.centre = (0.5, 0.5);
            }
            _ => {}
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
        Self::sync(g, (u32::from(body.width) * fw, u32::from(body.height) * fh));
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
    use ratatui::crossterm::event::KeyModifiers;
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

    #[test]
    fn zoom_clamps_at_both_ends() {
        let mut v = ImageViewer::new(mermaid(), "d", Some(Picker::halfblocks()));
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
    fn pixel_cap_holds_at_every_zoom() {
        // A picture already near the cap, in a huge window, at the top zoom.
        for nat in [(4000, 4000), (16_000, 1000), (100, 100), (1, 1)] {
            for zoom in ZOOMS {
                let (w, h) = zoom_dims(nat, (20_000, 20_000), zoom);
                assert!(
                    u64::from(w) * u64::from(h) <= MAX_IMAGE_PIXELS,
                    "{nat:?} {zoom}"
                );
            }
        }
        // 100 % never upscales past natural.
        assert_eq!(zoom_dims((100, 50), (4000, 4000), 100), (100, 50));
        // Fits the window at 100 %.
        let (w, h) = zoom_dims((1000, 500), (200, 200), 100);
        assert!(w <= 200 && h <= 200);
    }

    #[test]
    fn pan_clamps_inside_the_picture() {
        let mut v = ImageViewer::new(mermaid(), "d", Some(Picker::halfblocks()));
        let g = v.gfx.as_mut().unwrap();
        g.vp = (100, 100);
        g.zoomed = Some(Zoomed {
            img: RgbaImage::new(300, 200),
            key: (200, (100, 100)),
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
        let mut v = ImageViewer::new(mermaid(), "d", None);
        assert!(!v.busy());
        press(&mut v, KeyCode::Down);
        assert_eq!(v.scroll, 1);
        press(&mut v, KeyCode::Up);
        press(&mut v, KeyCode::Up);
        assert_eq!(v.scroll, 0);
        assert!(v.hint().contains("source view"));
    }

    #[test]
    fn worker_rerasterises_svg_sharper_at_higher_zoom() {
        let mut v = ImageViewer::new(mermaid(), "d", Some(Picker::halfblocks()));
        let g = v.gfx.as_mut().unwrap();
        let wait = |g: &mut Gfx, vp| {
            ImageViewer::sync(g, vp);
            let deadline = Instant::now() + Duration::from_secs(10);
            while g.zoomed.as_ref().map(|z| z.key) != g.asked {
                assert!(Instant::now() < deadline, "worker timed out");
                std::thread::sleep(Duration::from_millis(10));
                ImageViewer::sync(g, vp);
            }
            g.zoomed.as_ref().unwrap().img.width()
        };
        let vp = (4000, 4000);
        let fit = wait(g, vp);
        g.set_zoom(2);
        let big = wait(g, vp);
        assert!(big > fit, "{big} vs {fit}");
    }
}
