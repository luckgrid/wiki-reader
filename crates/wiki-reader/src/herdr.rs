//! Talking to herdr from the reader binary (std only, never on the UI thread).
//!
//! herdr 0.9.x starts plugin panes (overlay, popup, split, tab) without cell metrics, so they
//! cannot draw terminal graphics. [`open_split`] therefore opens the reader in an *ordinary*
//! shell pane, which does answer the terminal's queries.
//!
//! [`Publisher`] shows the page being read in herdr's sidebar (P3-10): display-only pane
//! metadata, sent from a worker thread, never touching navigation.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wiki_reader_core::herdr::{parse_context, parse_split_pane_id};

/// What the shell pane runs. `exec` replaces the shell, so quitting the reader closes the pane.
const RUN_COMMAND: &str = "exec wiki-reader";

/// How the launcher reaches herdr. Real use runs the CLI; tests record the calls.
pub trait Herdr: Send + Sync {
    /// Run `herdr <args>`: stdout on success, a one-line message on failure.
    fn run(&self, args: &[String]) -> Result<String, String>;
}

/// The `herdr` CLI: `$HERDR_BIN_PATH` (set for plugin commands and herdr panes), else `herdr`.
pub struct HerdrCli {
    bin: PathBuf,
}

impl HerdrCli {
    #[must_use]
    pub fn from_env() -> Self {
        let bin = std::env::var_os("HERDR_BIN_PATH")
            .filter(|path| !path.is_empty())
            .map_or_else(|| PathBuf::from("herdr"), PathBuf::from);
        Self { bin }
    }
}

impl Herdr for HerdrCli {
    fn run(&self, args: &[String]) -> Result<String, String> {
        let output = Command::new(&self.bin)
            .args(args)
            .output()
            .map_err(|err| format!("cannot run {}: {err}", self.bin.display()))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.lines().find(|line| !line.trim().is_empty());
        Err(format!(
            "herdr {} failed: {}",
            args.iter().take(2).cloned().collect::<Vec<_>>().join(" "),
            message.unwrap_or("no error message")
        ))
    }
}

/// Open the reader in a new ordinary split pane to the right of the focused pane, in that
/// pane's cwd, focus it, and return the new pane's id.
///
/// `context_json` is `HERDR_PLUGIN_CONTEXT_JSON`. A missing or malformed context is not an
/// error: herdr then splits its own focused pane and applies its `terminal.new_cwd` policy.
pub fn open_split(herdr: &dyn Herdr, context_json: Option<&str>) -> Result<String, String> {
    let context = context_json.and_then(parse_context).unwrap_or_default();
    let mut split: Vec<String> = vec!["pane".into(), "split".into()];
    if let Some(pane) = &context.focused_pane_id {
        split.push(pane.clone());
    }
    split.extend(["--direction".into(), "right".into()]);
    if let Some(cwd) = context.cwd.as_ref().filter(|cwd| cwd.is_dir()) {
        split.extend(["--cwd".into(), cwd.to_string_lossy().into_owned()]);
    }
    split.push("--focus".into());
    let response = herdr.run(&split)?;
    let pane = parse_split_pane_id(&response)
        .ok_or_else(|| "herdr pane split returned no pane id".to_owned())?;
    herdr
        .run(&[
            "pane".into(),
            "run".into(),
            pane.clone(),
            RUN_COMMAND.into(),
        ])
        .map_err(|err| format!("{err} (the new pane {pane} was left open)"))?;
    Ok(pane)
}

// ── Publishing the current page (P3-10) ───────────────────────────────────────────────────────

/// The `--source` herdr keys our metadata by.
const SOURCE: &str = "wiki-reader";
/// herdr caps titles and token values at 80 characters after its own normalisation.
const MAX_TEXT: usize = 80;

/// When the publisher sends.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Quiet time after the last page change before sending, so a burst (holding a key,
    /// stepping prev/next) collapses to its last page.
    pub debounce: Duration,
    /// How often the current page is re-sent so it does not expire while the reader runs.
    pub renew: Duration,
    /// How long herdr keeps the metadata after the last report, in milliseconds. If the reader
    /// dies without clearing it, it disappears after this.
    pub ttl_ms: u64,
    /// Consecutive failures after which publishing stops for the session.
    pub max_failures: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(250),
            renew: Duration::from_secs(240),
            ttl_ms: 600_000,
            max_failures: 3,
        }
    }
}

