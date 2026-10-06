#!/usr/bin/env python3
"""V22 regression check: an orphaned wiki-reader must exit, not spin.

Usage: scripts/check-orphan-exit.py [path/to/wiki-reader] [collection-root]

Runs the binary on a pseudo-terminal and checks that it exits by itself (an exit code,
not a crash signal) when
  (i)   the PTY master closes while the parent stays alive and the slave stays open
        (the original ~100 %CPU spin in crossterm's poll/read),
  (i')  the same, with the master closed while the app is still starting up (a closed tab),
  (ii)  its session leader (the shell that owns the session) is killed,
  (ii') the same with WIKI_READER_NO_WATCHDOG=1, where it must keep running,
  (ii'') only the launcher (its parent) exits while the session leader lives: it must keep
        running and idle, and
  (iii) stdio is not a terminal at all (non-TTY guard).
Exits 0 when every case passes. Unix only (CI: ubuntu and macOS).
"""
import os
import pty
import signal
import subprocess
import sys
import threading
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else "target/debug/wiki-reader"
ROOT = sys.argv[2] if len(sys.argv) > 2 else "fixtures/images"
LIMIT = 12.0  # seconds: watchdog polls every 1 s and hard-exits after a 2 s grace


def drain(master):
    """Read and discard the master side, like a live terminal emulator would.

    Without a reader the pty buffer fills, the app blocks writing (and, on macOS, blocks in
    exit while closing the slave), which would make the "parent killed" cases hang.
    """

    def run():
        try:
            while os.read(master, 65536):
                pass
        except OSError:
            pass

    threading.Thread(target=run, daemon=True).start()


def alive(pid):
    """True while `pid` exists and is not a zombie (ps reports Z, or nothing)."""
    out = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True
    ).stdout.strip()
    return bool(out) and not out.startswith("Z")


def wait_gone(pid, limit=LIMIT):
    deadline = time.time() + limit
    while time.time() < deadline:
        if not alive(pid):
            return True
        time.sleep(0.2)
    return False


def case_master_closed(delay=1.0):
    master, slave = pty.openpty()
    hold = os.dup(slave)  # keeps the slave open after the master closes
    proc = subprocess.Popen(
        [BIN, ROOT], stdin=slave, stdout=slave, stderr=slave, start_new_session=True
    )
    os.close(slave)
    try:
        time.sleep(delay)
        os.close(master)
        try:
            proc.wait(timeout=LIMIT)
            if proc.returncode < 0:
                # Killed by a signal (for example SIGABRT from a panic while printing to the
                # dead terminal): the process left, but not cleanly.
                return False, f"died on signal {-proc.returncode}, not a clean exit"
            return True, f"exited {proc.returncode}"
        except subprocess.TimeoutExpired:
            return False, f"still running after {LIMIT:.0f}s (pid {proc.pid})"
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
        os.close(hold)


def cpu_seconds(pid):
    """Cumulative CPU time of `pid` in seconds (ps TIME: [[dd-]hh:]mm:ss[.cc])."""
    out = subprocess.run(
        ["ps", "-o", "time=", "-p", str(pid)], capture_output=True, text=True
    ).stdout.strip()
    if not out:
        return None
    days = 0
    if "-" in out:
        d, out = out.split("-", 1)
        days = int(d)
    secs = 0.0
    for part in out.split(":"):
        secs = secs * 60 + float(part)
    return days * 86400 + secs


# Child program run inside the session: start the reader (stdio inherited, so it keeps the PTY),
# report its pid on fd `w`, then wait. No shell job control: `sh` implementations differ in what
# a background job gets as stdin (dash gives it /dev/null), which would make every case vacuous.
SPAWN_APP = (
    "import os, subprocess, sys\n"
    "bin_, root, w = sys.argv[1], sys.argv[2], int(sys.argv[3])\n"
    "p = subprocess.Popen([bin_, root])\n"
    "os.write(w, (str(p.pid) + '\\n').encode())\n"
    "os.close(w)\n"
    "p.wait()\n"
)
# Session leader that stays alive and delegates the spawn to a launcher child.
SPAWN_VIA_LAUNCHER = (
    "import subprocess, sys, time\n"
    "bin_, root, w, prog = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]\n"
    "subprocess.Popen([sys.executable, '-c', prog, bin_, root, w], pass_fds=(int(w),))\n"
    "time.sleep(60)\n"
)


