//! Hidden helper modes: the binary re-executes itself so a child can call `setsid` before it
//! runs the reader. `std` only offers that through the unsafe `pre_exec`, and the workspace
//! forbids `unsafe`; rustix's `setsid` is safe, and `Command::exec` is too.

use std::{
    env, fs,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, ExitCode},
    thread,
    time::Duration,
};

use rustix::process::setsid;

/// How long a session leader that delegates to a launcher stays alive.
const LEADER_LIFETIME: Duration = Duration::from_secs(60);

/// `check-orphan-exit --helper <mode> <bin> <root> [pidfile]`; `args` starts at `<mode>`.
pub fn run(args: &[String]) -> ExitCode {
    let [mode, bin, root, rest @ ..] = args else {
        eprintln!("helper: expected <mode> <bin> <root> [pidfile]");
        return ExitCode::from(2);
    };
    let pidfile = rest.first().map(Path::new);
    let new_session = matches!(mode.as_str(), "exec" | "app" | "launcher");
    if new_session && let Err(err) = setsid() {
        eprintln!("helper: setsid failed: {err}");
        return ExitCode::from(2);
    }
    match (mode.as_str(), pidfile) {
        // The reader itself becomes the session leader: same pid, no wrapper.
        ("exec", _) => {
            let err = Command::new(bin).arg(root).exec();
            eprintln!("helper: cannot exec {bin}: {err}");
            ExitCode::from(127)
        }
        // Session leader that is also the reader's parent.
        ("app" | "spawn", Some(pidfile)) => spawn_and_wait(bin, root, pidfile),
        // Session leader that stays idle while a launcher child starts the reader.
        ("launcher", Some(pidfile)) => {
            let spawned = env::current_exe().and_then(|exe| {
                Command::new(exe)
                    .args(["--helper", "spawn", bin, root])
                    .arg(pidfile)
                    .spawn()
            });
            if let Err(err) = spawned {
                eprintln!("helper: cannot start the launcher: {err}");
                return ExitCode::from(2);
            }
            thread::sleep(LEADER_LIFETIME);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("helper: unknown mode {mode:?} or missing pidfile");
            ExitCode::from(2)
        }
    }
}

/// Start the reader with inherited stdio, publish its pid, wait for it. No shell job control:
/// `sh` implementations differ in what a background job gets as stdin (dash gives it
/// `/dev/null`), which would make every case vacuous.
fn spawn_and_wait(bin: &str, root: &str, pidfile: &Path) -> ExitCode {
    let mut child = match Command::new(bin).arg(root).spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("helper: cannot start {bin}: {err}");
            return ExitCode::from(2);
        }
    };
    // Write then rename, so the harness never reads a half-written pid.
    let tmp = pidfile.with_extension("tmp");
    if let Err(err) =
        fs::write(&tmp, format!("{}\n", child.id())).and_then(|()| fs::rename(&tmp, pidfile))
    {
        eprintln!("helper: cannot write the pid file: {err}");
        return ExitCode::from(2);
    }
    let _ = child.wait();
    ExitCode::SUCCESS
}
