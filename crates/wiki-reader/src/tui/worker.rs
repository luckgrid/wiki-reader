//! Panic isolation for background workers.
//!
//! Workers run third-party renderers and decoders on untrusted input. A panic there must become
//! an error the UI can show, not a dead thread that leaves the viewer on "rendering…" forever.

use std::panic::{self, AssertUnwindSafe};

/// Run `f`, turning a panic into `Err("<what> crashed: <message>")`.
pub fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_owned());
        format!("{what} crashed: {message}")
    })
}

#[cfg(test)]
mod tests {
    use super::guarded;

    #[test]
    fn a_panic_becomes_an_error_with_its_message() {
        let ok = guarded("job", || 7);
        assert_eq!(ok, Ok(7));
        let boom = guarded("job", || -> u8 { panic!("bad svg {}", 3) });
        assert_eq!(boom, Err("job crashed: bad svg 3".to_owned()));
        let str_boom = guarded("job", || -> u8 { panic!("static") });
        assert_eq!(str_boom, Err("job crashed: static".to_owned()));
    }
}