def start_in_session(env, launcher):
    """Start the reader on a PTY inside a fresh session; return (leader, app_pid, master, pipe).

    launcher=False: the leader process is both the session leader and the reader's parent.
    launcher=True:  the leader stays alive and idle; a launcher child it spawned is the
                    reader's parent and is the one that can exit.
    """
    master, slave = pty.openpty()
    drain(master)
    pid_r, pid_w = os.pipe()
    if launcher:
        argv = [sys.executable, "-c", SPAWN_VIA_LAUNCHER, BIN, ROOT, str(pid_w), SPAWN_APP]
    else:
        argv = [sys.executable, "-c", SPAWN_APP, BIN, ROOT, str(pid_w)]
    leader = subprocess.Popen(
        argv,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        pass_fds=(pid_w,),
        start_new_session=True,
        env=env,
    )
    os.close(slave)
    os.close(pid_w)
    app_pid = int(os.read(pid_r, 32).decode().strip())
    time.sleep(1.5)
    return leader, app_pid, master, pid_r


def ppid_of(pid):
    out = subprocess.run(
        ["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True
    ).stdout.strip()
    return int(out)


def finish(leader, app_pid, master, pid_r):
    if app_pid is not None and alive(app_pid):
        os.kill(app_pid, signal.SIGKILL)
    if leader.poll() is None:
        leader.kill()
        leader.wait()
    os.close(pid_r)
    os.close(master)


def case_leader_killed(env=None, expect_exit=True):
    """The session leader (also the parent) dies: the reader spun at ~100 %CPU before V22."""
    leader, app_pid, master, pid_r = start_in_session(env, launcher=False)
    try:
        if not alive(app_pid):
            return False, f"pid {app_pid} was not running before the leader was killed"
        leader.send_signal(signal.SIGKILL)
        leader.wait()
        if expect_exit:
            if wait_gone(app_pid):
                return True, f"pid {app_pid} gone after the session leader died"
            return False, f"pid {app_pid} still running after the session leader died"
        # Opt-out: the leader check is off and the terminal is still open, so it stays up.
        if wait_gone(app_pid, limit=5.0):
            return False, f"pid {app_pid} exited although the leader check is disabled"
        return True, f"pid {app_pid} kept running (leader check disabled)"
    finally:
        finish(leader, app_pid, master, pid_r)


def case_launcher_exits():
    """Only the launcher (parent) exits; the session leader and the terminal stay alive.

    The reader must keep running and sit idle (it is a legitimate session, not an orphan).
    """
    leader, app_pid, master, pid_r = start_in_session(None, launcher=True)
    try:
        launcher_pid = ppid_of(app_pid)
        if launcher_pid == leader.pid or not alive(app_pid):
            return False, "setup: expected a launcher between the leader and the reader"
        os.kill(launcher_pid, signal.SIGKILL)
        time.sleep(4.0)  # let the reader settle after the reparent
        if not alive(app_pid):
            return False, f"pid {app_pid} exited when only its launcher exited"
        t0 = cpu_seconds(app_pid)
        time.sleep(4.0)
        t1 = cpu_seconds(app_pid)
        if t0 is None or t1 is None:
            return False, f"pid {app_pid} vanished while measuring CPU"
        used = t1 - t0
        if used > 2.0:  # a spinning reader burns ~4 s of CPU in a 4 s window
            return False, f"pid {app_pid} used {used:.1f}s CPU in 4s (spinning)"
        return True, f"pid {app_pid} kept running, {used:.2f}s CPU in 4s"
    finally:
        finish(leader, app_pid, master, pid_r)


def case_not_a_tty():
    with open(os.devnull, "rb") as dev_in, open(os.devnull, "wb") as dev_out:
        proc = subprocess.Popen([BIN, ROOT], stdin=dev_in, stdout=dev_out, stderr=dev_out)
        try:
            code = proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
            return False, "did not exit on /dev/null stdio"
    if code == 0:
        return False, "exited 0 on /dev/null stdio (expected an error)"
    return True, f"exited {code}"


def main():
    if not os.path.exists(BIN):
        print(f"missing binary: {BIN} (build it first)", file=sys.stderr)
        return 2
    cases = [
        ("(i) master closed, slave held", case_master_closed),
        ("(i') master closed during startup", lambda: case_master_closed(0.0)),
        ("(ii) session leader killed", case_leader_killed),
        (
            "(ii') leader killed, WIKI_READER_NO_WATCHDOG=1 keeps it running",
            lambda: case_leader_killed(dict(os.environ, WIKI_READER_NO_WATCHDOG="1"), False),
        ),
        ("(ii'') launcher exits, leader alive: keeps running, idle", case_launcher_exits),
        ("(iii) not a tty", case_not_a_tty),
    ]
    failed = False
    for name, fn in cases:
        ok, detail = fn()
        print(f"{'PASS' if ok else 'FAIL'} {name}: {detail}")
        failed |= not ok
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
