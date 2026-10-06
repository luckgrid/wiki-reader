//! Process inspection through `ps` (the same view on macOS and Linux) and signals via rustix.

use std::{
    io,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use rustix::process::{Pid, Signal, kill_process};

/// One `ps -o <field>= -p <pid>` value, or `None` when the process does not exist.
fn ps_field(field: &str, pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", &format!("{field}="), "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// True while `pid` exists and is not a zombie (`ps` reports `Z`, or nothing).
pub fn alive(pid: u32) -> bool {
    ps_field("stat", pid).is_some_and(|stat| !stat.starts_with('Z'))
}

/// Wait up to `limit` for `pid` to disappear.
pub fn wait_gone(pid: u32, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

/// The `ps` state of `pid` (for example `S`, `R+`, `Z`, or `E` for a macOS process stuck exiting).
pub fn state(pid: u32) -> Option<String> {
    ps_field("stat", pid)
}

pub fn ppid_of(pid: u32) -> Option<u32> {
    ps_field("ppid", pid)?.parse().ok()
}

/// Cumulative CPU time of `pid` in seconds, or `None` when the process is gone.
pub fn cpu_seconds(pid: u32) -> Option<f64> {
    parse_cpu_time(&ps_field("time", pid)?)
}

/// Parse `ps` TIME: `[[dd-]hh:]mm:ss[.cc]`.
pub fn parse_cpu_time(text: &str) -> Option<f64> {
    let (days, clock) = match text.split_once('-') {
        Some((days, rest)) => (days.parse::<u32>().ok()?, rest),
        None => (0, text),
    };
    let mut secs = 0.0_f64;
    for part in clock.split(':') {
        secs = secs.mul_add(60.0, part.parse::<f64>().ok()?);
    }
    Some(f64::from(days).mul_add(86400.0, secs))
}

pub fn send(pid: u32, signal: Signal) -> io::Result<()> {
    let raw = i32::try_from(pid).map_err(|_| io::Error::other("pid out of range"))?;
    let pid = Pid::from_raw(raw).ok_or_else(|| io::Error::other("invalid pid"))?;
    Ok(kill_process(pid, signal)?)
}

#[cfg(test)]
mod tests {
    use super::parse_cpu_time;

    #[test]
    fn parses_every_ps_time_shape() {
        assert_eq!(parse_cpu_time("0:00.12"), Some(0.12));
        assert_eq!(parse_cpu_time("01:02"), Some(62.0));
        assert_eq!(parse_cpu_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_cpu_time("2-01:00:00"), Some(2.0 * 86400.0 + 3600.0));
        assert_eq!(parse_cpu_time("nonsense"), None);
        assert_eq!(parse_cpu_time(""), None);
    }
}
