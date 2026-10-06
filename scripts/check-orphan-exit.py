#!/usr/bin/env python3
"""V22 regression check: an orphaned wiki-reader must exit, not spin.

Usage: scripts/check-orphan-exit.py [path/to/wiki-reader] [collection-root]

Runs the binary on a pseudo-terminal and checks that it exits by itself (an exit code,
not a crash signal) when
  (i)   the PTY master closes while the parent stays alive and the slave stays open
        (the original ~100 %CPU spin in crossterm's poll/read),
  (i')  the same, with the master closed while the app is still starting up (a closed tab),
  (ii)  its parent process is killed (reparented to init), and
  (iii) stdio is not a terminal at all (non-TTY guard).
Exits 0 when every case passes. Unix only (CI: ubuntu and macOS).
"""
import os
import pty
import signal
import subprocess
import sys
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else "target/debug/wiki-reader"
ROOT = sys.argv[2] if len(sys.argv) > 2 else "fixtures/images"
LIMIT = 12.0  # seconds: watchdog polls every 1 s and hard-exits after a 2 s grace


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


def case_parent_killed():
    master, slave = pty.openpty()
    pid_r, pid_w = os.pipe()
    # `sh` is the parent; the reader runs in the background and reports its pid.
    shell = subprocess.Popen(
        ["sh", "-c", f'"$0" "$1" & echo $! >&{pid_w}; wait', BIN, ROOT],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        pass_fds=(pid_w,),
        start_new_session=True,
    )
    os.close(slave)
    os.close(pid_w)
    app_pid = None
    try:
        app_pid = int(os.read(pid_r, 32).decode().strip())
        time.sleep(1.0)
        shell.send_signal(signal.SIGKILL)
        shell.wait()
        if wait_gone(app_pid):
            return True, f"pid {app_pid} gone after parent kill"
        return False, f"pid {app_pid} still running after parent kill"
    finally:
        if app_pid is not None and alive(app_pid):
            os.kill(app_pid, signal.SIGKILL)
        os.close(pid_r)
        os.close(master)


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
        ("(ii) parent killed", case_parent_killed),
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