/// The pane to publish to: `HERDR_PANE_ID`, when publishing is enabled and the process runs
/// inside herdr. Plugin popups have no pane id of their own and are left out on purpose: the
/// pane id herdr reports for them would belong to an unrelated pane.
#[must_use]
pub fn target_pane(
    enabled: bool,
    herdr_env: Option<&str>,
    pane_id: Option<&str>,
) -> Option<String> {
    if !enabled || herdr_env != Some("1") {
        return None;
    }
    pane_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// `text` without control characters or surrounding space, at most [`MAX_TEXT`] characters,
/// keeping the start (`…` replaces the cut end).
fn clean_head(text: &str) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim();
    if text.chars().count() <= MAX_TEXT {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MAX_TEXT - 1).collect();
    format!("{kept}…")
}

/// Like [`clean_head`] but keeps the end of a long path (the file name is what matters).
fn clean_tail(text: &str) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim();
    let count = text.chars().count();
    if count <= MAX_TEXT {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(count - (MAX_TEXT - 1)).collect();
    format!("…{kept}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Page {
    title: String,
    path: String,
}

enum Msg {
    Page(Page),
    Stop,
}

/// Sends the page being read to herdr as display-only pane metadata: a title and a `page`
/// token, with a TTL that is renewed while the reader runs and cleared on exit.
///
/// All herdr calls happen on a worker thread. A missing binary or a failing call never reaches
/// the caller; after [`Timing::max_failures`] in a row the worker stops sending. It never
/// reports agent state, so the pane stays an ordinary pane.
pub struct Publisher {
    tx: Sender<Msg>,
    done: mpsc::Receiver<()>,
}

impl Publisher {
    /// A publisher for this process's herdr pane, or `None` outside herdr, in a popup, or when
    /// `enabled` is false.
    #[must_use]
    pub fn from_env(enabled: bool) -> Option<Self> {
        let env = std::env::var("HERDR_ENV").ok();
        let pane = std::env::var("HERDR_PANE_ID").ok();
        let pane = target_pane(enabled, env.as_deref(), pane.as_deref())?;
        Some(Self::spawn(
            Arc::new(HerdrCli::from_env()),
            pane,
            Timing::default(),
        ))
    }

    #[must_use]
    pub fn spawn(herdr: Arc<dyn Herdr>, pane: String, timing: Timing) -> Self {
        let (tx, rx) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        std::thread::spawn(move || {
            Worker::new(herdr, pane, timing).run(&rx);
            let _ = done_tx.send(());
        });
        Self { tx, done }
    }

    /// Queue the page now being read; returns at once.
    pub fn publish(&self, title: &str, path: &str) {
        let _ = self.tx.send(Msg::Page(Page {
            title: clean_head(title),
            path: clean_tail(path),
        }));
    }
}

impl Drop for Publisher {
    /// Clear the metadata, waiting only briefly so a stuck herdr cannot hold up exit.
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        let _ = self.done.recv_timeout(Duration::from_millis(1500));
    }
}

struct Worker {
    herdr: Arc<dyn Herdr>,
    pane: String,
    timing: Timing,
    /// A page waiting out the debounce, and when it is due.
    pending: Option<(Page, Instant)>,
    /// The page herdr currently shows, and when to renew it.
    shown: Option<(Page, Instant)>,
    last_seq: u64,
    failures: u32,
    stopped: bool,
}

impl Worker {
    fn new(herdr: Arc<dyn Herdr>, pane: String, timing: Timing) -> Self {
        Self {
            herdr,
            pane,
            timing,
            pending: None,
            shown: None,
            last_seq: 0,
            failures: 0,
            stopped: false,
        }
    }

    fn run(&mut self, rx: &mpsc::Receiver<Msg>) {
        loop {
            let wake = [
                self.pending.as_ref().map(|(_, due)| *due),
                self.shown.as_ref().map(|(_, renew)| *renew),
            ]
            .into_iter()
            .flatten()
            .min();
            let msg = match wake {
                Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match msg {
                Ok(Msg::Page(page)) if !self.stopped => {
                    self.pending = Some((page, Instant::now() + self.timing.debounce));
                }
                Ok(Msg::Page(_)) => {}
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    self.clear();
                    return;
                }
                Err(RecvTimeoutError::Timeout) => self.on_timer(),
            }
        }
    }

