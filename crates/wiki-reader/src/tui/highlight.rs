//! Syntect markdown highlighting for raw view (line gutter applied at draw).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};

use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Highlighted span for one run of text.
#[derive(Debug, Clone)]
pub struct HlSpan {
    /// Ratatui style.
    pub style: Style,
    /// Text fragment.
    pub text: String,
}

fn syntax_set() -> &'static SyntaxSet {
    static PS: OnceLock<SyntaxSet> = OnceLock::new();
    PS.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static TS: OnceLock<ThemeSet> = OnceLock::new();
    TS.get_or_init(ThemeSet::load_defaults)
}

fn markdown_syntax(ps: &SyntaxSet) -> Option<&SyntaxReference> {
    ps.find_syntax_by_token("md")
        .or_else(|| ps.find_syntax_by_name("Markdown"))
}

/// The preset's syntect theme, or any bundled one if the name is unknown.
fn pick_theme<'a>(ts: &'a ThemeSet, name: &str) -> Option<&'a Theme> {
    ts.themes.get(name).or_else(|| ts.themes.values().next())
}

/// Highlight `source` as markdown. Falls back to plain lines on error.
///
/// Call off the UI thread — full-page syntect is ~370 ms / 50 KB (release) and
/// applies asynchronously; the UI paints plain text until the worker result lands.
#[cfg(test)]
#[must_use]
pub fn highlight_markdown(source: &str, theme_name: &str) -> Vec<Vec<HlSpan>> {
    highlight_markdown_until(source, theme_name, &|| false).unwrap_or_default()
}

