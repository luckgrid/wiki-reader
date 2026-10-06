//! V22 regression check: an orphaned wiki-reader must exit, not spin.
//!
//! Usage: `check-orphan-exit [path/to/wiki-reader] [collection-root]`
//!
//! Runs the binary on a pseudo-terminal and checks that it exits by itself (an exit code, not a
//! crash signal) when
//!   (i)    the PTY master closes while the parent stays alive and the slave stays open (the
//!          original ~100 %CPU spin in crossterm's poll/read),
//!   (i')   the same, with the master closed while the app is still starting up (a closed tab),
//!   (ii)   its session leader (the shell that owns the session) is killed,
//!   (ii')  the same with `WIKI_READER_NO_WATCHDOG=1`, where it must keep running,
//!   (ii'') only the launcher (its parent) exits while the session leader lives: it must keep
//!          running and idle,
//!   (iii)  stdio is not a terminal at all (non-TTY guard), and
//!   (iv)   SIGHUP, SIGTERM and SIGINT (registered before raw mode) end the session cleanly, and
//!   (v)    the master stays open but is never read (V23) while the session leader dies.
//! Exits 0 when every case passes, 1 when one fails, 2 on a setup error. Unix only (CI: ubuntu
//! and macOS).

#[cfg(unix)]
mod cases;
#[cfg(unix)]
mod helper;
#[cfg(unix)]
mod proc;
#[cfg(unix)]
mod pty;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use std::{io::Write, path::PathBuf, process::ExitCode, time::Duration};

    use rustix::process::Signal;

    use cases::{Config, Outcome};

    type Case<'a> = (&'a str, Box<dyn Fn() -> Outcome + 'a>);

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--helper") {
        return helper::run(&args[1..]);
    }
    let cfg = Config {
        bin: PathBuf::from(
            args.first()
                .map_or("target/debug/wiki-reader", String::as_str),
        ),
        root: PathBuf::from(args.get(1).map_or("fixtures/images", String::as_str)),
    };
    if !cfg.bin.exists() {
        eprintln!("missing binary: {} (build it first)", cfg.bin.display());
        return ExitCode::from(2);
    }

    let cases: Vec<Case<'_>> = vec![
        (
            "(i) master closed, slave held",
            Box::new(|| cases::master_closed(&cfg, Duration::from_secs(1))),
        ),
        (
            "(i') master closed during startup",
            Box::new(|| cases::master_closed(&cfg, Duration::ZERO)),
        ),
        (
            "(ii) session leader killed",
            Box::new(|| cases::leader_killed(&cfg, false)),
        ),
        (
            "(ii') leader killed, WIKI_READER_NO_WATCHDOG=1 keeps it running",
            Box::new(|| cases::leader_killed(&cfg, true)),
        ),
        (
            "(ii'') launcher exits, leader alive: keeps running, idle",
            Box::new(|| cases::launcher_exits(&cfg)),
        ),
        (
            "(iv) SIGHUP ends the session",
            Box::new(|| cases::signal(&cfg, Signal::HUP, "SIGHUP")),
        ),
        (
            "(iv') SIGTERM ends the session",
            Box::new(|| cases::signal(&cfg, Signal::TERM, "SIGTERM")),
        ),
        (
            "(iv'') SIGINT ends the session",
            Box::new(|| cases::signal(&cfg, Signal::INT, "SIGINT")),
        ),
        (
            "(v) V23: unread master, leader killed: still exits",
            Box::new(|| cases::unread_master_leader_killed(&cfg)),
        ),
        ("(iii) not a tty", Box::new(|| cases::not_a_tty(&cfg))),
    ];

    let mut failed = false;
    for (name, case) in cases {
        let (verdict, detail) = match case() {
            Ok(detail) => ("PASS", detail),
            Err(detail) => {
                failed = true;
                ("FAIL", detail)
            }
        };
        println!("{verdict} {name}: {detail}");
        let _ = std::io::stdout().flush();
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("check-orphan-exit needs a Unix pseudo-terminal");
    std::process::ExitCode::from(2)
}