    fn on_timer(&mut self) {
        let now = Instant::now();
        if let Some((page, due)) = self.pending.take() {
            if due > now {
                self.pending = Some((page, due));
            } else if self.shown.as_ref().is_none_or(|(shown, _)| *shown != page) {
                self.report(&page);
            }
        }
        if let Some((page, renew)) = self.shown.clone()
            && renew <= now
        {
            self.report(&page);
        }
    }

    /// The next `--seq`: Unix time in ms, so a restarted reader is not ignored as stale, and
    /// strictly increasing within this process.
    fn next_seq(&mut self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        self.last_seq = now.max(self.last_seq + 1);
        self.last_seq.to_string()
    }

    fn base_args(&mut self) -> Vec<String> {
        let seq = self.next_seq();
        vec![
            "pane".into(),
            "report-metadata".into(),
            self.pane.clone(),
            "--source".into(),
            SOURCE.into(),
            "--seq".into(),
            seq,
        ]
    }

    fn report(&mut self, page: &Page) {
        // `--name=value` so a title that starts with `-` is never read as a flag.
        let mut args = self.base_args();
        args.extend([
            format!("--title={}", page.title),
            format!("--token=page={}", page.path),
            "--ttl-ms".into(),
            self.timing.ttl_ms.to_string(),
        ]);
        if self.herdr.run(&args).is_ok() {
            self.failures = 0;
            self.shown = Some((page.clone(), Instant::now() + self.timing.renew));
        } else {
            self.failures += 1;
            if self.failures >= self.timing.max_failures {
                self.stopped = true;
                self.pending = None;
                self.shown = None;
            }
        }
    }

