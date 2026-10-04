//! Talking to herdr from the reader binary (std only, never on the UI thread).
//!
//! herdr 0.9.x starts plugin panes (overlay, popup, split, tab) without cell metrics, so they
//! cannot draw terminal graphics. [`open_split`] therefore opens the reader in an *ordinary*
//! shell pane, which does answer the terminal's queries.

use std::path::PathBuf;
use std::process::Command;

use wiki_reader_core::herdr::{parse_context, parse_split_pane_id};

/// What the shell pane runs. `exec` replaces the shell, so quitting the reader closes the pane.
const RUN_COMMAND: &str = "exec wiki-reader";

/// How the launcher reaches herdr. Real use runs the CLI; tests record the calls.
pub trait Herdr {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Replies in order and records every call.
    struct Fake {
        calls: RefCell<Vec<Vec<String>>>,
        replies: RefCell<Vec<Result<String, String>>>,
    }

    impl Fake {
        fn new(replies: Vec<Result<String, String>>) -> Self {
            Self {
                calls: RefCell::default(),
                replies: RefCell::new(replies.into_iter().rev().collect()),
            }
        }
        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }
    }

    impl Herdr for Fake {
        fn run(&self, args: &[String]) -> Result<String, String> {
            self.calls.borrow_mut().push(args.to_vec());
            self.replies.borrow_mut().pop().expect("unexpected call")
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
}
