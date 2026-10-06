//! Pseudo-terminal plumbing, all through rustix's safe API (the workspace forbids `unsafe`).

use std::{
    fs::File,
    io::{self, Read},
    os::fd::OwnedFd,
    thread,
};

use rustix::{
    fs::{Mode, OFlags, open},
    io::{FdFlags, fcntl_setfd},
    pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
    termios::{Winsize, tcsetwinsize},
};

/// Both ends of a fresh pseudo-terminal.
pub struct Pty {
    pub master: OwnedFd,
    pub slave: OwnedFd,
}

/// Open a PTY like Python's `pty.openpty()`. Neither end becomes a controlling terminal
/// (`NOCTTY`), and both are close-on-exec so that a child holds only the descriptors it is
/// handed as stdio; a leaked master would keep the "master closed" cases from ever closing.
pub fn open_pty() -> io::Result<Pty> {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    grantpt(&master)?;
    unlockpt(&master)?;
    let name = ptsname(&master, Vec::new())?;
    let slave = open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY,
        Mode::empty(),
    )?;
    fcntl_setfd(&master, FdFlags::CLOEXEC)?;
    fcntl_setfd(&slave, FdFlags::CLOEXEC)?;
    Ok(Pty { master, slave })
}

/// Resize the terminal; a large screen makes every frame big enough to fill the PTY buffer.
pub fn set_winsize(slave: &OwnedFd, rows: u16, cols: u16) -> io::Result<()> {
    Ok(tcsetwinsize(
        slave,
        Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )?)
}

/// Read and discard everything on `master`, like a live terminal emulator would.
///
/// Without a reader the PTY buffer fills, the app blocks writing, and the "parent killed" cases
/// would hang. The thread owns the master and ends when the last slave descriptor closes (the
/// read then fails), so callers just kill the processes on the slave side to clean up.
pub fn drain(master: OwnedFd) {
    thread::spawn(move || {
        let mut file = File::from(master);
        let mut buf = vec![0_u8; 65536];
        while matches!(file.read(&mut buf), Ok(n) if n > 0) {}
    });
}