    /// Remove what we published. Best effort: the TTL removes it if this fails.
    fn clear(&mut self) {
        if self.shown.is_none() {
            return;
        }
        let mut args = self.base_args();
        args.extend([
            "--clear-title".into(),
            "--clear-token".into(),
            "page".into(),
        ]);
        let _ = self.herdr.run(&args);
        self.shown = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Replies in order and records every call.
    struct Fake {
        calls: Mutex<Vec<Vec<String>>>,
        replies: Mutex<Vec<Result<String, String>>>,
    }

    impl Fake {
        fn new(replies: Vec<Result<String, String>>) -> Self {
            Self {
                calls: Mutex::default(),
                replies: Mutex::new(replies.into_iter().rev().collect()),
            }
        }
        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Herdr for Fake {
        fn run(&self, args: &[String]) -> Result<String, String> {
            self.calls.lock().unwrap().push(args.to_vec());
            self.replies.lock().unwrap().pop().expect("unexpected call")
        }
    }

    const SPLIT_OK: &str = r#"{"id":"cli:pane:split","result":{"pane":{"pane_id":"w1:p9"}}}"#;

    #[test]
    fn splits_the_focused_pane_in_its_cwd_then_runs_the_reader() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().into_owned();
        let context = format!(r#"{{"focused_pane_id":"w1:p2","focused_pane_cwd":"{cwd}"}}"#);
        let fake = Fake::new(vec![Ok(SPLIT_OK.into()), Ok(String::new())]);
        assert_eq!(open_split(&fake, Some(&context)).as_deref(), Ok("w1:p9"));
        assert_eq!(
            fake.calls(),
            [
                vec![
                    "pane",
                    "split",
                    "w1:p2",
                    "--direction",
                    "right",
                    "--cwd",
                    &cwd,
                    "--focus"
                ],
                vec!["pane", "run", "w1:p9", "exec wiki-reader"],
            ]
        );
    }

    #[test]
    fn paths_with_spaces_stay_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let spaced = dir.path().join("a collection; rm -rf");
        std::fs::create_dir(&spaced).unwrap();
        let context = format!(r#"{{"focused_pane_cwd":"{}"}}"#, spaced.display());
        let fake = Fake::new(vec![Ok(SPLIT_OK.into()), Ok(String::new())]);
        open_split(&fake, Some(&context)).unwrap();
        let split = &fake.calls()[0];
        let at = split.iter().position(|a| a == "--cwd").unwrap();
        assert_eq!(split[at + 1], spaced.to_string_lossy());
    }

    #[test]
    fn missing_or_unusable_context_lets_herdr_choose_the_target_and_cwd() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let not_a_dir = format!(r#"{{"focused_pane_cwd":"{}"}}"#, file.path().display());
        for context in [None, Some("not json"), Some("{}"), Some(not_a_dir.as_str())] {
            let fake = Fake::new(vec![Ok(SPLIT_OK.into()), Ok(String::new())]);
            open_split(&fake, context).unwrap();
            assert_eq!(
                fake.calls()[0],
                ["pane", "split", "--direction", "right", "--focus"],
                "{context:?}"
            );
        }
    }

    #[test]
    fn split_failure_or_a_response_without_a_pane_id_stops_before_running_anything() {
        let fake = Fake::new(vec![Err("herdr pane split failed: no workspace".into())]);
        assert_eq!(
            open_split(&fake, None),
            Err("herdr pane split failed: no workspace".into())
        );
        assert_eq!(fake.calls().len(), 1);
        for response in ["", "{}", r#"{"result":{"pane":{}}}"#] {
            let fake = Fake::new(vec![Ok(response.into())]);
            assert_eq!(
                open_split(&fake, None),
                Err("herdr pane split returned no pane id".into())
            );
            assert_eq!(fake.calls().len(), 1, "{response}");
        }
    }

    #[test]
    fn run_failure_names_the_pane_that_was_left_open() {
        let fake = Fake::new(vec![
            Ok(SPLIT_OK.into()),
            Err("herdr pane run failed: gone".into()),
        ]);
        let err = open_split(&fake, None).unwrap_err();
        assert!(err.contains("herdr pane run failed: gone"), "{err}");
        assert!(err.contains("w1:p9"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn cli_reports_spawn_and_exit_failures() {
        use std::os::unix::fs::PermissionsExt;
        let missing = HerdrCli {
            bin: PathBuf::from("/nope-wiki-reader-missing/herdr"),
        };
        assert!(
            missing
                .run(&["pane".into()])
                .unwrap_err()
                .starts_with("cannot run")
        );

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("herdr");
        std::fs::write(&script, "#!/bin/sh\necho 'boom: bad pane' >&2\nexit 3\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let failing = HerdrCli { bin: script };
        assert_eq!(
            failing.run(&["pane".into(), "split".into()]).unwrap_err(),
            "herdr pane split failed: boom: bad pane"
        );
    }
    // ── Publisher ────────────────────────────────────────────────────────────────────────────

    /// Records every call, optionally failing them all.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<Vec<String>>>,
        fail: std::sync::atomic::AtomicBool,
    }

    impl Recorder {
        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Herdr for Recorder {
        fn run(&self, args: &[String]) -> Result<String, String> {
            self.calls.lock().unwrap().push(args.to_vec());
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
                Err("herdr pane report-metadata failed: gone".into())
            } else {
                Ok(String::new())
            }
        }
    }

    fn fast() -> Timing {
        Timing {
            debounce: Duration::from_millis(30),
            renew: Duration::from_secs(60),
            ttl_ms: 4242,
            max_failures: 3,
        }
    }

    fn publisher(timing: Timing) -> (Publisher, Arc<Recorder>) {
        let recorder = Arc::new(Recorder::default());
        let herdr: Arc<dyn Herdr> = Arc::clone(&recorder) as Arc<dyn Herdr>;
        (Publisher::spawn(herdr, "w1:p2".into(), timing), recorder)
    }

    fn wait_for(recorder: &Recorder, calls: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while recorder.calls().len() < calls {
            assert!(
                Instant::now() < deadline,
                "expected {calls} calls: {:?}",
                recorder.calls()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn seq(call: &[String]) -> u64 {
        let at = call.iter().position(|a| a == "--seq").unwrap();
        call[at + 1].parse().unwrap()
    }

    #[test]
    fn publishes_only_inside_herdr_with_its_own_pane_and_when_enabled() {
        assert_eq!(
            target_pane(true, Some("1"), Some(" w1:p2 ")).as_deref(),
            Some("w1:p2")
        );
        assert_eq!(
            target_pane(false, Some("1"), Some("w1:p2")),
            None,
            "opted out"
        );
        assert_eq!(
            target_pane(true, None, Some("w1:p2")),
            None,
            "outside herdr"
        );
        assert_eq!(target_pane(true, Some("0"), Some("w1:p2")), None);
        assert_eq!(
            target_pane(true, Some("1"), None),
            None,
            "a plugin popup has no pane"
        );
        assert_eq!(target_pane(true, Some("1"), Some("  ")), None);
    }

    #[test]
    fn text_is_cleaned_and_capped() {
        assert_eq!(clean_head("  a\u{1b}[0m\tb\n "), "a[0mb");
        let long = "x".repeat(200);
        let head = clean_head(&long);
        assert_eq!(head.chars().count(), MAX_TEXT);
        assert!(head.ends_with('…'));
        let path = format!("{}/guide.md", "d".repeat(200));
        let tail = clean_tail(&path);
        assert_eq!(tail.chars().count(), MAX_TEXT);
        assert!(tail.starts_with('…') && tail.ends_with("/guide.md"));
        assert_eq!(clean_tail("a/b.md"), "a/b.md");
    }

    #[test]
    fn sends_the_page_as_title_and_token_with_a_ttl() {
        let (publisher, recorder) = publisher(fast());
        publisher.publish("-Starts with a dash", "wiki/guides/README.md");
        wait_for(&recorder, 1);
        let call = &recorder.calls()[0];
        assert_eq!(
            &call[..5],
            [
                "pane",
                "report-metadata",
                "w1:p2",
                "--source",
                "wiki-reader"
            ]
        );
        assert_eq!(
            &call[7..],
            [
                "--title=-Starts with a dash",
                "--token=page=wiki/guides/README.md",
                "--ttl-ms",
                "4242"
            ]
        );
        assert_eq!(call[5], "--seq");
        assert!(
            seq(call) > 1_700_000_000_000,
            "Unix time in ms, so restarts stay ahead"
        );
        assert!(
            !call.iter().any(|a| a.contains("agent")),
            "never reports agent state: {call:?}"
        );
    }

    #[test]
    fn a_burst_of_page_changes_sends_only_the_last() {
        let (publisher, recorder) = publisher(fast());
        for n in 0..6 {
            publisher.publish(&format!("Page {n}"), &format!("p{n}.md"));
        }
        wait_for(&recorder, 1);
        std::thread::sleep(Duration::from_millis(150));
        let calls = recorder.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert!(calls[0].contains(&"--title=Page 5".to_owned()));
    }

    #[test]
    fn the_same_page_is_not_sent_twice_but_a_new_one_is() {
        let (publisher, recorder) = publisher(fast());
        publisher.publish("A", "a.md");
        wait_for(&recorder, 1);
        publisher.publish("A", "a.md");
        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(recorder.calls().len(), 1, "unchanged page");
        publisher.publish("B", "b.md");
        wait_for(&recorder, 2);
        assert!(recorder.calls()[1].contains(&"--title=B".to_owned()));
    }

    #[test]
    fn the_page_is_renewed_with_increasing_sequence_numbers() {
        let (publisher, recorder) = publisher(Timing {
            renew: Duration::from_millis(40),
            ..fast()
        });
        publisher.publish("A", "a.md");
        wait_for(&recorder, 4);
        let calls = recorder.calls();
        for pair in calls.windows(2) {
            assert_eq!(&pair[0][7..], &pair[1][7..], "same page each time");
            assert!(seq(&pair[0]) < seq(&pair[1]), "{pair:?}");
        }
    }

    #[test]
    fn dropping_clears_what_was_published() {
        let (publisher, recorder) = publisher(fast());
        publisher.publish("A", "a.md");
        wait_for(&recorder, 1);
        drop(publisher);
        let calls = recorder.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        let clear = &calls[1];
        assert_eq!(&clear[7..], ["--clear-title", "--clear-token", "page"]);
        assert!(seq(clear) > seq(&calls[0]), "the clear is not stale");
    }

    #[test]
    fn nothing_is_cleared_when_nothing_was_published() {
        let (publisher, recorder) = publisher(fast());
        drop(publisher);
        assert!(recorder.calls().is_empty());
        let (publisher, recorder) = publisher_with_failures();
        publisher.publish("A", "a.md");
        wait_for(&recorder, 1);
        drop(publisher);
        assert_eq!(recorder.calls().len(), 1, "a failed report is not cleared");
    }

    fn publisher_with_failures() -> (Publisher, Arc<Recorder>) {
        let (publisher, recorder) = publisher(Timing {
            max_failures: 3,
            ..fast()
        });
        recorder
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);
        (publisher, recorder)
    }

    #[test]
    fn repeated_failures_stop_publishing_without_blocking_anything() {
        let (publisher, recorder) = publisher_with_failures();
        for n in 0..6 {
            publisher.publish(&format!("Page {n}"), &format!("p{n}.md"));
            // Let each one go out before the next arrives.
            std::thread::sleep(Duration::from_millis(100));
        }
        let started = Instant::now();
        drop(publisher);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(recorder.calls().len(), 3, "gave up after max_failures");
    }
}