/// Like [`highlight_markdown`], but checks `cancelled` before every line and returns `None`
/// as soon as it is true, so a superseded job stops using the CPU.
#[must_use]
pub fn highlight_markdown_until(
    source: &str,
    theme_name: &str,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Vec<HlSpan>>> {
    let ps = syntax_set();
    let ts = theme_set();
    let Some(syntax) = markdown_syntax(ps) else {
        return Some(plain(source));
    };
    let Some(theme) = pick_theme(ts, theme_name) else {
        return Some(plain(source));
    };
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = Vec::new();
    for line in LinesWithEndings::from(source) {
        if cancelled() {
            return None;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        match h.highlight_line(line, ps) {
            Ok(ranges) => {
                let spans: Vec<HlSpan> = ranges
                    .into_iter()
                    .map(|(sty, text)| HlSpan {
                        style: syn_to_ratatui(sty),
                        text: text.trim_end_matches(['\r', '\n']).to_owned(),
                    })
                    .collect();
                if spans.is_empty() {
                    out.push(vec![HlSpan {
                        style: Style::default(),
                        text: trimmed.to_owned(),
                    }]);
                } else {
                    out.push(spans);
                }
            }
            Err(_) => out.push(vec![HlSpan {
                style: Style::default(),
                text: trimmed.to_owned(),
            }]),
        }
    }
    if out.is_empty() {
        out.push(vec![HlSpan {
            style: Style::default(),
            text: String::new(),
        }]);
    }
    Some(out)
}

/// One raw-view highlight request.
struct Job {
    token: u64,
    source: String,
    theme: &'static str,
}

/// Shared between the UI thread and the worker: the newest job, and the token still wanted.
struct Mailbox {
    job: Mutex<Option<Job>>,
    wake: Condvar,
    /// Token of the request the UI still wants; a running job with another token stops.
    wanted: AtomicU64,
    closed: AtomicBool,
}

/// One persistent worker thread that highlights raw pages, newest request first.
///
/// A new request replaces any queued one (the old source is dropped, not highlighted), and the
/// running job notices it is superseded between lines. Rapid navigation therefore costs at most
/// one active job, not one thread and one page buffer per page visited.
pub struct HighlightWorker {
    mailbox: Arc<Mailbox>,
    results: Receiver<(u64, Vec<Vec<HlSpan>>)>,
}

impl HighlightWorker {
    #[must_use]
    pub fn spawn() -> Self {
        let mailbox = Arc::new(Mailbox {
            job: Mutex::new(None),
            wake: Condvar::new(),
            wanted: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        });
        let (tx, results) = mpsc::channel();
        let shared = Arc::clone(&mailbox);
        std::thread::spawn(move || {
            while let Some(job) = Self::next_job(&shared) {
                let wanted = || shared.wanted.load(Ordering::Relaxed) != job.token;
                // A panic skips this page (it stays plain) and the worker lives on.
                let Ok(Some(hl)) = crate::tui::worker::guarded("syntax highlight", || {
                    highlight_markdown_until(&job.source, job.theme, &wanted)
                }) else {
                    continue;
                };
                if tx.send((job.token, hl)).is_err() {
                    return;
                }
            }
        });
        Self { mailbox, results }
    }

    fn next_job(mailbox: &Mailbox) -> Option<Job> {
        let mut slot = mailbox.job.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if mailbox.closed.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(job) = slot.take() {
                return Some(job);
            }
            slot = mailbox
                .wake
                .wait(slot)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Ask for `source` to be highlighted as request `token`, superseding every earlier one.
    pub fn submit(&self, token: u64, source: String, theme: &'static str) {
        self.mailbox.wanted.store(token, Ordering::Relaxed);
        *self
            .mailbox
            .job
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Job {
            token,
            source,
            theme,
        });
        self.mailbox.wake.notify_one();
    }

    /// Nothing is wanted any more: drop the queued job and stop the running one.
    pub fn cancel(&self, token: u64) {
        self.mailbox.wanted.store(token, Ordering::Relaxed);
        self.mailbox
            .job
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }

    /// A finished result, if any: `(token, highlights)`.
    pub fn try_recv(&self) -> Result<(u64, Vec<Vec<HlSpan>>), mpsc::TryRecvError> {
        self.results.try_recv()
    }
}

impl Drop for HighlightWorker {
    fn drop(&mut self) {
        self.mailbox.closed.store(true, Ordering::Relaxed);
        self.mailbox.wake.notify_one();
    }
}

fn plain(source: &str) -> Vec<Vec<HlSpan>> {
    let mut lines: Vec<Vec<HlSpan>> = source
        .lines()
        .map(|l| {
            vec![HlSpan {
                style: Style::default(),
                text: l.to_owned(),
            }]
        })
        .collect();
    if lines.is_empty() {
        lines.push(vec![HlSpan {
            style: Style::default(),
            text: String::new(),
        }]);
    }
    lines
}

fn syn_to_ratatui(s: syntect::highlighting::Style) -> Style {
    let fg = s.foreground;
    let mut style = Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    if s.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn highlight_stops_as_soon_as_it_is_cancelled() {
        let source = "# Heading\n\ntext\n".repeat(500);
        let seen = std::cell::Cell::new(0usize);
        let out = highlight_markdown_until(&source, "base16-ocean.dark", &|| {
            seen.set(seen.get() + 1);
            seen.get() > 10
        });
        assert!(out.is_none(), "a cancelled job returns nothing");
        assert_eq!(
            seen.get(),
            11,
            "it stopped on the 11th line check, not at the end"
        );
        assert!(highlight_markdown_until(&source, "base16-ocean.dark", &|| false).is_some());
    }

    #[test]
    fn rapid_requests_leave_only_the_newest_result() {
        let worker = HighlightWorker::spawn();
        let source = "# Heading\n\nParagraph with `code` and **bold**.\n\n".repeat(3000);
        for token in 1..=20 {
            worker.submit(token, source.clone(), "base16-ocean.dark");
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        let mut tokens = Vec::new();
        while !tokens.contains(&20) {
            assert!(Instant::now() < deadline, "newest request never finished");
            while let Ok((token, _)) = worker.try_recv() {
                tokens.push(token);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            tokens.len() < 20,
            "superseded jobs were dropped or aborted, got {tokens:?}"
        );
    }

    #[test]
    fn cancel_drops_the_queued_job_and_the_worker_stays_usable() {
        let worker = HighlightWorker::spawn();
        worker.submit(1, "# one\n".repeat(3000), "base16-ocean.dark");
        worker.cancel(2);
        worker.submit(3, "# three\n".into(), "base16-ocean.dark");
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        loop {
            assert!(Instant::now() < deadline, "no result for the later request");
            if let Ok((token, hl)) = worker.try_recv()
                && token == 3
            {
                assert!(!hl.is_empty());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn highlight_scaling_not_quadratic() {
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let small = chunk.repeat(350);
        let large = chunk.repeat(700);
        let _ = highlight_markdown("warm", "base16-ocean.dark");
        let t_small = {
            let start = Instant::now();
            assert!(!highlight_markdown(&small, "base16-ocean.dark").is_empty());
            start.elapsed()
        };
        let t_large = {
            let start = Instant::now();
            assert!(!highlight_markdown(&large, "base16-ocean.dark").is_empty());
            start.elapsed()
        };
        let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-9);
        assert!(
            ratio < 3.0,
            "doubling input slowed highlight {ratio:.2}× (small={t_small:?}, large={t_large:?})"
        );
    }

    /// UI path: `RawDoc` for ~50 KB stays well under syntect cost (no sync highlight).
    /// `cargo test -p wiki-reader-tui --release -- raw_load_budget --ignored`
    #[test]
    #[ignore = "release budget; run with --ignored --release"]
    fn raw_load_budget_50kb_under_50ms() {
        use crate::tui::viewer_doc::{RawDoc, ViewerDoc};
        let chunk =
            "# Heading\n\nParagraph with `code` and **bold**.\n\n```rust\nfn main() {}\n```\n\n";
        let source = chunk.repeat(700);
        assert!(source.len() >= 40_000);
        // Warm parse / allocator; highlight must stay unused on this path.
        let _ = RawDoc::from_source(chunk, None);
        let start = Instant::now();
        let doc = RawDoc::from_source(&source, None);
        let elapsed = start.elapsed();
        assert!(!doc.lines().is_empty());
        assert!(doc.highlights.is_empty(), "UI path must not sync-highlight");
        // ponytail: ~25 ms is sync markdown parse for 50 KB; syntect was ~367 ms.
        // Raise only if parse itself becomes the freeze; upgrade = parse off UI thread.
        assert!(
            elapsed.as_millis() < 50,
            "raw load took {elapsed:?}, expected < 50ms (release)"
        );
    }

    #[test]
    fn raw_doc_does_not_highlight_on_construct() {
        use crate::tui::viewer_doc::RawDoc;
        let doc = RawDoc::from_source("# Hi\n\npara\n", None);
        assert!(doc.highlights.is_empty());
    }
}
