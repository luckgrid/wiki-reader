//! The V22 cases. Each returns `Ok(detail)` for a pass and `Err(detail)` for a failure.

use std::{
    env, fs,
    os::{fd::OwnedFd, unix::process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    thread,
    time::{Duration, Instant},
};

use rustix::process::Signal;

use crate::{
    proc::{alive, cpu_seconds, ppid_of, send, state, wait_gone},
    pty::{Pty, drain, open_pty, set_winsize},
};

pub type Outcome = Result<String, String>;

/// Watchdog polls every 1 s and hard-exits after a 2 s grace.
pub const LIMIT: Duration = Duration::from_secs(12);

pub struct Config {
    pub bin: PathBuf,
    pub root: PathBuf,
}

fn setup<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|err| format!("setup: {err}"))
}

/// Kills and reaps the child when dropped, so no case leaves a process behind.
struct Guard(Child);

impl Guard {
    fn id(&self) -> u32 {
        self.0.id()
    }

    /// Wait up to `limit` for the child to exit.
    fn wait(&mut self, limit: Duration) -> Option<ExitStatus> {
        let deadline = Instant::now() + limit;
        loop {
            match self.0.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                _ => return None,
            }
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

/// A command that re-executes this binary in one of the helper modes.
fn helper(mode: &str, cfg: &Config) -> Result<Command, String> {
    let exe = setup(env::current_exe())?;
    let mut cmd = Command::new(exe);
    cmd.args(["--helper", mode]).arg(&cfg.bin).arg(&cfg.root);
    Ok(cmd)
}

/// Spawn `cmd` with the slave as stdin, stdout and stderr, and drop the parent's copies again.
fn spawn_on(slave: &OwnedFd, mut cmd: Command) -> Result<Guard, String> {
    cmd.stdin(Stdio::from(setup(slave.try_clone())?))
        .stdout(Stdio::from(setup(slave.try_clone())?))
        .stderr(Stdio::from(setup(slave.try_clone())?));
    Ok(Guard(setup(cmd.spawn())?))
}

fn clean_exit(status: ExitStatus) -> Outcome {
    match status.signal() {
        // For example SIGABRT from a panic while printing to a dead terminal: the process left,
        // but not cleanly.
        Some(sig) => Err(format!("died on signal {sig}, not a clean exit")),
        None => Ok(format!("exited {}", status.code().unwrap_or(0))),
    }
}

/// (i) / (i'): the master closes while the parent stays alive and the slave stays open. This
/// was the ~100 %CPU spin in crossterm's poll/read.
pub fn master_closed(cfg: &Config, delay: Duration) -> Outcome {
    let Pty { master, slave } = setup(open_pty())?;
    let hold = setup(slave.try_clone())?; // keeps the slave open after the master closes
    let mut child = spawn_on(&slave, helper("exec", cfg)?)?;
    drop(slave);
    thread::sleep(delay);
    drop(master);
    let outcome = match child.wait(LIMIT) {
        Some(status) => clean_exit(status),
        None => Err(format!(
            "still running after {}s (pid {})",
            LIMIT.as_secs(),
            child.id()
        )),
    };
    drop(hold);
    outcome
}

static NEXT_PIDFILE: AtomicU32 = AtomicU32::new(0);

/// A reader running inside a session whose leader the case controls.
struct Session {
    leader: Guard,
    app_pid: u32,
    pidfile: PathBuf,
    /// The master end when nothing drains it (V23); dropped after the reader is killed.
    master: Option<OwnedFd>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if alive(self.app_pid) {
            let _ = send(self.app_pid, Signal::KILL);
        }
        let _ = fs::remove_file(&self.pidfile);
        // `leader` is killed and reaped by its own guard.
    }
}

fn wait_for_pid(pidfile: &Path) -> Result<u32, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(pid) = fs::read_to_string(pidfile)
            .ok()
            .and_then(|s| s.trim().parse().ok())
        {
            return Ok(pid);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err("setup: the reader never reported its pid".into())
}

/// Start the reader on a PTY inside a fresh session.
///
/// `opts.launcher == false`: the session leader is also the reader's parent.
/// `opts.launcher == true`: the leader stays alive and idle; a launcher child it spawned is the
/// reader's parent and is the one that can exit.
#[derive(Clone, Copy, Default)]
struct Opts {
    /// Set `WIKI_READER_NO_WATCHDOG=1` for the reader.
    watchdog_off: bool,
    /// Insert a launcher between the session leader and the reader.
    launcher: bool,
    /// Keep the master open but never read it, on a large screen (V23).
    unread_master: bool,
}

fn start_in_session(cfg: &Config, opts: Opts) -> Result<Session, String> {
    let Pty { master, slave } = setup(open_pty())?;
    let master = if opts.unread_master {
        setup(set_winsize(&slave, 120, 400))?;
        Some(master)
    } else {
        drain(master);
        None
    };
    let n = NEXT_PIDFILE.fetch_add(1, Ordering::Relaxed);
    let pidfile = env::temp_dir().join(format!(
        "wiki-reader-orphan-exit-{}-{n}.pid",
        std::process::id()
    ));
    let mut cmd = helper(if opts.launcher { "launcher" } else { "app" }, cfg)?;
    cmd.arg(&pidfile);
    if opts.watchdog_off {
        cmd.env("WIKI_READER_NO_WATCHDOG", "1");
    }
    let leader = spawn_on(&slave, cmd)?;
    drop(slave);
    let mut session = Session {
        leader,
        app_pid: 0,
        pidfile,
        master,
    };
    session.app_pid = wait_for_pid(&session.pidfile)?;
    thread::sleep(Duration::from_millis(1500));
    Ok(session)
}

/// (ii) / (ii'): the session leader (also the parent) dies. The reader spun at ~100 %CPU before
/// V22; with the watchdog disabled it must stay up.
pub fn leader_killed(cfg: &Config, watchdog_off: bool) -> Outcome {
    let mut session = start_in_session(
        cfg,
        Opts {
            watchdog_off,
            ..Opts::default()
        },
    )?;
    let app = session.app_pid;
    if !alive(app) {
        return Err(format!(
            "pid {app} was not running before the leader was killed"
        ));
    }
    setup(send(session.leader.id(), Signal::KILL))?;
    session.leader.wait(LIMIT);
    if !watchdog_off {
        return if wait_gone(app, LIMIT) {
            Ok(format!("pid {app} gone after the session leader died"))
        } else {
            Err(format!(
                "pid {app} still running after the session leader died"
            ))
        };
    }
    // Opt-out: the leader check is off and the terminal is still open, so it stays up.
    if wait_gone(app, Duration::from_secs(5)) {
        Err(format!(
            "pid {app} exited although the leader check is disabled"
        ))
    } else {
        Ok(format!("pid {app} kept running (leader check disabled)"))
    }
}

/// (ii''): only the launcher (the reader's parent) exits; the session leader and the terminal
/// stay alive. The reader must keep running and sit idle: it is a legitimate session.
pub fn launcher_exits(cfg: &Config) -> Outcome {
    let session = start_in_session(
        cfg,
        Opts {
            launcher: true,
            ..Opts::default()
        },
    )?;
    let app = session.app_pid;
    let launcher = ppid_of(app).filter(|&p| p != session.leader.id());
    let Some(launcher) = launcher.filter(|_| alive(app)) else {
        return Err("setup: expected a launcher between the leader and the reader".into());
    };
    setup(send(launcher, Signal::KILL))?;
    thread::sleep(Duration::from_secs(4)); // let the reader settle after the reparent
    if !alive(app) {
        return Err(format!("pid {app} exited when only its launcher exited"));
    }
    let t0 = cpu_seconds(app);
    thread::sleep(Duration::from_secs(4));
    let (Some(t0), Some(t1)) = (t0, cpu_seconds(app)) else {
        return Err(format!("pid {app} vanished while measuring CPU"));
    };
    let used = t1 - t0;
    if used > 2.0 {
        // A spinning reader burns ~4 s of CPU in a 4 s window.
        return Err(format!("pid {app} used {used:.1}s CPU in 4s (spinning)"));
    }
    Ok(format!("pid {app} kept running, {used:.2}s CPU in 4s"))
}

/// (iv): SIGHUP, SIGTERM and SIGINT are registered before raw mode and must end the session
/// cleanly.
pub fn signal(cfg: &Config, sig: Signal, name: &str) -> Outcome {
    let Pty { master, slave } = setup(open_pty())?;
    drain(master);
    let mut child = spawn_on(&slave, helper("exec", cfg)?)?;
    drop(slave);
    thread::sleep(Duration::from_millis(1500));
    if let Ok(Some(status)) = child.0.try_wait() {
        return Err(format!(
            "exited {} before the signal was sent",
            status.code().unwrap_or(-1)
        ));
    }
    setup(send(child.id(), sig))?;
    match child.wait(LIMIT) {
        None => Err(format!("still running {}s after {name}", LIMIT.as_secs())),
        Some(status) => match status.signal() {
            Some(died) => Err(format!(
                "died on signal {died} after {name}, not a clean exit"
            )),
            None => Ok(format!(
                "exited {} after {name}",
                status.code().unwrap_or(0)
            )),
        },
    }
}

/// (iii): stdio is not a terminal at all (the non-TTY guard).
pub fn not_a_tty(cfg: &Config) -> Outcome {
    let child = Command::new(&cfg.bin)
        .arg(&cfg.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = Guard(setup(child)?);
    match child.wait(Duration::from_secs(5)) {
        None => Err("did not exit on /dev/null stdio".into()),
        Some(status) if status.code() == Some(0) => {
            Err("exited 0 on /dev/null stdio (expected an error)".into())
        }
        Some(status) => Ok(format!(
            "exited {}",
            status
                .code()
                .map_or_else(|| "on a signal".into(), |c| c.to_string())
        )),
    }
}

/// (v) V23: the terminal keeps the master open but never reads it, on a screen large enough to
/// fill the PTY buffer, and the session leader dies, so the watchdog hard-exits the reader while
/// its main thread may be blocked in a `write`. The reader must still exit with the master open.
/// (An early test harness saw macOS state `E` here and blamed an undrained close; the exit takes
/// the same ~0.6 s with a drained master, so this case guards the property, not a delay.)
pub fn unread_master_leader_killed(cfg: &Config) -> Outcome {
    let mut session = start_in_session(
        cfg,
        Opts {
            unread_master: true,
            ..Opts::default()
        },
    )?;
    let app = session.app_pid;
    if !alive(app) {
        return Err(format!(
            "pid {app} was not running before the leader was killed"
        ));
    }
    // A few redraws on the large screen, none of them read.
    if let Some(master) = &session.master {
        for _ in 0..10 {
            setup(rustix::io::write(master, b"\x1b[6~"))?;
            thread::sleep(Duration::from_millis(20));
        }
    }
    thread::sleep(Duration::from_millis(1500));
    setup(send(session.leader.id(), Signal::KILL))?;
    session.leader.wait(LIMIT);
    // Watchdog: polls every 1 s and hard-exits after a 2 s grace.
    if wait_gone(app, Duration::from_secs(8)) {
        return Ok(format!(
            "pid {app} exited although nothing reads the master"
        ));
    }
    let stuck = state(app).unwrap_or_else(|| "?".into());
    Err(format!(
        "pid {app} still running (state {stuck}) with the master open and unread"
    ))
}
