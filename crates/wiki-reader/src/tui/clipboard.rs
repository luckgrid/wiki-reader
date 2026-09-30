//! OSC 52 clipboard write (injectable for tests).

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

/// Soft cap for OSC 52 payloads (many terminals drop larger sequences).
pub const OSC52_MAX_BYTES: usize = 100_000;

/// Write OSC 52 clipboard payloads (swappable in tests).
pub trait ClipboardWriter: Send {
    /// Copy `text` to the terminal clipboard via OSC 52.
    ///
    /// # Errors
    ///
    /// Propagates write failures, or `InvalidInput` when over [`OSC52_MAX_BYTES`].
    fn copy(&mut self, text: &str) -> io::Result<()>;
}

/// Real stdout OSC 52 writer.
#[derive(Debug, Default)]
pub struct Osc52Clipboard;

impl ClipboardWriter for Osc52Clipboard {
    fn copy(&mut self, text: &str) -> io::Result<()> {
        use std::io::stdout;
        if text.len() > OSC52_MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "copy too large ({} bytes; max {OSC52_MAX_BYTES})",
                    text.len()
                ),
            ));
        }
        let b64 = base64_encode(text.as_bytes());
        let mut out = stdout().lock();
        write!(out, "\x1b]52;c;{b64}\x07")?;
        out.flush()
    }
}

/// Recording clipboard for tests.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // constructed from `#[cfg(test)]` app tests
pub struct RecordingClipboard {
    pub copied: Arc<Mutex<Vec<String>>>,
}

impl ClipboardWriter for RecordingClipboard {
    fn copy(&mut self, text: &str) -> io::Result<()> {
        if text.len() > OSC52_MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "copy too large ({} bytes; max {OSC52_MAX_BYTES})",
                    text.len()
                ),
            ));
        }
        self.copied
            .lock()
            .expect("clipboard mutex")
            .push(text.to_owned());
        Ok(())
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n =
            (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    match bytes.len() - i {
        1 => {
            let n = u32::from(bytes[i]) << 16;
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn oversize_copy_refused() {
        let mut clip = RecordingClipboard::default();
        let big = "x".repeat(OSC52_MAX_BYTES + 1);
        let err = clip.copy(&big).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(clip.copied.lock().unwrap().is_empty());
    }
}
